//! The roads, ruts and runways painted into the ground on the GPU (V67, [`crate::ground_mesh`]).
//!
//! The original draws a freeride scene's screen-space meshes from above into the ground's paint
//! target, after the splat's passes, and the ground samples that target. Here they are drawn from
//! above into two targets of their own, each of which holds what the meshes do to whatever paint is
//! under them — `over` in the colour and `keep` in the alpha, the paint becoming `paint * keep +
//! over` ([`GroundMeshes::affine`]) — and the ground's shader applies them to the splat's colour:
//!
//! * **far**: the whole of the meshes' rectangle, drawn once when the ground is set;
//! * **near**: [`NEAR_FEET`] round a view's eye at [`NEAR_PER_FOOT`] texels a foot, drawn again
//!   whenever the eye has moved to another [`STEP`]-foot square, so the meshes under the rider are as
//!   sharp as their own textures.
//!
//! Every view has a near target of its own (V89: split screen's second player saw only the far
//! target's whole-map paint), and views standing in the same square share one. Each near target has
//! its own bind group — group 2 of the ground's pipeline: the near and far targets and where they lie
//! — and a view's ground is drawn with its target's group. Both are read through a clamped sampler
//! from the renderer's own (V91), trilinear and as anisotropic as its Texture Filtering, so a road
//! seen down its length keeps its paint sharp across rather than picking one blurred mip; the
//! renderer hands a new one to [`GpuMeshes::set_sampler`] and every group is made again. A near target is 2048² texels with its
//! mips, about 21 MiB, made only when a view stands in a square no other view's target holds, so one
//! view costs what it always did and eight views apart cost about 171 MiB; a target no view needs any
//! more is drawn over for the next square that does, and the pool shrinks back when views go.
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

/// A near target, the square its eye stood in when it was drawn, and the ground's bind group that
/// samples it: the ground shader's uniform `at` (this near square and the far one, each
/// `x0, z0, 1 / side, 0`), this target and the far one.
struct Near {
    target: Target,
    square: Option<[i32; 2]>,
    at: wgpu::Buffer,
    ground: wgpu::BindGroup,
    /// The anisotropy `ground`'s sampler filtered with when it was made.
    anisotropy: u16,
}

pub(super) struct GpuMeshes {
    /// The sampler group 2 reads both targets with (V91), and the anisotropy it filters with.
    sampler: wgpu::Sampler,
    anisotropy: u16,
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    /// Runs of triangles with one texture, in drawing order: texture, first vertex, vertex count.
    runs: Vec<(usize, u32, u32)>,
    textures: Vec<wgpu::BindGroup>,
    rect_layout: wgpu::BindGroupLayout,
    /// The ground pipeline's group 2: [`layout_entries`].
    pub(super) layout: wgpu::BindGroupLayout,
    nears: Vec<Near>,
    /// Which of `nears` each view's ground samples, as of the last [`GpuMeshes::follow`].
    view_near: Vec<usize>,
    far: Target,
    far_rect: [f32; 4],
}

/// The ground shader's declarations: its group 2, one per near target.
pub(super) const WGSL_BINDINGS: &str = "struct MeshAt { near: vec4f, far: vec4f }\n\
  @group(2) @binding(0) var mesh_near: texture_2d<f32>;\n\
  @group(2) @binding(1) var mesh_far: texture_2d<f32>;\n\
  @group(2) @binding(2) var<uniform> mesh_at: MeshAt;\n\
  @group(2) @binding(3) var mesh_smp: sampler;\n";

/// The ground shader's code that lays the meshes over `frame`, the splat's colour in 0..255, at the
/// world's `x` and `z`. Both targets are sampled everywhere, as the derivatives need, and the near
/// one is used where it reaches.
pub(super) const APPLY_WGSL: &str = "  let mesh_un = (vec2f(x, z) - mesh_at.near.xy) * mesh_at.near.z;\n\
  let mesh_uf = (vec2f(x, z) - mesh_at.far.xy) * mesh_at.far.z;\n\
  let mesh_n = textureSample(mesh_near, mesh_smp, mesh_un);\n\
  let mesh_f = textureSample(mesh_far, mesh_smp, mesh_uf);\n\
  let mesh_in_near = all(mesh_un > vec2f(0.001)) && all(mesh_un < vec2f(0.999));\n\
  let mesh_in_far = all(mesh_uf >= vec2f(0.0)) && all(mesh_uf <= vec2f(1.0));\n\
  let mesh = select(select(vec4f(0.0, 0.0, 0.0, 1.0), mesh_f, mesh_in_far), mesh_n, mesh_in_near);\n\
  frame = vec4f(frame.rgb * mesh.a + mesh.rgb * 255.0, frame.a);\n";

/// The bind group layout entries of the ground's group 2, [`WGSL_BINDINGS`].
fn layout_entries() -> [wgpu::BindGroupLayoutEntry; 4] {
    [
        texture_entry(0, true, wgpu::TextureViewDimension::D2),
        texture_entry(1, true, wgpu::TextureViewDimension::D2),
        uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
        sampler_entry(3),
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
    /// `sampler` draws the meshes' own textures into the targets; `read` is what the ground reads the
    /// targets with and its anisotropy ([`GpuMeshes::set_sampler`]).
    pub(super) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sampler: &wgpu::Sampler,
        read: (&wgpu::Sampler, u16),
        m: &GroundMeshes,
    ) -> GpuMeshes {
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
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ground meshes"),
            entries: &layout_entries(),
        });
        let far = target(device, &rect_layout, FAR_SIZE, "ground meshes far");
        let (lo, hi) = m.bounds();
        let side = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(1.0);
        let far_rect = [lo[0], lo[1], 1.0 / side, 0.0];
        let mut meshes = GpuMeshes {
            sampler: read.0.clone(),
            anisotropy: read.1,
            pipeline,
            vertices: vertex_buffer,
            runs,
            textures,
            rect_layout,
            layout,
            nears: Vec::new(),
            view_near: Vec::new(),
            far,
            far_rect,
        };
        // One near target to begin with, whose square is nowhere until the first frame.
        meshes.add_near(device, queue);
        queue.write_buffer(&meshes.far.rect, 0, bytemuck::cast_slice(&far_rect));
        let mut encoder = device.create_command_encoder(&Default::default());
        meshes.draw(&mut encoder, &meshes.far);
        queue.submit([encoder.finish()]);
        meshes
    }

    /// Another near target, and its ground bind group.
    fn add_near(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let target = target(device, &self.rect_layout, NEAR_SIZE, "ground meshes near");
        let at = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ground meshes at"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&at, 0, bytemuck::cast_slice(&[[f32::MAX, f32::MAX, 0.0, 0.0], self.far_rect]));
        let ground = self.ground_bind(device, &target, &at);
        self.nears.push(Near { target, square: None, at, ground, anisotropy: self.anisotropy });
    }

    /// The ground's group 2 over near target `target` placed by `at`: it, the far one and the sampler.
    fn ground_bind(&self, device: &wgpu::Device, target: &Target, at: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ground meshes"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&target.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.far.view) },
                wgpu::BindGroupEntry { binding: 2, resource: at.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    /// Read both targets with `sampler`, filtering up to `anisotropy`, from now on (V91): every near
    /// target's group is made again, and with it the far target's reading.
    pub(super) fn set_sampler(&mut self, device: &wgpu::Device, sampler: &wgpu::Sampler, anisotropy: u16) {
        self.sampler = sampler.clone();
        self.anisotropy = anisotropy;
        for i in 0..self.nears.len() {
            let ground = self.ground_bind(device, &self.nears[i].target, &self.nears[i].at);
            self.nears[i].ground = ground;
            self.nears[i].anisotropy = anisotropy;
        }
    }

    /// The anisotropy each near target's group was made with: what a test reads (V91).
    pub(super) fn bound_anisotropy(&self) -> impl Iterator<Item = u16> + '_ {
        self.nears.iter().map(|n| n.anisotropy)
    }

    /// Give each of `eyes` (a view's each) the near target round its square: one already there, or
    /// one no view needs any more drawn again there, or a new one.
    pub(super) fn follow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, eyes: &[V3]) {
        let squares: Vec<[i32; 2]> = eyes.iter().map(|e| [(e.x / STEP).floor() as i32, (e.z / STEP).floor() as i32]).collect();
        // No more targets than views: a seat that has gone gives its memory back.
        self.nears.truncate(squares.len().max(1));
        let wanted = |sq: Option<[i32; 2]>| sq.is_some_and(|sq| squares.contains(&sq));
        self.view_near.clear();
        for &square in &squares {
            let n = match self.nears.iter().position(|n| n.square == Some(square)) {
                Some(n) => n,
                None => {
                    let n = match self.nears.iter().position(|n| !wanted(n.square)) {
                        Some(n) => n,
                        None => {
                            self.add_near(device, queue);
                            self.nears.len() - 1
                        }
                    };
                    self.nears[n].square = Some(square);
                    let centre = [(square[0] as f32 + 0.5) * STEP, (square[1] as f32 + 0.5) * STEP];
                    let rect = [centre[0] - NEAR_FEET / 2.0, centre[1] - NEAR_FEET / 2.0, 1.0 / NEAR_FEET, 0.0];
                    queue.write_buffer(&self.nears[n].target.rect, 0, bytemuck::cast_slice(&rect));
                    queue.write_buffer(&self.nears[n].at, 0, bytemuck::cast_slice(&[rect, self.far_rect]));
                    self.draw(encoder, &self.nears[n].target);
                    n
                }
            };
            self.view_near.push(n);
        }
    }

    /// The ground's group 2 for view `v`: its near target, the far one, and where they lie.
    pub(super) fn bind(&self, v: usize) -> &wgpu::BindGroup {
        &self.nears[self.view_near.get(v).copied().unwrap_or(0).min(self.nears.len() - 1)].ground
    }

    /// How many near targets there are, for the tests.
    #[cfg(test)]
    pub(super) fn near_targets(&self) -> usize {
        self.nears.len()
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

/// A target `size` texels square with all its mips, and its square's uniform for drawing into it.
fn target(device: &wgpu::Device, rect_layout: &wgpu::BindGroupLayout, size: u32, label: &str) -> Target {
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
        layout: rect_layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: rect.as_entire_binding() }],
    });
    Target { levels: level_views, view: texture.create_view(&Default::default()), rect, bind }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ground_mesh::Flat;
    use crate::mat::v3;
    use crate::pst::Texture;
    use crate::splat::Pass;

    #[test]
    fn each_view_gets_a_near_target_and_views_in_one_square_share_it() {
        let gpu = match super::super::Renderer::headless() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipped: {e}");
                return;
            }
        };
        let pass = Pass {
            texture: 0,
            scale: [1.0, 1.0],
            angle: 0.0,
            turn: (0.0, 1.0),
            rgba: [128.0; 4],
            blend: Some([0, 1, 0, 1]),
            fixed: 128.0,
            reads: (true, true),
            mip: None,
        };
        let tri = Flat::new([[0.0, 0.0], [4000.0, 0.0], [0.0, 4000.0]], [[0.0; 2]; 3], [[128.0; 4]; 3], pass);
        let m = GroundMeshes::of(vec![Texture { width: 2, height: 2, rgba: vec![128; 16] }], vec![tri]).unwrap();
        let mut meshes = GpuMeshes::new(&gpu.device, &gpu.queue, &gpu.sampler, (&gpu.sampler, 1), &m);
        let mut follow = |eyes: &[V3]| {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            meshes.follow(&gpu.device, &gpu.queue, &mut encoder, eyes);
            gpu.queue.submit([encoder.finish()]);
            (meshes.near_targets(), meshes.view_near.clone())
        };
        let (a, b, c) = (v3(100.0, 0.0, 100.0), v3(110.0, 5.0, 120.0), v3(3000.0, 0.0, 3000.0));
        // One view, one target, as before V89.
        assert_eq!(follow(&[a]), (1, vec![0]));
        // Two views in one square share it; apart, the second gets its own.
        assert_eq!(follow(&[a, b]), (1, vec![0, 0]));
        assert_eq!(follow(&[a, c]), (2, vec![0, 1]));
        // Swapping seats swaps targets without drawing either again.
        assert_eq!(follow(&[c, a]), (2, vec![1, 0]));
        // A new sampler (V91's Texture Filtering) reaches both targets' groups.
        meshes.set_sampler(&gpu.device, &gpu.sampler, 16);
        assert_eq!(meshes.bound_anisotropy().collect::<Vec<_>>(), vec![16, 16]);
        let mut follow = |eyes: &[V3]| {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            meshes.follow(&gpu.device, &gpu.queue, &mut encoder, eyes);
            gpu.queue.submit([encoder.finish()]);
            (meshes.near_targets(), meshes.view_near.clone())
        };
        // Eight seats apart: eight targets; four of them gone, four.
        let eight: Vec<V3> = (0..8).map(|k| v3(k as f32 * 500.0, 0.0, 0.0)).collect();
        assert_eq!(follow(&eight).0, 8);
        assert_eq!(follow(&eight[..4]).0, 4);
        // A view that moves to a square no one else holds takes over a target no view needs.
        let (n, at) = follow(&[eight[0], eight[1], eight[2], v3(-900.0, 0.0, 0.0)]);
        assert_eq!((n, &at[..3]), (4, &[0, 1, 2][..]));
        assert_eq!(at[3], 3);
    }
}
