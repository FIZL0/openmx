//! The GPU renderer (P44): the world [`crate::raster`] draws, drawn through wgpu.
//!
//! The software rasteriser fills every pixel on one CPU thread, and at 1080p that is over half a
//! second a frame. This draws the same things — the sky, the ground and its paint, every prop the
//! scene places, the bikes and the riders — with the work where a GPU wants it:
//!
//! * **Everything is uploaded once.** A model's strips become one vertex buffer the first time it is
//!   drawn, sorted by level of detail and material, and a texture becomes a GPU texture (with a mip
//!   chain, built here) the first time a material asks for it. Neither crosses the bus again.
//! * **Skinning is on the GPU.** Every vertex of a `.psg` hangs off one node, so a pose is only the
//!   node matrices: the vertex shader reads its node's matrix out of a palette texture. A prop's
//!   palette is its bind pose, written once; a body's or a chassis' is the frame's pose — a hundred
//!   matrices a rider, rather than every triangle re-made on the CPU (`skin::skin`).
//! * **Draws are batched and culled.** Each view's props are culled against its frustum and its reach
//!   (the fog's end, or none: [`Graphics`]), and the ones under a pixel across are dropped. The rest
//!   are gathered by model and level of detail, the disc's own test choosing it ([`Model::group_for`]),
//!   and drawn with one call per material for every copy.
//! * **The ground has levels of detail** (V32). It is uploaded once as square chunks of corners, and
//!   each view draws each chunk it sees at the coarsest level whose height error is under a pixel,
//!   stitched to its neighbours ([`crate::terrain::lod_indices`]), in one draw.
//! * **Depth is reversed** (V32): 1 at the near plane and 0 at the far one, so a float depth buffer
//!   keeps its precision out to the far side of a whole map and the ruts and shadows laid on the
//!   ground do not fight it there.
//! * **One pass for every view.** A split screen's views (V22) are viewports of one render pass, each
//!   with its own camera.
//!
//! The light, fog, cut-out and blends are [`crate::world`]'s: the sun and ambient on the normal times
//! the vertex colour, a fog factor from the eye per vertex, a material's alpha test and its alpha or
//! additive blend. Where this differs from the rasteriser it is on purpose and by eye (AGENT.md:
//! rendering is ours, never 1:1): textures are filtered and mip-mapped rather than nearest, the ground
//! is drawn out to the fog rather than to a radius round the rider, opaque materials are drawn before
//! translucent ones, and the sky is filtered.
//!
//! It asks for WebGL2's limits and keeps to what WebGL2 has — uniform buffers, textures, no storage
//! buffers, no compute, no instance offsets — so the same code can draw in a browser through WebGPU
//! or WebGL2.
//!
//! The HUD and the menus are still drawn by the CPU into frames ([`crate::raster::Frame`]) and laid
//! over the picture as an overlay ([`Patch`]): they are few pixels and cheap.

pub use wgpu;

use crate::mat::{M4, V3, v3};
use crate::psg::Model;
use crate::pst::Texture;
use crate::scene::{Scene, Shading};
use crate::sky::Sky;
use crate::splat::Splat;
use crate::terrain::{lod_balance, lod_coarser, lod_errors, lod_indices, lod_indices_where, lod_level, lod_levels};
use crate::world::{Assets, Camera, Fog, Graphics, Lighting, Surface};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// The format of the depth buffer.
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The format an offscreen frame is drawn in: plain bytes, as the rasteriser's are.
pub const OFFSCREEN: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// How many pixels a chunk of ground's height may be out by, at the level of detail it is drawn at.
const GROUND_PIXELS: f32 = 0.75;
/// How few pixels across a prop may be before it is not drawn at all: less than one can show.
const PROP_PIXELS: f32 = 0.5;
/// Node matrices a palette row holds (four texels each).
const PALETTE_ROW: u32 = 256;
/// A view's uniforms, padded to the alignment a dynamic offset needs.
const VIEW_STRIDE: u64 = 256;

/// A vertex of a model as its `.psg` strip has it: in its node's space, and which node that is.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    color: [u8; 4],
    node: u32,
}

/// A vertex of the ground: where it is in the world, and the stored normal.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default, PartialEq)]
pub struct GroundVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
}

/// One drawn copy of a model: where it stands, where its palette starts and how long it is.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceRaw {
    m: [[f32; 4]; 4],
    palette: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ViewUniform {
    vp: [[f32; 4]; 4],
    eye: [f32; 4],
    sun: [f32; 4],
    sun_color: [f32; 4],
    ambient: [f32; 4],
    fog_color: [f32; 4],
    /// near, far, whether there is a sky, the most fog anything gets ([`Fog`]).
    fog: [f32; 4],
    /// The sky's ray: forward, and the view's right and up scaled by its half extents.
    forward: [f32; 4],
    right: [f32; 4],
    up: [f32; 4],
    _pad: [f32; 12],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    constant: [f32; 4],
    cutoff: f32,
    shading: u32,
    _pad: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PlaceUniform {
    /// Where the image's top left lands in the target, and how many target pixels one of its own is.
    origin: [f32; 2],
    scale: f32,
    _pad: f32,
}

/// How a material lays its pixels over the frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Blend {
    Opaque,
    Alpha,
    Add,
}

/// What picks a pipeline for a material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PipelineKey {
    blend: Blend,
    depth_write: bool,
    /// Drawn see-through at the pass's blend constant, whatever its own blend ([`Actor::alpha`]).
    ghost: bool,
}

/// What makes two materials one material on the GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MaterialKey {
    texture: Option<usize>,
    shading: (u8, [u32; 4]),
    cutoff: u8,
    pipeline: PipelineKey,
    /// Clamped at the texture's edges along u and v rather than repeated.
    clamp: [bool; 2],
}

impl MaterialKey {
    fn of(texture: Option<usize>, s: &Surface) -> MaterialKey {
        let shading = match s.shading {
            Shading::Diffuse => (0, [0; 4]),
            Shading::Prelit => (1, [0; 4]),
            Shading::Constant(c) => (2, c.map(f32::to_bits)),
        };
        let blend = match (s.blend, s.additive) {
            (false, _) => Blend::Opaque,
            (true, false) => Blend::Alpha,
            (true, true) => Blend::Add,
        };
        MaterialKey {
            texture,
            shading,
            cutoff: s.cutoff,
            pipeline: PipelineKey {
                blend,
                depth_write: s.depth_write,
                ghost: false,
            },
            clamp: s.clamp,
        }
    }
}

struct GpuMaterial {
    bind: wgpu::BindGroup,
    pipeline: PipelineKey,
}

/// A run of one model's vertices in one material.
#[derive(Clone, Copy, Debug)]
struct Range {
    material: usize,
    start: u32,
    count: u32,
    blended: bool,
}

struct GpuModel {
    vertices: wgpu::Buffer,
    /// Per level of detail, its runs, the opaque ones first.
    groups: Vec<Vec<Range>>,
    /// The distances its levels of detail switch at, as the file has them ([`Model::group_for`]).
    group_distance: Vec<f32>,
    /// Where the bind pose's palette starts, and how many nodes it has.
    bind_palette: u32,
    nodes: u32,
    radius: f32,
}

impl GpuModel {
    fn group(&self, g: usize) -> &[Range] {
        match self.groups.get(g) {
            Some(r) => r,
            None => self.groups.last().map_or(&[], |r| r),
        }
    }
}

/// A square patch of the ground, `cells` cells a side: its corners, with a sphere to cull it by.
///
/// Every chunk of a ground is the same size, a power of two, and has all `(cells + 1)²` corners, row
/// by row along `x`; one at the map's edge repeats the edge's corners past it, which makes empty
/// triangles rather than a different shape. The GPU picks how many of them to draw
/// ([`crate::terrain::lod_indices`]).
#[derive(Clone, Debug, Default)]
pub struct GroundChunk {
    pub vertices: Vec<GroundVertex>,
    pub cells: u32,
    /// Which chunk this is, across and along.
    pub at: (u32, u32),
    /// The cells that have no ground, row by row; empty when there are none. A chunk with holes is
    /// always drawn whole.
    pub missing: Vec<bool>,
    pub centre: V3,
    pub radius: f32,
}

struct GpuChunk {
    /// Where its corners start in the vertex buffer.
    base: u32,
    centre: V3,
    radius: f32,
    /// Its height error at each level ([`lod_errors`]).
    errors: Vec<f32>,
    /// A chunk with holes: its whole grid, stitched each of the sixteen ways.
    own: Option<Vec<Vec<u32>>>,
}

struct GpuGround {
    vertices: wgpu::Buffer,
    chunks: Vec<GpuChunk>,
    cells: u32,
    across: usize,
    along: usize,
    /// The grid of chunks: which one is at each place, if any.
    slots: Vec<Option<usize>>,
    /// Every level's triangles, stitched each of the sixteen ways: `level * 16 + coarser sides`.
    patterns: Vec<Vec<u32>>,
    /// Each view's triangles, as last built, and what they were built from.
    views: Vec<GroundView>,
    levels: Vec<u32>,
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
}

/// One view's ground: what it drew last frame, kept until the chunks it sees or their levels change.
struct GroundView {
    buffer: wgpu::Buffer,
    capacity: u64,
    /// For each chunk, the pattern it was drawn with, or `u32::MAX` for not drawn.
    key: Vec<u32>,
    count: u32,
    indices: Vec<u32>,
}

/// Something to draw in a view besides the scene's own props: a bike, its parts, a rider, the plane.
#[derive(Clone, Copy, Debug)]
pub struct Actor<'a> {
    /// The model's name in [`Assets::models`].
    pub model: &'a str,
    /// Model space to world space.
    pub at: M4,
    /// Which of [`Scene3::poses`] it is posed by, or `None` for its bind pose.
    pub pose: Option<usize>,
    /// Where to measure the camera's distance from for its level of detail ([`Model::group_for`]);
    /// `None` draws the finest, as the props are drawn.
    pub lod_from: Option<V3>,
    /// 1 is solid. Below it the actor is drawn see-through, every material of it at this alpha over
    /// what is behind it, after everything solid and translucent in its view (a bike's put-back
    /// ghost); 0 or less is not drawn.
    pub alpha: f32,
}

/// One view of the frame: where in the frame it goes, and the camera it is drawn from.
pub struct View<'a> {
    /// x, y, width, height, in the frame's pixels.
    pub rect: [u32; 4],
    pub camera: &'a Camera,
    /// What it draws besides the scene; one list may serve every view.
    pub actors: &'a [Actor<'a>],
}

/// A piece of the overlay: over `rect` of the frame, each pixel becomes `under * mul / 255 + add`.
///
/// The HUD and the menus are drawn by the CPU, and everything they do to a pixel — fill it, darken
/// it, mix a colour over it by an alpha, add light to it — is a multiply and an add, so a layer of them
/// is two images: the layer drawn over black (`add`), and over white less over black (`mul`). RGB
/// rows, `rect`'s width across.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
    pub rect: [u32; 4],
    pub mul: Vec<u8>,
    pub add: Vec<u8>,
}

impl Patch {
    /// The patch a layer makes, from the layer drawn over a black frame and over a white one, both
    /// `width` by `height` and placed at `at` in the frame; `None` where it touched no pixel. Only the
    /// rectangle it touched is kept.
    pub fn of(black: &[u8], white: &[u8], width: usize, height: usize, at: (u32, u32)) -> Option<Patch> {
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for y in 0..height {
            let row = y * width * 3;
            let (b, w) = (&black[row..row + width * 3], &white[row..row + width * 3]);
            if b.iter().all(|&c| c == 0) && w.iter().all(|&c| c == 255) {
                continue;
            }
            let first = (0..width).find(|&x| b[x * 3..x * 3 + 3] != [0; 3] || w[x * 3..x * 3 + 3] != [255; 3]);
            let last = (0..width).rev().find(|&x| b[x * 3..x * 3 + 3] != [0; 3] || w[x * 3..x * 3 + 3] != [255; 3]);
            if let (Some(f), Some(l)) = (first, last) {
                (x0, x1, y0, y1) = (x0.min(f), x1.max(l + 1), y0.min(y), y + 1);
            }
        }
        if x0 >= x1 {
            return None;
        }
        let (w, h) = (x1 - x0, y1 - y0);
        let mut mul = Vec::with_capacity(w * h * 3);
        let mut add = Vec::with_capacity(w * h * 3);
        for y in y0..y1 {
            let row = (y * width + x0) * 3;
            add.extend_from_slice(&black[row..row + w * 3]);
            mul.extend(white[row..row + w * 3].iter().zip(&black[row..row + w * 3]).map(|(&w, &b)| w.saturating_sub(b)));
        }
        Some(Patch {
            rect: [at.0 + x0 as u32, at.1 + y0 as u32, w as u32, h as u32],
            mul,
            add,
        })
    }
}

/// What one frame is made of.
pub struct Scene3<'a> {
    pub views: &'a [View<'a>],
    /// The node matrices of each posed actor, as skinning reads them (`skin::skin`'s `world`).
    pub poses: &'a [Vec<M4>],
    pub overlay: &'a [Patch],
    pub light: &'a Lighting,
}

/// How much a frame drew.
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    /// Triangles sent to the GPU, per view.
    pub triangles: Vec<usize>,
    pub draws: usize,
    /// Ground chunks drawn, at each level of detail, over every view.
    pub ground_levels: Vec<usize>,
    /// Props drawn, over every view.
    pub props: usize,
}

/// Where a frame goes: a texture view, how big that texture is, and where in it the frame sits.
pub struct Target<'a> {
    pub view: &'a wgpu::TextureView,
    pub size: (u32, u32),
    /// x, y, width, height of the frame inside the texture, and how many texture pixels one of the
    /// frame's is (for the overlay, which is drawn at the frame's size).
    pub frame: [u32; 4],
    pub scale: u32,
}

/// The renderer: a device, what is on it, and how to draw it.
pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    /// The adapter's name and backend, for a line at start-up.
    pub adapter: String,
    mesh_module: wgpu::ShaderModule,
    view_layout: wgpu::BindGroupLayout,
    material_layout: wgpu::BindGroupLayout,
    mesh_layout: wgpu::PipelineLayout,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    sky_pipeline: wgpu::RenderPipeline,
    sky_layout: wgpu::BindGroupLayout,
    sky_bind: wgpu::BindGroup,
    has_sky: bool,
    place_layout: wgpu::BindGroupLayout,
    overlay_mul: wgpu::RenderPipeline,
    overlay_add: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    clamp_sampler: wgpu::Sampler,
    white: wgpu::TextureView,
    textures: HashMap<usize, wgpu::TextureView>,
    materials: Vec<GpuMaterial>,
    material_ids: HashMap<MaterialKey, usize>,
    models: Vec<GpuModel>,
    model_ids: HashMap<String, Option<usize>>,
    /// The bind poses, as uploaded, one matrix after another; entry 0 is the identity.
    statics: Vec<[f32; 16]>,
    statics_written: usize,
    palette: wgpu::Texture,
    palette_rows: u32,
    dynamic: Vec<[f32; 16]>,
    view_buffer: wgpu::Buffer,
    view_capacity: u64,
    view_bind: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instance_capacity: u64,
    depth: Option<(wgpu::TextureView, (u32, u32))>,
    overlay: Option<Overlay>,
    ground: Option<GpuGround>,
    props: Vec<Prop>,
    /// How far the world is drawn (V32), and how far apart the ground's two farthest points are.
    graphics: Graphics,
    span: f32,
    /// The frame's instances and batches, kept to reuse their allocations.
    /// Each copy's batch key, its see-through alpha's bits (0 for solid) and the copy.
    scratch: Vec<(u64, u32, InstanceRaw)>,
    batches: Vec<Batch>,
    flat: Vec<InstanceRaw>,
    blit: Option<Blit>,
}

/// A prop the scene places, as culling needs it.
struct Prop {
    name: String,
    model: Option<Option<usize>>,
    at: M4,
}

/// A run of copies of one model at one level of detail, in one view.
#[derive(Clone, Copy)]
struct Batch {
    view: usize,
    model: usize,
    group: usize,
    /// The see-through alpha's bits, 0 for a solid batch.
    ghost: u32,
    first: u32,
    count: u32,
}

struct Overlay {
    size: (u32, u32),
    mul: wgpu::Texture,
    add: wgpu::Texture,
    place: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// What was written last frame, to be put back to "no overlay" first.
    last: Vec<[u32; 4]>,
}

struct Blit {
    size: (u32, u32),
    texture: wgpu::Texture,
    place: wgpu::Buffer,
    bind: wgpu::BindGroup,
    rgba: Vec<u8>,
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32, filterable: bool, dimension: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: dimension,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn depth_state(write: bool, compare: wgpu::CompareFunction) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: Default::default(),
        bias: Default::default(),
    }
}

/// A matrix as four texels, its rows in order.
fn flat(m: M4) -> [f32; 16] {
    let r = m.0;
    [
        r[0][0], r[0][1], r[0][2], r[0][3], r[1][0], r[1][1], r[1][2], r[1][3], r[2][0], r[2][1], r[2][2], r[2][3],
        r[3][0], r[3][1], r[3][2], r[3][3],
    ]
}

/// A texture's mip chain, each level half the last, by a box filter.
fn mips(t: &Texture) -> Vec<(u32, u32, Vec<u8>)> {
    let mut out = vec![(t.width as u32, t.height as u32, t.rgba.clone())];
    while let Some((w, h, px)) = out.last()
        && (*w > 1 || *h > 1)
    {
        let (w, h) = (*w as usize, *h as usize);
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            for x in 0..nw {
                for k in 0..4 {
                    let at = |xx: usize, yy: usize| px[(yy.min(h - 1) * w + xx.min(w - 1)) * 4 + k] as u32;
                    let s = at(2 * x, 2 * y) + at(2 * x + 1, 2 * y) + at(2 * x, 2 * y + 1) + at(2 * x + 1, 2 * y + 1);
                    next[(y * nw + x) * 4 + k] = ((s + 2) / 4) as u8;
                }
            }
        }
        out.push((nw as u32, nh as u32, next));
    }
    out
}

fn upload_texture(device: &wgpu::Device, queue: &wgpu::Queue, t: &Texture) -> wgpu::TextureView {
    let fallback;
    let t = match t.width == 0 || t.height == 0 || t.rgba.len() < t.width * t.height * 4 {
        true => {
            fallback = Texture {
                width: 1,
                height: 1,
                rgba: vec![255, 0, 255, 255],
            };
            &fallback
        }
        false => t,
    };
    let levels = mips(t);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: t.width as u32,
            height: t.height as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, (w, h, px)) in levels.iter().enumerate() {
        write_layer(queue, &texture, level as u32, 0, (*w, *h), 4, px);
    }
    texture.create_view(&Default::default())
}

fn write_layer(queue: &wgpu::Queue, texture: &wgpu::Texture, level: u32, layer: u32, size: (u32, u32), texel: u32, px: &[u8]) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: level,
            origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
            aspect: wgpu::TextureAspect::All,
        },
        px,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.0 * texel),
            rows_per_image: Some(size.1),
        },
        wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        },
    );
}

fn palette_texture(device: &wgpu::Device, rows: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("palette"),
        size: wgpu::Extent3d {
            width: PALETTE_ROW * 4,
            height: rows,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn view_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    palette: &wgpu::Texture,
) -> wgpu::BindGroup {
    let palette = palette.create_view(&Default::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("view"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ViewUniform>() as u64),
                }),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&palette),
            },
        ],
    })
}

/// The sky's six faces as one array texture; a black one-texel cube where there is no sky.
fn sky_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    sky: Option<&Sky>,
) -> wgpu::BindGroup {
    let (w, h) = sky
        .and_then(|s| s.faces.first())
        .map_or((1, 1), |f| (f.width.max(1) as u32, f.height.max(1) as u32));
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sky"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for face in 0..6u32 {
        // A face of another size is resampled to the first's, nearest; the disc's are all one size.
        let px: Vec<u8> = match sky.and_then(|s| s.faces.get(face as usize)) {
            Some(f) if f.width > 0 && f.height > 0 => (0..h as usize)
                .flat_map(|y| {
                    (0..w as usize).flat_map(move |x| {
                        let (sx, sy) = (x * f.width / w as usize, y * f.height / h as usize);
                        let at = (sy * f.width + sx) * 4;
                        [f.rgba[at], f.rgba[at + 1], f.rgba[at + 2], 255]
                    })
                })
                .collect(),
            _ => vec![0; (w * h * 4) as usize],
        };
        write_layer(queue, &texture, 0, face, (w, h), 4, &px);
    }
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sky"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// The baked light map's tiles as one grey image, for the ground's shader.
fn upload_shadow(device: &wgpu::Device, queue: &wgpu::Queue, s: &crate::splat::Shadow) -> wgpu::TextureView {
    let across = s.across.max(1);
    let rows = s.tiles.len() / across;
    let (tw, th) = s.tiles.first().map_or((1, 1), |t| (t.width.max(1), t.height.max(1)));
    let (w, h) = (tw * across, th * rows.max(1));
    let mut px = vec![128u8; w * h];
    for (i, tile) in s.tiles.iter().enumerate().take(across * rows) {
        let (ox, oy) = ((i % across) * tw, (i / across) * th);
        if tile.width == 0 || tile.height == 0 {
            continue;
        }
        for y in 0..th {
            for x in 0..tw {
                let (sx, sy) = (x * tile.width / tw, y * tile.height / th);
                px[(oy + y) * w + ox + x] = tile.rgba[(sy * tile.width + sx) * 4];
            }
        }
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("light map"),
        size: wgpu::Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_layer(queue, &texture, 0, 0, (w as u32, h as u32), 1, &px);
    texture.create_view(&Default::default())
}

/// A model's strips as the GPU draws them: three vertices a triangle, by the same ADC rule as
/// [`Model::triangles`], gathered per level of detail and per material, opaque runs first.
fn strips_of(model: &Model, material_of: &[usize], blended: impl Fn(usize) -> bool) -> (Vec<MeshVertex>, Vec<Vec<Range>>) {
    let groups = model.meshes.iter().map(|m| m.group + 1).max().unwrap_or(0).max(model.groups).max(1);
    let mut vertices = Vec::new();
    let mut out = Vec::with_capacity(groups);
    for g in 0..groups {
        let mut runs: Vec<(bool, usize, usize)> = model
            .meshes
            .iter()
            .enumerate()
            .filter(|(_, m)| m.group == g)
            .map(|(i, m)| {
                let material = material_of.get(m.material).copied().unwrap_or(0);
                (blended(material), material, i)
            })
            .collect();
        runs.sort();
        let mut ranges: Vec<Range> = Vec::new();
        for (blend, material, mesh) in runs {
            let start = vertices.len() as u32;
            for strip in &model.meshes[mesh].strips {
                for i in 2..strip.verts.len() {
                    if strip.verts[i].adc {
                        continue;
                    }
                    for v in &strip.verts[i - 2..=i] {
                        vertices.push(MeshVertex {
                            pos: [v.pos.x, v.pos.y, v.pos.z],
                            normal: [v.normal.x, v.normal.y, v.normal.z],
                            uv: v.uv,
                            color: v.color,
                            node: v.node.min(u32::MAX as usize) as u32,
                        });
                    }
                }
            }
            let count = vertices.len() as u32 - start;
            match ranges.last_mut() {
                Some(r) if r.material == material && r.start + r.count == start => r.count += count,
                _ if count > 0 => ranges.push(Range {
                    material,
                    start,
                    count,
                    blended: blend,
                }),
                _ => {}
            }
        }
        out.push(ranges);
    }
    (vertices, out)
}

/// A four-bit mask of a chunk's coarser sides as [`lod_indices`] wants them.
fn sides(mask: usize) -> [bool; 4] {
    [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0]
}

/// World to clip for a view of this shape, with its far plane at `far`, depth running 0 to 1 from
/// near to far.
fn view_projection(camera: &Camera, width: u32, height: u32, far: f32) -> M4 {
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    M4::look_at(camera.eye, camera.at, v3(0.0, 1.0, 0.0)).mul(M4::perspective(camera.fov, aspect, camera.near, far.max(camera.near * 2.0)))
}

/// [`view_projection`] as the GPU draws it: depth reversed, 1 at the near plane and 0 at the far one.
fn reversed(vp: M4) -> M4 {
    let mut m = vp.0;
    for row in &mut m {
        row[2] = row[3] - row[2];
    }
    M4(m)
}

/// What one view draws (V32): what its frustum holds, no further than its reach, and nothing too
/// small to make a pixel.
pub struct Cull {
    planes: [[f32; 4]; 6],
    eye: V3,
    reach: f32,
    focal: f32,
}

impl Cull {
    /// The cull of `camera` drawn `width` by `height` pixels under `graphics`, for a map `span` feet
    /// across and a scene lit (and fogged) by `light`.
    pub fn of(camera: &Camera, width: u32, height: u32, graphics: Graphics, light: &Lighting, span: f32) -> Cull {
        let vp = view_projection(camera, width, height, graphics.far_clip(light, span));
        Cull {
            planes: frustum(&vp),
            eye: camera.eye,
            reach: graphics.fog(light, span).reach,
            focal: camera.focal(height.max(1) as usize),
        }
    }

    /// Whether a sphere is in view: inside the frustum and not past the reach.
    pub fn sees(&self, centre: V3, radius: f32) -> bool {
        centre.sub(self.eye).len() - radius <= self.reach && sees(&self.planes, centre, radius)
    }

    /// How far the camera is from a sphere's surface; 0 inside it.
    pub fn distance(&self, centre: V3, radius: f32) -> f32 {
        (centre.sub(self.eye).len() - radius).max(0.0)
    }

    /// Whether a sphere is at least `pixels` across on screen.
    pub fn big_enough(&self, centre: V3, radius: f32, pixels: f32) -> bool {
        2.0 * radius * self.focal >= pixels * centre.sub(self.eye).len()
    }
}

/// The frustum's planes out of a row-vector world-to-clip matrix: left, right, bottom, top, near, far,
/// as `(normal, offset)` with the inside where `normal . p + offset >= 0`.
fn frustum(vp: &M4) -> [[f32; 4]; 6] {
    let m = vp.0;
    let col = |k: usize| [m[0][k], m[1][k], m[2][k], m[3][k]];
    let (x, y, z, w) = (col(0), col(1), col(2), col(3));
    let plane = |a: [f32; 4], s: f32, b: [f32; 4]| {
        let p = [a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2], a[3] + s * b[3]];
        let n = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-12);
        [p[0] / n, p[1] / n, p[2] / n, p[3] / n]
    };
    [plane(w, 1.0, x), plane(w, -1.0, x), plane(w, 1.0, y), plane(w, -1.0, y), plane(z, 0.0, z), plane(w, -1.0, z)]
}

fn sees(planes: &[[f32; 4]; 6], c: V3, r: f32) -> bool {
    planes.iter().all(|p| p[0] * c.x + p[1] * c.y + p[2] * c.z + p[3] >= -r)
}

impl Renderer {
    /// A renderer with no window, drawing offscreen in [`OFFSCREEN`]: for the tests and `--headless`.
    /// Fails with the reason where there is no adapter to draw with.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn headless() -> Result<Renderer, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no GPU adapter: {e}"))?;
        Renderer::new(&adapter, OFFSCREEN)
    }

    /// A renderer on `adapter`, drawing into textures of `format`: [`Renderer::request`], waited for.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> Result<Renderer, String> {
        pollster::block_on(Renderer::request(adapter, format))
    }

    /// A renderer on `adapter`, drawing into textures of `format`, once the device is granted. The
    /// one wait in making a renderer; a browser cannot block on it, so it is a future its page drives
    /// (P48), where the desktop waits on it in [`Renderer::new`].
    pub async fn request(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> Result<Renderer, String> {
        let info = adapter.get_info();
        // WebGL2's limits, raised to the size of texture this adapter can take: what is drawn here can
        // be drawn in a browser.
        let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("openmx"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|e| format!("{}: no device: {e}", info.name))?;
        let adapter_name = format!("{} ({:?})", info.name, info.backend);

        let mesh_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mesh"),
            source: wgpu::ShaderSource::Wgsl(MESH_WGSL.into()),
        });
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("view"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<ViewUniform>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                texture_entry(1, true, wgpu::TextureViewDimension::D2),
                sampler_entry(2),
            ],
        });
        let mesh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh"),
            bind_group_layouts: &[Some(&view_layout), Some(&material_layout)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("repeat"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let clamp_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = upload_texture(
            &device,
            &queue,
            &Texture {
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            },
        );

        // The sky: six faces as one array texture, and a triangle over the view that looks them up.
        let sky_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky"),
            entries: &[texture_entry(0, true, wgpu::TextureViewDimension::D2Array), sampler_entry(1)],
        });
        let sky_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(SKY_WGSL.into()),
        });
        let sky_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky"),
            bind_group_layouts: &[Some(&view_layout), Some(&sky_layout)],
            immediate_size: 0,
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&sky_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sky_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(false, wgpu::CompareFunction::Always)),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &sky_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sky_bind = sky_bind_group(&device, &queue, &sky_layout, &clamp_sampler, None);

        // The overlay and the blit: images read texel for texel and laid over the target.
        let place_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("place"),
            entries: &[
                texture_entry(0, false, wgpu::TextureViewDimension::D2),
                texture_entry(1, false, wgpu::TextureViewDimension::D2),
                uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let place_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("place"),
            source: wgpu::ShaderSource::Wgsl(PLACE_WGSL.into()),
        });
        let place_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("place"),
            bind_group_layouts: &[Some(&place_layout)],
            immediate_size: 0,
        });
        let place_pipeline = |entry: &str, blend: Option<wgpu::BlendState>, depth: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&place_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &place_module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: depth.then(|| depth_state(false, wgpu::CompareFunction::Always)),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &place_module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let multiply = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::Src,
            operation: wgpu::BlendOperation::Add,
        };
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let overlay_mul = place_pipeline(
            "fs_mul",
            Some(wgpu::BlendState {
                color: multiply,
                alpha: multiply,
            }),
            true,
        );
        let overlay_add = place_pipeline("fs_add", Some(wgpu::BlendState { color: add, alpha: add }), true);
        let blit_pipeline = place_pipeline("fs_blit", None, false);

        let palette_rows = 16;
        let palette = palette_texture(&device, palette_rows);
        let view_capacity = 4;
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("views"),
            size: VIEW_STRIDE * view_capacity,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view_bind = view_bind_group(&device, &view_layout, &view_buffer, &palette);
        let instance_capacity = 4096;
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: instance_capacity * std::mem::size_of::<InstanceRaw>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Renderer {
            device,
            queue,
            format,
            adapter: adapter_name,
            mesh_module,
            view_layout,
            material_layout,
            mesh_layout,
            pipelines: HashMap::new(),
            sky_pipeline,
            sky_layout,
            sky_bind,
            has_sky: false,
            place_layout,
            overlay_mul,
            overlay_add,
            blit_pipeline,
            sampler,
            clamp_sampler,
            white,
            textures: HashMap::new(),
            materials: Vec::new(),
            material_ids: HashMap::new(),
            models: Vec::new(),
            model_ids: HashMap::new(),
            // Entry 0 is the identity: what places a vertex whose node a pose does not reach, as
            // `skin::skin` leaves such a vertex where it is.
            statics: vec![flat(M4::IDENTITY)],
            statics_written: 0,
            palette,
            palette_rows,
            dynamic: Vec::new(),
            view_buffer,
            view_capacity,
            view_bind,
            instances,
            instance_capacity,
            depth: None,
            overlay: None,
            ground: None,
            props: Vec::new(),
            graphics: Graphics::default(),
            span: 0.0,
            scratch: Vec::new(),
            batches: Vec::new(),
            flat: Vec::new(),
            blit: None,
        })
    }

    /// The scene's props, drawn in every frame from now on.
    pub fn set_scene(&mut self, scene: &Scene) {
        self.props = scene
            .models
            .iter()
            .map(|p| Prop {
                name: p.model.clone(),
                model: None,
                at: p.transform(),
            })
            .collect();
    }

    /// The sky behind every view, or none: the fog's colour instead.
    pub fn set_sky(&mut self, sky: Option<&Sky>) {
        self.has_sky = sky.is_some_and(|s| s.faces.len() >= 6);
        let sky = sky.filter(|_| self.has_sky);
        self.sky_bind = sky_bind_group(&self.device, &self.queue, &self.sky_layout, &self.clamp_sampler, sky);
    }

    /// The ground: its chunks, and the track's paint for it — or, without paint, the one colour `dirt`
    /// (0..255) lit by the sun.
    ///
    /// Every chunk must be the same size, a power of two, with all its corners ([`GroundChunk`]).
    pub fn set_ground(&mut self, chunks: &[GroundChunk], paint: Option<&Splat>, dirt: [f32; 3]) -> Result<(), String> {
        let cells = chunks.first().map_or(1, |c| c.cells);
        if !cells.is_power_of_two() {
            return Err(format!("ground chunks of {cells} cells: not a power of two"));
        }
        let corners = ((cells + 1) * (cells + 1)) as usize;
        let mut vertices: Vec<GroundVertex> = Vec::with_capacity(chunks.len() * corners);
        let mut gpu_chunks = Vec::with_capacity(chunks.len());
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for c in chunks {
            if c.cells != cells || c.vertices.len() != corners {
                return Err(format!("a ground chunk of {} cells and {} corners among chunks of {cells}", c.cells, c.vertices.len()));
            }
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&c.vertices);
            for v in &c.vertices {
                lo = v3(lo.x.min(v.pos[0]), lo.y.min(v.pos[1]), lo.z.min(v.pos[2]));
                hi = v3(hi.x.max(v.pos[0]), hi.y.max(v.pos[1]), hi.z.max(v.pos[2]));
            }
            let heights: Vec<f32> = c.vertices.iter().map(|v| v.pos[1]).collect();
            let own = c.missing.iter().any(|&m| m).then(|| {
                (0..16)
                    .map(|mask| {
                        let row = cells as usize;
                        lod_indices_where(cells, 0, sides(mask), |i, j| !c.missing[j as usize * row + i as usize])
                    })
                    .collect()
            });
            gpu_chunks.push(GpuChunk {
                base,
                centre: c.centre,
                radius: c.radius,
                errors: lod_errors(&heights, cells),
                own,
            });
        }
        if gpu_chunks.is_empty() {
            self.ground = None;
            return Ok(());
        }
        self.span = if lo.x <= hi.x { hi.sub(lo).len() } else { 0.0 };
        let across = chunks.iter().map(|c| c.at.0 as usize + 1).max().unwrap_or(0);
        let along = chunks.iter().map(|c| c.at.1 as usize + 1).max().unwrap_or(0);
        let mut slots = vec![None; across * along];
        for (i, c) in chunks.iter().enumerate() {
            slots[c.at.1 as usize * across + c.at.0 as usize] = Some(i);
        }
        let patterns: Vec<Vec<u32>> = (0..=lod_levels(cells))
            .flat_map(|level| (0..16).map(move |mask| lod_indices(cells, level, sides(mask))))
            .collect();
        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ground vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // The paint is a little program (`splat.rs`); here it is written out as the shader's own code.
        let layers: &[Texture] = paint.map_or(&[], |p| &p.textures);
        let max = self.device.limits().max_sampled_textures_per_shader_stage as usize;
        if layers.len() + 1 > max {
            return Err(format!(
                "the ground's paint has {} textures and this device samples {max} at once",
                layers.len()
            ));
        }
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ground"),
            source: wgpu::ShaderSource::Wgsl(ground_wgsl(paint, dirt).into()),
        });
        let mut entries = vec![
            sampler_entry(0),
            sampler_entry(1),
            texture_entry(2, true, wgpu::TextureViewDimension::D2),
        ];
        for i in 0..layers.len() {
            entries.push(texture_entry(3 + i as u32, true, wgpu::TextureViewDimension::D2));
        }
        let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ground"),
            entries: &entries,
        });
        let shadow = match paint.and_then(|p| p.shadow.as_ref()) {
            Some(s) => upload_shadow(&self.device, &self.queue, s),
            None => self.white.clone(),
        };
        let views: Vec<wgpu::TextureView> = layers.iter().map(|t| upload_texture(&self.device, &self.queue, t)).collect();
        let mut bind_entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&self.clamp_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&shadow),
            },
        ];
        for (i, v) in views.iter().enumerate() {
            bind_entries.push(wgpu::BindGroupEntry {
                binding: 3 + i as u32,
                resource: wgpu::BindingResource::TextureView(v),
            });
        }
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ground"),
            layout: &layout,
            entries: &bind_entries,
        });
        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ground"),
            bind_group_layouts: &[Some(&self.view_layout), Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ground"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GroundVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(true, wgpu::CompareFunction::Greater)),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(self.format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.ground = Some(GpuGround {
            vertices: vertex_buffer,
            levels: vec![0; slots.len()],
            chunks: gpu_chunks,
            cells,
            across,
            along,
            slots,
            patterns,
            views: Vec::new(),
            pipeline,
            bind,
        });
        Ok(())
    }

    /// How far the world is drawn from now on (V32): the original's fog and far clip, or all of it.
    pub fn set_graphics(&mut self, graphics: Graphics) {
        self.graphics = graphics;
    }

    pub fn graphics(&self) -> Graphics {
        self.graphics
    }

    /// The fog every view is drawn with, under the current [`Graphics`], and where its far plane is.
    pub fn fog(&self, light: &Lighting) -> (Fog, f32) {
        (self.graphics.fog(light, self.span), self.graphics.far_clip(light, self.span))
    }

    /// Forget every model, material and texture uploaded so far: a new ride's assets are not the last
    /// one's, though their names may be.
    pub fn forget_assets(&mut self) {
        self.models.clear();
        self.model_ids.clear();
        self.materials.clear();
        self.material_ids.clear();
        self.textures.clear();
        self.statics.truncate(1);
        self.statics_written = 0;
        self.props.clear();
    }

    /// Wait for everything submitted so far to be drawn.
    pub fn finish(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// How many models, materials and textures are on the GPU.
    pub fn uploaded(&self) -> (usize, usize, usize) {
        (self.models.len(), self.materials.len(), self.textures.len())
    }

    /// Upload every model the assets hold now, rather than as the views first see them.
    pub fn upload_all(&mut self, assets: &Assets) {
        let mut names: Vec<&String> = assets.models.keys().collect();
        names.sort();
        for name in names {
            self.model(assets, name);
        }
    }

    /// A model's index on the GPU, uploading it the first time it is asked for.
    fn model(&mut self, assets: &Assets, name: &str) -> Option<usize> {
        if let Some(&id) = self.model_ids.get(name) {
            return id;
        }
        let id = assets.models.get(name).map(|loaded| {
            let model = &loaded.model;
            let materials: Vec<usize> = (0..model.materials.len().max(1))
                .map(|m| {
                    let texture = loaded.textures.get(m).copied().flatten();
                    let surface = loaded.surfaces.get(m).copied().unwrap_or_default();
                    self.material(assets, texture, &surface)
                })
                .collect();
            let (vertices, groups) = strips_of(model, &materials, |m| self.materials[m].pipeline.blend != Blend::Opaque);
            let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(name),
                contents: bytemuck::cast_slice(if vertices.is_empty() { &[0u8; 4] } else { bytemuck::cast_slice(&vertices) }),
                usage: wgpu::BufferUsages::VERTEX,
            });
            // The bind pose: each node's place in the model, as `Model::triangles` puts its vertices.
            let bind_palette = self.statics.len() as u32;
            self.statics.extend((0..model.nodes.len()).map(|i| flat(model.world(i))));
            self.models.push(GpuModel {
                vertices: buffer,
                groups,
                group_distance: model.group_distance.clone(),
                bind_palette,
                nodes: model.nodes.len() as u32,
                radius: loaded.radius,
            });
            self.models.len() - 1
        });
        self.model_ids.insert(name.to_string(), id);
        id
    }

    fn material(&mut self, assets: &Assets, texture: Option<usize>, surface: &Surface) -> usize {
        let texture = texture.filter(|&t| t < assets.textures.len());
        let key = MaterialKey::of(texture, surface);
        if let Some(&id) = self.material_ids.get(&key) {
            return id;
        }
        let view = match texture {
            Some(t) => self
                .textures
                .entry(t)
                .or_insert_with(|| upload_texture(&self.device, &self.queue, &assets.textures[t]))
                .clone(),
            None => self.white.clone(),
        };
        let constant = match surface.shading {
            Shading::Constant(c) => c,
            _ => [128.0; 4],
        };
        let uniform = MaterialUniform {
            constant,
            cutoff: surface.cutoff as f32,
            shading: key.shading.0 as u32,
            _pad: [0; 2],
        };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("material"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        // The few materials that clamp (palm fronds, bark, cut-outs) get a sampler of their own;
        // everything else shares the repeating one.
        let clamped = (key.clamp != [false; 2]).then(|| {
            let mode = |on: bool| if on { wgpu::AddressMode::ClampToEdge } else { wgpu::AddressMode::Repeat };
            self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("material clamp"),
                address_mode_u: mode(key.clamp[0]),
                address_mode_v: mode(key.clamp[1]),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            })
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(clamped.as_ref().unwrap_or(&self.sampler)),
                },
            ],
        });
        self.pipeline(key.pipeline);
        self.pipeline(PipelineKey { ghost: true, ..key.pipeline });
        self.materials.push(GpuMaterial {
            bind,
            pipeline: key.pipeline,
        });
        let id = self.materials.len() - 1;
        self.material_ids.insert(key, id);
        id
    }

    fn pipeline(&mut self, key: PipelineKey) {
        if self.pipelines.contains_key(&key) {
            return;
        }
        // A ghost lays every material at the pass's blend constant: over what is behind it, or added
        // to it at that share for an additive one.
        let constant = |dst_factor| wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Constant,
                dst_factor,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        let blend = match key.blend {
            _ if key.ghost && key.blend == Blend::Add => Some(constant(wgpu::BlendFactor::One)),
            _ if key.ghost => Some(constant(wgpu::BlendFactor::OneMinusConstant)),
            Blend::Opaque => None,
            Blend::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
            Blend::Add => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }),
        };
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh"),
            layout: Some(&self.mesh_layout),
            vertex: wgpu::VertexState {
                module: &self.mesh_module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Unorm8x4, 4 => Uint32
                        ],
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<InstanceRaw>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Uint32x2
                        ],
                    }),
                ],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(key.depth_write, wgpu::CompareFunction::Greater)),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.mesh_module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.insert(key, pipeline);
    }

    /// A texture to draw a frame into offscreen, and read back with [`Renderer::read`].
    pub fn offscreen(&self, width: u32, height: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// A texture's pixels as RGB rows, the way a [`crate::raster::Frame`] holds them.
    pub fn read(&self, texture: &wgpu::Texture) -> Vec<u8> {
        let (w, h) = (texture.width(), texture.height());
        let row = (w * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("read back"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let Ok(data) = slice.get_mapped_range() else {
            return vec![0; (w * h * 3) as usize];
        };
        let bgr = matches!(texture.format(), wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut out = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h as usize {
            let line = &data[y * row as usize..y * row as usize + w as usize * 4];
            for px in line.chunks_exact(4) {
                match bgr {
                    true => out.extend_from_slice(&[px[2], px[1], px[0]]),
                    false => out.extend_from_slice(&px[..3]),
                }
            }
        }
        out
    }

    fn depth_for(&mut self, size: (u32, u32)) -> wgpu::TextureView {
        if let Some((view, at)) = &self.depth
            && *at == size
        {
            return view.clone();
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.depth = Some((view.clone(), size));
        view
    }

    /// Write the palette: the bind poses that are new since the last frame, and this frame's poses
    /// after them. Answers where the poses start.
    fn write_palette(&mut self, poses: &[Vec<M4>]) -> Vec<u32> {
        let statics_rows = (self.statics.len() as u32).div_ceil(PALETTE_ROW);
        let dynamic_start = statics_rows * PALETTE_ROW;
        self.dynamic.clear();
        let mut starts = Vec::with_capacity(poses.len());
        for pose in poses {
            starts.push(dynamic_start + self.dynamic.len() as u32);
            self.dynamic.extend(pose.iter().map(|m| flat(*m)));
        }
        let rows = statics_rows + (self.dynamic.len() as u32).div_ceil(PALETTE_ROW);
        if rows > self.palette_rows {
            self.palette_rows = rows.next_power_of_two().max(16);
            self.palette = palette_texture(&self.device, self.palette_rows);
            self.view_bind = view_bind_group(&self.device, &self.view_layout, &self.view_buffer, &self.palette);
            self.statics_written = 0;
        }
        let write = |queue: &wgpu::Queue, palette: &wgpu::Texture, first: u32, m: &[[f32; 16]]| {
            // Whole rows at a time, the last padded.
            for (r, chunk) in m.chunks(PALETTE_ROW as usize).enumerate() {
                let mut row = [[0f32; 16]; PALETTE_ROW as usize];
                row[..chunk.len()].copy_from_slice(chunk);
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: palette,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: first / PALETTE_ROW + r as u32,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    bytemuck::cast_slice(&row[..]),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(PALETTE_ROW * 4 * 16),
                        rows_per_image: Some(1),
                    },
                    wgpu::Extent3d {
                        width: PALETTE_ROW * 4,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                );
            }
        };
        if self.statics_written < self.statics.len() {
            // From the start of the row the first new one is in.
            let from = self.statics_written / PALETTE_ROW as usize * PALETTE_ROW as usize;
            write(&self.queue, &self.palette, from as u32, &self.statics[from..]);
            self.statics_written = self.statics.len();
        }
        if !self.dynamic.is_empty() {
            write(&self.queue, &self.palette, dynamic_start, &self.dynamic);
        }
        starts
    }

    /// Draw one frame into `target`: every view, then the overlay. The models the views name are
    /// uploaded from `assets` as they are first seen.
    pub fn render(&mut self, target: &Target, assets: &Assets, frame: &Scene3) -> Drawn {
        let light = frame.light;
        let views = frame.views;
        // Everything a view needs to know before the pass: its uniforms, and its batches.
        if views.len() as u64 > self.view_capacity {
            self.view_capacity = views.len() as u64;
            self.view_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("views"),
                size: VIEW_STRIDE * self.view_capacity,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.view_bind = view_bind_group(&self.device, &self.view_layout, &self.view_buffer, &self.palette);
        }
        // Resolve what the frame names before the palette is written, so a model seen for the first
        // time has its bind pose in this frame's palette.
        for p in 0..self.props.len() {
            if self.props[p].model.is_none() {
                let name = std::mem::take(&mut self.props[p].name);
                self.props[p].model = Some(self.model(assets, &name));
                self.props[p].name = name;
            }
        }
        for view in views {
            for a in view.actors {
                self.model(assets, a.model);
            }
        }
        let pose_starts = self.write_palette(frame.poses);

        let mut uniforms = vec![0u8; (VIEW_STRIDE as usize) * views.len()];
        let mut ground_counts = vec![0u32; views.len()];
        let mut drawn = Drawn {
            triangles: vec![0; views.len()],
            draws: 0,
            ground_levels: Vec::new(),
            props: 0,
        };
        let (fog, far) = self.fog(light);
        self.scratch.clear();
        for (v, view) in views.iter().enumerate() {
            let [_, _, w, h] = view.rect;
            let (w, h) = (w * target.scale, h * target.scale);
            let camera = view.camera;
            let vp = view_projection(camera, w, h, far);
            let cull = Cull::of(camera, w, h, self.graphics, light, self.span);
            // The sky's ray, as `Sky::draw` makes it: the screen's right and up, whichever way the
            // cross products came out.
            let forward = camera.at.sub(camera.eye).norm();
            let mut right = forward.cross(v3(0.0, 1.0, 0.0)).norm();
            let probe = vp.clip(camera.eye.add(forward).add(right.scale(0.1)));
            if probe[0] / probe[3] < 0.0 {
                right = right.scale(-1.0);
            }
            let mut up = right.cross(forward).norm();
            let probe = vp.clip(camera.eye.add(forward).add(up.scale(0.1)));
            if probe[1] / probe[3] < 0.0 {
                up = up.scale(-1.0);
            }
            let half_h = (camera.fov / 2.0).tan();
            let half_w = half_h * w as f32 / h.max(1) as f32;
            let u = ViewUniform {
                vp: reversed(vp).0,
                eye: [camera.eye.x, camera.eye.y, camera.eye.z, 0.0],
                sun: [light.sun.x, light.sun.y, light.sun.z, 0.0],
                sun_color: [light.sun_color[0], light.sun_color[1], light.sun_color[2], 0.0],
                ambient: [light.ambient[0], light.ambient[1], light.ambient[2], 0.0],
                fog_color: [light.fog_color[0] as f32, light.fog_color[1] as f32, light.fog_color[2] as f32, 0.0],
                fog: [fog.near, fog.far, if self.has_sky { 1.0 } else { 0.0 }, fog.max],
                forward: [forward.x, forward.y, forward.z, 0.0],
                right: [right.x * half_w, right.y * half_w, right.z * half_w, 0.0],
                up: [up.x * half_h, up.y * half_h, up.z * half_h, 0.0],
                _pad: [0.0; 12],
            };
            let at = v * VIEW_STRIDE as usize;
            uniforms[at..at + std::mem::size_of::<ViewUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
            let focal = camera.focal(h as usize);
            // The props: culled against the frustum and the reach (the fog's end, as `draw_scene` culls
            // them, or the whole map), and each at the level of detail the disc's own test picks.
            for prop in &self.props {
                let Some(Some(m)) = prop.model else { continue };
                let model = &self.models[m];
                let centre = v3(prop.at.0[3][0], prop.at.0[3][1], prop.at.0[3][2]);
                if !cull.sees(centre, model.radius) || !cull.big_enough(centre, model.radius, PROP_PIXELS) {
                    continue;
                }
                drawn.props += 1;
                let group = crate::psg::group_at(&model.group_distance, centre.sub(camera.eye).len(), model.radius, focal);
                let key = ((v as u64) << 48) | ((m as u64) << 16) | group.min(0xffff) as u64;
                self.scratch.push((
                    key,
                    0,
                    InstanceRaw {
                        m: prop.at.0,
                        palette: [model.bind_palette, model.nodes],
                    },
                ));
            }
            // The actors: few, and each at the level of detail this camera has earned.
            for a in view.actors {
                let Some(Some(m)) = self.model_ids.get(a.model).copied() else { continue };
                if !(a.alpha > 0.0) {
                    continue;
                }
                let ghost = if a.alpha < 1.0 { a.alpha.to_bits() } else { 0 };
                let model = &self.models[m];
                let group = match a.lod_from {
                    Some(at) => {
                        let away = at.sub(camera.eye).len();
                        let loaded = assets.models.get(a.model).map(|l| &l.model);
                        loaded.map_or(0, |l| l.group_for(away, model.radius, focal))
                    }
                    None => 0,
                };
                let palette = match a.pose.and_then(|p| pose_starts.get(p).map(|&s| (s, frame.poses[p].len()))) {
                    Some((start, len)) => [start, len as u32],
                    None => [model.bind_palette, model.nodes],
                };
                let key = ((v as u64) << 48) | ((m as u64) << 16) | group.min(0xffff) as u64;
                self.scratch.push((key, ghost, InstanceRaw { m: a.at.0, palette }));
            }
            ground_counts[v] = self.ground_for(v, &cull, focal, &mut drawn);
        }
        self.queue.write_buffer(&self.view_buffer, 0, &uniforms);

        // Gather the copies into batches, model by model, and upload them as one buffer.
        self.scratch.sort_unstable_by_key(|(k, g, _)| (*k, *g));
        self.batches.clear();
        self.flat.clear();
        for (i, (key, ghost, inst)) in self.scratch.iter().enumerate() {
            let (view, model, group) = ((key >> 48) as usize, ((key >> 16) & 0xffff_ffff) as usize, (key & 0xffff) as usize);
            let ghost = *ghost;
            match self.batches.last_mut() {
                Some(b) if b.view == view && b.model == model && b.group == group && b.ghost == ghost => b.count += 1,
                _ => self.batches.push(Batch {
                    view,
                    model,
                    group,
                    ghost,
                    first: i as u32,
                    count: 1,
                }),
            }
            self.flat.push(*inst);
        }
        if self.flat.len() as u64 > self.instance_capacity {
            self.instance_capacity = (self.flat.len() as u64).next_power_of_two();
            self.instances = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instances"),
                size: self.instance_capacity * std::mem::size_of::<InstanceRaw>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !self.flat.is_empty() {
            self.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&self.flat));
        }
        self.write_overlay(target, frame.overlay);

        let depth = self.depth_for(target.size);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let stride = std::mem::size_of::<InstanceRaw>() as u64;
            let mut b = 0;
            for (v, view) in views.iter().enumerate() {
                let [x, y, w, h] = view.rect;
                let s = target.scale;
                let (x, y, w, h) = (target.frame[0] + x * s, target.frame[1] + y * s, w * s, h * s);
                if w == 0 || h == 0 || x + w > target.size.0 || y + h > target.size.1 {
                    continue;
                }
                pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, w, h);
                let offset = [(v as u64 * VIEW_STRIDE) as u32];
                // The backdrop: the sky, or the fog's colour.
                pass.set_pipeline(&self.sky_pipeline);
                pass.set_bind_group(0, &self.view_bind, &offset);
                pass.set_bind_group(1, &self.sky_bind, &[]);
                pass.draw(0..3, 0..1);
                drawn.draws += 1;
                // The ground: the chunks this view sees, each at its level, in one draw.
                if let Some(ground) = &self.ground
                    && let Some(gv) = ground.views.get(v)
                    && ground_counts[v] > 0
                {
                    pass.set_pipeline(&ground.pipeline);
                    pass.set_bind_group(0, &self.view_bind, &offset);
                    pass.set_bind_group(1, &ground.bind, &[]);
                    pass.set_vertex_buffer(0, ground.vertices.slice(..));
                    pass.set_index_buffer(gv.buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..ground_counts[v], 0, 0..1);
                    drawn.draws += 1;
                    drawn.triangles[v] += (ground_counts[v] / 3) as usize;
                }
                // The models: the opaque runs of every batch, then the translucent ones over them.
                let end = (b..self.batches.len()).find(|&i| self.batches[i].view != v).unwrap_or(self.batches.len());
                let mine = &self.batches[b..end];
                b = end;
                for blended in [false, true] {
                    let mut current: Option<PipelineKey> = None;
                    let mut bound: Option<usize> = None;
                    for batch in mine.iter().filter(|b| b.ghost == 0) {
                        let model = &self.models[batch.model];
                        let ranges = model.group(batch.group);
                        if !ranges.iter().any(|r| r.blended == blended) {
                            continue;
                        }
                        pass.set_vertex_buffer(0, model.vertices.slice(..));
                        // The batch's own copies, from the start of the buffer slice: WebGL2 has no
                        // first instance to draw from.
                        pass.set_vertex_buffer(1, self.instances.slice(batch.first as u64 * stride..(batch.first + batch.count) as u64 * stride));
                        for r in ranges.iter().filter(|r| r.blended == blended) {
                            let material = &self.materials[r.material];
                            if current != Some(material.pipeline) {
                                pass.set_pipeline(&self.pipelines[&material.pipeline]);
                                pass.set_bind_group(0, &self.view_bind, &offset);
                                current = Some(material.pipeline);
                            }
                            if bound != Some(r.material) {
                                pass.set_bind_group(1, &material.bind, &[]);
                                bound = Some(r.material);
                            }
                            pass.draw(r.start..r.start + r.count, 0..batch.count);
                            drawn.draws += 1;
                            drawn.triangles[v] += (r.count / 3 * batch.count) as usize;
                        }
                    }
                }
                // The see-through copies last, over everything else in the view: every run of each,
                // its solid ones first, at its alpha as the blend constant.
                let mut current: Option<PipelineKey> = None;
                let mut bound: Option<usize> = None;
                for batch in mine.iter().filter(|b| b.ghost != 0) {
                    let model = &self.models[batch.model];
                    let alpha = f32::from_bits(batch.ghost) as f64;
                    pass.set_blend_constant(wgpu::Color { r: alpha, g: alpha, b: alpha, a: alpha });
                    pass.set_vertex_buffer(0, model.vertices.slice(..));
                    pass.set_vertex_buffer(1, self.instances.slice(batch.first as u64 * stride..(batch.first + batch.count) as u64 * stride));
                    for r in model.group(batch.group) {
                        let material = &self.materials[r.material];
                        let key = PipelineKey { ghost: true, ..material.pipeline };
                        if current != Some(key) {
                            pass.set_pipeline(&self.pipelines[&key]);
                            pass.set_bind_group(0, &self.view_bind, &offset);
                            current = Some(key);
                        }
                        if bound != Some(r.material) {
                            pass.set_bind_group(1, &material.bind, &[]);
                            bound = Some(r.material);
                        }
                        pass.draw(r.start..r.start + r.count, 0..batch.count);
                        drawn.draws += 1;
                        drawn.triangles[v] += (r.count / 3 * batch.count) as usize;
                    }
                }
            }
            // The overlay, over the whole frame.
            if let Some(overlay) = &self.overlay
                && !frame.overlay.is_empty()
            {
                let [x, y, w, h] = target.frame;
                pass.set_viewport(0.0, 0.0, target.size.0 as f32, target.size.1 as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, w.min(target.size.0 - x), h.min(target.size.1 - y));
                for pipeline in [&self.overlay_mul, &self.overlay_add] {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &overlay.bind, &[]);
                    pass.draw(0..3, 0..1);
                    drawn.draws += 1;
                }
            }
        }
        self.queue.submit([encoder.finish()]);
        drawn
    }

    /// View `v`'s ground for this frame: each chunk's level from this view's camera, balanced so
    /// neighbours differ by one at most, stitched, and the chunks it sees written into the view's own
    /// index buffer — only when something changed since its last frame. How many indices it holds.
    fn ground_for(&mut self, v: usize, cull: &Cull, focal: f32, drawn: &mut Drawn) -> u32 {
        let Some(g) = &mut self.ground else { return 0 };
        let top = lod_levels(g.cells);
        for (slot, level) in g.slots.iter().zip(g.levels.iter_mut()) {
            *level = match slot {
                Some(c) => {
                    let c = &g.chunks[*c];
                    match c.own {
                        Some(_) => 0,
                        None => lod_level(&c.errors, cull.distance(c.centre, c.radius), focal, GROUND_PIXELS),
                    }
                }
                // No chunk here: it holds nobody back.
                None => top + 1,
            };
        }
        lod_balance(&mut g.levels, g.across, g.along);
        for (slot, level) in g.slots.iter().zip(g.levels.iter_mut()) {
            if slot.is_none() {
                *level = 0;
            }
        }
        while g.views.len() <= v {
            g.views.push(GroundView {
                buffer: self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ground indices"),
                    size: 4,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity: 0,
                key: Vec::new(),
                count: 0,
                indices: Vec::new(),
            });
        }
        if drawn.ground_levels.len() <= top as usize {
            drawn.ground_levels.resize(top as usize + 1, 0);
        }
        let mut key = Vec::with_capacity(g.chunks.len());
        for (at, slot) in g.slots.iter().enumerate() {
            let Some(c) = *slot else { continue };
            let chunk = &g.chunks[c];
            if !cull.sees(chunk.centre, chunk.radius) {
                key.push(u32::MAX);
                continue;
            }
            let level = g.levels[at];
            let coarser = lod_coarser(&g.levels, g.across, g.along, at % g.across, at / g.across);
            let mask = coarser.iter().enumerate().map(|(i, &b)| (b as u32) << i).sum::<u32>();
            drawn.ground_levels[level as usize] += 1;
            key.push(level * 16 + mask);
        }
        let gv = &mut g.views[v];
        if key != gv.key {
            gv.indices.clear();
            let chunks = g.slots.iter().flatten();
            for (&c, &k) in chunks.zip(&key) {
                if k == u32::MAX {
                    continue;
                }
                let chunk = &g.chunks[c];
                let pattern = match &chunk.own {
                    Some(own) => &own[(k % 16) as usize],
                    None => &g.patterns[k as usize],
                };
                gv.indices.extend(pattern.iter().map(|i| i + chunk.base));
            }
            let bytes = (gv.indices.len() * 4) as u64;
            if bytes > gv.capacity {
                gv.capacity = bytes.next_power_of_two();
                gv.buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ground indices"),
                    size: gv.capacity,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            if !gv.indices.is_empty() {
                self.queue.write_buffer(&gv.buffer, 0, bytemuck::cast_slice(&gv.indices));
            }
            gv.count = gv.indices.len() as u32;
            gv.key = key;
        }
        gv.count
    }

    /// Put this frame's overlay patches into the overlay textures, after putting last frame's back.
    fn write_overlay(&mut self, target: &Target, patches: &[Patch]) {
        let size = (target.frame[2] / target.scale.max(1), target.frame[3] / target.scale.max(1));
        if patches.is_empty() && self.overlay.as_ref().is_none_or(|o| o.last.is_empty()) {
            return;
        }
        if self.overlay.as_ref().is_none_or(|o| o.size != size) {
            let texture = |label, fill: u8| {
                let t = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0.max(1),
                        height: size.1.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let px = vec![fill; (size.0.max(1) * size.1.max(1) * 4) as usize];
                write_layer(&self.queue, &t, 0, 0, (size.0.max(1), size.1.max(1)), 4, &px);
                t
            };
            let (mul, add) = (texture("overlay mul", 255), texture("overlay add", 0));
            let place = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("overlay place"),
                size: std::mem::size_of::<PlaceUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = self.place_bind(&mul, &add, &place);
            self.overlay = Some(Overlay {
                size,
                mul,
                add,
                place,
                bind,
                last: Vec::new(),
            });
        }
        let overlay = self.overlay.as_mut().expect("made above");
        let place = PlaceUniform {
            origin: [target.frame[0] as f32, target.frame[1] as f32],
            scale: target.scale.max(1) as f32,
            _pad: 0.0,
        };
        self.queue.write_buffer(&overlay.place, 0, bytemuck::bytes_of(&place));
        let clip = |r: [u32; 4]| {
            let (x, y) = (r[0].min(size.0), r[1].min(size.1));
            [x, y, r[2].min(size.0 - x), r[3].min(size.1 - y)]
        };
        let put = |queue: &wgpu::Queue, t: &wgpu::Texture, r: [u32; 4], px: &[u8]| {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: t,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: r[0], y: r[1], z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                px,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(r[2] * 4),
                    rows_per_image: Some(r[3]),
                },
                wgpu::Extent3d {
                    width: r[2],
                    height: r[3],
                    depth_or_array_layers: 1,
                },
            );
        };
        for r in std::mem::take(&mut overlay.last) {
            let r = clip(r);
            if r[2] == 0 || r[3] == 0 {
                continue;
            }
            let n = (r[2] * r[3] * 4) as usize;
            put(&self.queue, &overlay.mul, r, &vec![255; n]);
            put(&self.queue, &overlay.add, r, &vec![0; n]);
        }
        for p in patches {
            let r = clip(p.rect);
            if r[2] == 0 || r[3] == 0 || p.mul.len() < (p.rect[2] * p.rect[3] * 3) as usize {
                continue;
            }
            let rgba = |px: &[u8]| -> Vec<u8> {
                let mut out = Vec::with_capacity((r[2] * r[3] * 4) as usize);
                for y in 0..r[3] as usize {
                    let row = &px[y * p.rect[2] as usize * 3..][..r[2] as usize * 3];
                    for c in row.chunks_exact(3) {
                        out.extend_from_slice(&[c[0], c[1], c[2], 255]);
                    }
                }
                out
            };
            put(&self.queue, &overlay.mul, r, &rgba(&p.mul));
            put(&self.queue, &overlay.add, r, &rgba(&p.add));
            overlay.last.push(r);
        }
    }

    fn place_bind(&self, a: &wgpu::Texture, b: &wgpu::Texture, place: &wgpu::Buffer) -> wgpu::BindGroup {
        let (a, b) = (a.create_view(&Default::default()), b.create_view(&Default::default()));
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("place"),
            layout: &self.place_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&b),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: place.as_entire_binding(),
                },
            ],
        })
    }

    /// Put a frame the CPU drew (RGB rows, `width` by `height`) on `target`, at its frame rectangle
    /// and scale, black round it: the front end's menus, which are drawn in software.
    pub fn blit(&mut self, target: &Target, rgb: &[u8], width: u32, height: u32) {
        if self.blit.as_ref().is_none_or(|b| b.size != (width, height)) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("blit"),
                size: wgpu::Extent3d {
                    width: width.max(1),
                    height: height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let place = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("blit place"),
                size: std::mem::size_of::<PlaceUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind = self.place_bind(&texture, &texture, &place);
            self.blit = Some(Blit {
                size: (width, height),
                texture,
                place,
                bind,
                rgba: Vec::new(),
            });
        }
        let blit = self.blit.as_mut().expect("made above");
        blit.rgba.clear();
        for c in rgb.chunks_exact(3).take((width * height) as usize) {
            blit.rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
        if blit.rgba.len() < (width * height * 4) as usize {
            return;
        }
        write_layer(&self.queue, &blit.texture, 0, 0, (width.max(1), height.max(1)), 4, &blit.rgba);
        let place = PlaceUniform {
            origin: [target.frame[0] as f32, target.frame[1] as f32],
            scale: target.scale.max(1) as f32,
            _pad: 0.0,
        };
        self.queue.write_buffer(&blit.place, 0, bytemuck::bytes_of(&place));
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let [x, y, w, h] = target.frame;
            pass.set_scissor_rect(x, y, w.min(target.size.0 - x.min(target.size.0)), h.min(target.size.1 - y.min(target.size.1)));
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &blit.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
    }
}

/// The ground's shader, with the track's paint program written out pass by pass (`Splat::paint`).
fn ground_wgsl(paint: Option<&Splat>, dirt: [f32; 3]) -> String {
    let mut s = String::from(VIEW_WGSL);
    s.push_str(
        "@group(1) @binding(0) var tiled: sampler;\n\
         @group(1) @binding(1) var clamped: sampler;\n\
         @group(1) @binding(2) var shadow: texture_2d<f32>;\n",
    );
    let layers = paint.map_or(0, |p| p.textures.len());
    for i in 0..layers {
        s.push_str(&format!("@group(1) @binding({}) var layer{i}: texture_2d<f32>;\n", 3 + i));
    }
    s.push_str(
        "struct Out { @builtin(position) clip: vec4f, @location(0) world: vec3f, @location(1) sunlit: vec3f, @location(2) fog: f32 }\n\
         @vertex fn vs(@location(0) pos: vec3f, @location(1) normal: vec3f) -> Out {\n\
           var o: Out;\n\
           o.clip = to_clip(pos);\n\
           o.world = pos;\n\
           let l = length(normal);\n\
           let n = select(normal, normal / l, l > 0.0);\n\
           o.sunlit = view.ambient.rgb + view.sun_color.rgb * max(-dot(n, view.sun.xyz), 0.0);\n\
           o.fog = fog_at(pos);\n\
           return o;\n\
         }\n\
         @fragment fn fs(i: Out) -> @location(0) vec4f {\n\
           let x = i.world.x;\n\
           let z = i.world.z;\n\
           var frame = vec4f(0.0);\n\
           var src = vec4f(0.0);\n\
           var t = vec4f(0.0);\n",
    );
    let f = |v: f32| format!("{v:?}");
    if let Some(p) = paint {
        for pass in &p.passes {
            let (sin, cos) = pass.angle.sin_cos();
            s.push_str(&format!(
                "  t = textureSample(layer{}, tiled, vec2f((x * {c} + z * {sn}) * {su}, (z * {c} - x * {sn}) * {sv})) * 255.0;\n",
                pass.texture,
                c = f(cos),
                sn = f(sin),
                su = f(pass.scale[0]),
                sv = f(pass.scale[1]),
            ));
            s.push_str(&format!(
                "  src = vec4f(t.rgb * vec3f({}, {}, {}) / 128.0, min(t.a * 0.5 * {} / 128.0, 255.0));\n",
                f(pass.rgba[0]),
                f(pass.rgba[1]),
                f(pass.rgba[2]),
                f(pass.rgba[3])
            ));
            match pass.blend {
                None => s.push_str("  frame = src;\n"),
                Some([a, b, c, d]) => {
                    let pick = |w: u8| match w {
                        0 => "src.rgb",
                        1 => "frame.rgb",
                        _ => "vec3f(0.0)",
                    };
                    let alpha = match c {
                        0 => "src.a".to_string(),
                        1 => "frame.a".to_string(),
                        _ => f(pass.fixed),
                    };
                    s.push_str(&format!(
                        "  frame = vec4f(clamp(({} - {}) * {alpha} / 128.0 + {}, vec3f(0.0), vec3f(255.0)), src.a);\n",
                        pick(a),
                        pick(b),
                        pick(d)
                    ));
                }
            }
        }
    }
    let (x0, x1, z0, z1, mapped) = match paint.and_then(|p| p.shadow.as_ref()) {
        Some(sh) => (sh.x0, sh.x1, sh.z0, sh.z1, true),
        None => (0.0, 1.0, 0.0, 1.0, false),
    };
    s.push_str(&format!(
        "  let u = (x - {x0}) / ({x1} - {x0});\n\
           let v = ({z1} - z) / ({z1} - {z0});\n\
           let baked = textureSample(shadow, clamped, vec2f(u, v)).r * 255.0 / 128.0;\n\
           let inside = {mapped} && u >= 0.0 && u < 1.0 && v >= 0.0 && v < 1.0;\n",
        x0 = f(x0),
        x1 = f(x1),
        z0 = f(z0),
        z1 = f(z1),
    ));
    match paint {
        Some(_) => s.push_str(
            "  let painted = floor(clamp(frame.rgb, vec3f(0.0), vec3f(255.0)));\n\
               let texel = select(painted, min(painted * baked, vec3f(255.0)), inside);\n\
               let lit = select(i.sunlit, vec3f(1.0), inside);\n",
        ),
        None => s.push_str(&format!(
            "  let texel = vec3f(255.0);\n\
               let lit = i.sunlit * vec3f({}, {}, {}) / 255.0;\n",
            f(dirt[0]),
            f(dirt[1]),
            f(dirt[2])
        )),
    }
    s.push_str(
        "  let out = clamp(texel * lit * (1.0 - i.fog) + view.fog_color.rgb * i.fog, vec3f(0.0), vec3f(255.0));\n\
           return vec4f(out / 255.0, 1.0);\n\
         }\n",
    );
    s
}

/// What every shader of the 3D picture knows about its view.
const VIEW_WGSL: &str = r#"
struct View {
  vp: mat4x4f,
  eye: vec4f,
  sun: vec4f,
  sun_color: vec4f,
  ambient: vec4f,
  fog_color: vec4f,
  fog: vec4f,
  forward: vec4f,
  right: vec4f,
  up: vec4f,
}
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var palette: texture_2d<f32>;

// A point through a row-vector matrix (`M4::clip`): the matrix's rows are uploaded as its columns.
fn rows(m: mat4x4f, p: vec3f) -> vec4f {
  return p.x * m[0] + p.y * m[1] + p.z * m[2] + m[3];
}
fn to_clip(p: vec3f) -> vec4f {
  return rows(view.vp, p);
}
fn fog_at(p: vec3f) -> f32 {
  return clamp((distance(p, view.eye.xyz) - view.fog.x) / max(view.fog.y - view.fog.x, 1.0), 0.0, view.fog.w);
}
"#;

const MESH_WGSL: &str = r#"
struct View {
  vp: mat4x4f,
  eye: vec4f,
  sun: vec4f,
  sun_color: vec4f,
  ambient: vec4f,
  fog_color: vec4f,
  fog: vec4f,
  forward: vec4f,
  right: vec4f,
  up: vec4f,
}
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var palette: texture_2d<f32>;

fn rows(m: mat4x4f, p: vec3f) -> vec4f {
  return p.x * m[0] + p.y * m[1] + p.z * m[2] + m[3];
}
fn dir(m: mat4x4f, d: vec3f) -> vec3f {
  return (d.x * m[0] + d.y * m[1] + d.z * m[2]).xyz;
}

struct Material {
  constant: vec4f,
  cutoff: f32,
  shading: u32,
}
@group(1) @binding(0) var<uniform> material: Material;
@group(1) @binding(1) var tex: texture_2d<f32>;
@group(1) @binding(2) var samp: sampler;

struct In {
  @location(0) pos: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) color: vec4f,
  @location(4) node: u32,
  @location(5) m0: vec4f,
  @location(6) m1: vec4f,
  @location(7) m2: vec4f,
  @location(8) m3: vec4f,
  @location(9) pal: vec2u,
}
struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) lit: vec3f,
  @location(2) fog: f32,
}

// A node's matrix, four texels of the palette.
fn node_matrix(i: u32) -> mat4x4f {
  let x = i32(i % 256u) * 4;
  let y = i32(i / 256u);
  return mat4x4f(
    textureLoad(palette, vec2i(x, y), 0),
    textureLoad(palette, vec2i(x + 1, y), 0),
    textureLoad(palette, vec2i(x + 2, y), 0),
    textureLoad(palette, vec2i(x + 3, y), 0),
  );
}

@vertex fn vs(v: In) -> Out {
  // A vertex whose node the pose does not reach stays where it is, as `skin::skin` leaves it.
  let node = node_matrix(select(0u, v.pal.x + v.node, v.node < v.pal.y));
  let inst = mat4x4f(v.m0, v.m1, v.m2, v.m3);
  let world = rows(inst, rows(node, v.pos).xyz).xyz;
  let n = dir(inst, dir(node, v.normal));
  let l = length(n);
  let normal = select(n, n / l, l > 0.0);
  let paint = v.color.rgb * 255.0 / 128.0;
  var lit = (view.ambient.rgb + view.sun_color.rgb * max(-dot(normal, view.sun.xyz), 0.0)) * paint;
  if (material.shading == 1u) {
    lit = paint;
  } else if (material.shading == 2u) {
    lit = material.constant.rgb / 128.0;
  }
  var o: Out;
  o.clip = rows(view.vp, world);
  o.uv = v.uv;
  o.lit = lit;
  o.fog = clamp((distance(world, view.eye.xyz) - view.fog.x) / max(view.fog.y - view.fog.x, 1.0), 0.0, view.fog.w);
  return o;
}

@fragment fn fs(i: Out) -> @location(0) vec4f {
  let t = textureSample(tex, samp, i.uv) * 255.0;
  if (t.a < material.cutoff) {
    discard;
  }
  let out = clamp(t.rgb * i.lit * (1.0 - i.fog) + view.fog_color.rgb * i.fog, vec3f(0.0), vec3f(255.0));
  return vec4f(out / 255.0, t.a / 255.0);
}
"#;

/// The backdrop: the sky's faces looked up along each pixel's ray (`Sky::color`), or the fog's colour.
const SKY_WGSL: &str = r#"
struct View {
  vp: mat4x4f,
  eye: vec4f,
  sun: vec4f,
  sun_color: vec4f,
  ambient: vec4f,
  fog_color: vec4f,
  fog: vec4f,
  forward: vec4f,
  right: vec4f,
  up: vec4f,
}
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var faces: texture_2d_array<f32>;
@group(1) @binding(1) var samp: sampler;

struct Out {
  @builtin(position) pos: vec4f,
  @location(0) ndc: vec2f,
}

@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
  let p = vec2f(f32((i & 1u) << 2u) - 1.0, f32((i & 2u) << 1u) - 1.0);
  var o: Out;
  o.pos = vec4f(p, 0.5, 1.0);
  o.ndc = p;
  return o;
}

@fragment fn fs(i: Out) -> @location(0) vec4f {
  let d = view.forward.xyz + view.right.xyz * i.ndc.x + view.up.xyz * i.ndc.y;
  let a = abs(d);
  var face = 0;
  var uv = vec2f(0.0);
  if (a.y >= a.x && a.y >= a.z) {
    if (d.y > 0.0) {
      face = 4;
      uv = vec2f(d.x, d.z) / a.y;
    } else {
      face = 5;
      uv = vec2f(d.x, -d.z) / a.y;
    }
  } else if (a.z >= a.x) {
    if (d.z > 0.0) {
      face = 0;
      uv = vec2f(d.x, -d.y) / a.z;
    } else {
      face = 2;
      uv = vec2f(-d.x, -d.y) / a.z;
    }
  } else if (d.x < 0.0) {
    face = 1;
    uv = vec2f(d.z, -d.y) / a.x;
  } else {
    face = 3;
    uv = vec2f(-d.z, -d.y) / a.x;
  }
  let c = textureSampleLevel(faces, samp, (uv + 1.0) * 0.5, face, 0.0).rgb;
  return vec4f(select(view.fog_color.rgb / 255.0, c, view.fog.z > 0.5), 1.0);
}
"#;

/// Images laid over the target texel for texel: the overlay's multiply and add, and the blit.
const PLACE_WGSL: &str = r#"
struct Place {
  origin: vec2f,
  scale: f32,
}
@group(0) @binding(0) var a: texture_2d<f32>;
@group(0) @binding(1) var b: texture_2d<f32>;
@group(0) @binding(2) var<uniform> place: Place;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  return vec4f(f32((i & 1u) << 2u) - 1.0, f32((i & 2u) << 1u) - 1.0, 0.5, 1.0);
}

fn texel(t: texture_2d<f32>, pos: vec4f) -> vec4f {
  let size = vec2i(textureDimensions(t));
  let at = clamp(vec2i(floor((pos.xy - place.origin) / place.scale)), vec2i(0), size - 1);
  return textureLoad(t, at, 0);
}

@fragment fn fs_mul(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  return vec4f(texel(a, pos).rgb, 1.0);
}
@fragment fn fs_add(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  return vec4f(texel(b, pos).rgb, 0.0);
}
@fragment fn fs_blit(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  return vec4f(texel(a, pos).rgb, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_overlay_keeps_only_what_it_touched() {
        // A 4x3 layer that filled one pixel red at (1, 1) and halved the one at (2, 2).
        let (w, h) = (4, 3);
        let mut black = vec![0u8; w * h * 3];
        let mut white = vec![255u8; w * h * 3];
        for (frame, under) in [(&mut black, 0u8), (&mut white, 255u8)] {
            frame[(w + 1) * 3..(w + 1) * 3 + 3].copy_from_slice(&[255, 0, 0]);
            let half = under / 2;
            frame[(2 * w + 2) * 3..(2 * w + 2) * 3 + 3].copy_from_slice(&[half; 3]);
        }
        let p = Patch::of(&black, &white, w, h, (10, 20)).expect("it touched two pixels");
        assert_eq!(p.rect, [11, 21, 2, 2]);
        // The fill: nothing of what is under, all of the red.
        assert_eq!(&p.mul[..3], &[0, 0, 0]);
        assert_eq!(&p.add[..3], &[255, 0, 0]);
        // The halving: half of what is under, nothing added.
        assert_eq!(&p.mul[9..12], &[127; 3]);
        assert_eq!(&p.add[9..12], &[0; 3]);
        // An untouched pixel inside the rectangle passes the frame through.
        assert_eq!(&p.mul[3..6], &[255; 3]);
        assert!(Patch::of(&vec![0; 12], &vec![255; 12], 2, 2, (0, 0)).is_none());
    }

    #[test]
    fn the_frustum_sees_in_front_and_not_behind() {
        let camera = Camera {
            eye: v3(0.0, 0.0, 0.0),
            at: v3(0.0, 0.0, 10.0),
            fov: 1.0,
            near: 0.5,
            far: 100.0,
        };
        let planes = frustum(&view_projection(&camera, 640, 480, camera.far));
        assert!(sees(&planes, v3(0.0, 0.0, 50.0), 1.0));
        assert!(!sees(&planes, v3(0.0, 0.0, -50.0), 1.0));
        assert!(!sees(&planes, v3(500.0, 0.0, 50.0), 1.0));
        // A sphere straddling the edge is seen.
        assert!(sees(&planes, v3(30.0, 0.0, 50.0), 10.0));
    }

    #[test]
    fn the_paint_program_is_written_out_as_the_shader() {
        let splat = Splat {
            textures: vec![Texture {
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            }],
            passes: vec![crate::splat::Pass {
                texture: 0,
                scale: [0.01, 0.01],
                angle: 0.0,
                turn: 0f32.sin_cos(),
                reads: (true, true),
                rgba: [128.0; 4],
                blend: Some([0, 1, 0, 1]),
                fixed: 128.0,
            }],
            shadow: None,
        };
        let s = ground_wgsl(Some(&splat), [1.0; 3]);
        assert!(s.contains("var layer0: texture_2d<f32>"), "{s}");
        assert!(s.contains("frame = vec4f(clamp((src.rgb - frame.rgb) * src.a / 128.0 + frame.rgb"), "{s}");
        assert!(ground_wgsl(None, [150.0, 118.0, 86.0]).contains("let texel = vec3f(255.0)"));
    }

    /// A made-up scene of two flat-coloured quads, one lit by the sun and one prelit, one translucent
    /// over the other, in fog: drawn by the rasteriser and offscreen on the GPU.
    fn known_scene() -> (Scene, Assets, Camera) {
        use crate::psg::{Mesh, Node, Strip, Vertex};
        let quad = |x: f32, z: f32, size: f32, color: [u8; 4]| Strip {
            verts: [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
                .iter()
                .map(|&(u, v)| Vertex {
                    pos: v3(x + (u - 0.5) * size, (v - 0.5) * size, z),
                    normal: v3(0.0, 0.0, -1.0),
                    uv: [u, v],
                    color,
                    node: 0,
                    adc: false,
                })
                .collect(),
        };
        let model = Model {
            nodes: vec![Node {
                name: "root".into(),
                local: M4::placement(v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0), v3(0.0, 1.0, 0.0)),
                parent: None,
                center: v3(0.0, 0.0, 0.0),
                half_extents: v3(4.0, 4.0, 1.0),
                radius: 6.0,
            }],
            materials: vec!["lit".into(), "glass".into()],
            meshes: vec![
                Mesh {
                    material: 0,
                    group: 0,
                    strips: vec![quad(-1.0, 0.0, 4.0, [128, 64, 32, 128]), quad(2.5, 3.0, 3.0, [100, 128, 128, 128])],
                },
                Mesh {
                    material: 1,
                    group: 0,
                    strips: vec![quad(0.5, -1.0, 2.0, [128, 128, 128, 128])],
                },
            ],
            groups: 1,
            group_distance: vec![f32::MAX],
        };
        let mut assets = Assets::default();
        // One colour each, so the GPU's filtering and the rasteriser's nearest texel agree.
        assets.textures.push(Texture {
            width: 4,
            height: 4,
            rgba: [200, 180, 160, 255].repeat(16),
        });
        assets.textures.push(Texture {
            width: 2,
            height: 2,
            rgba: [40, 90, 255, 128].repeat(4),
        });
        let tris = model.triangles(0);
        let glass = Surface {
            shading: Shading::Prelit,
            cutoff: 0,
            blend: true,
            additive: false,
            depth_write: false,
            clamp: [false; 2],
        };
        assets.models.insert(
            "board".into(),
            crate::world::Loaded {
                model,
                tris,
                textures: vec![Some(0), Some(1)],
                surfaces: vec![Surface::default(), glass],
                radius: 6.0,
            },
        );
        let scene = Scene {
            name: "test".into(),
            start: v3(0.0, 0.0, 0.0),
            start_dir: v3(0.0, 0.0, 1.0),
            terrain: String::new(),
            fog_color: [90, 110, 140],
            fog_near: 5.0,
            fog_far: 40.0,
            lights: Vec::new(),
            models: vec![
                crate::scene::Placement {
                    model: "board".into(),
                    pos: v3(0.0, 0.0, 8.0),
                    look: v3(0.0, 0.0, 1.0),
                    up: v3(0.0, 1.0, 0.0),
                },
                crate::scene::Placement {
                    model: "board".into(),
                    pos: v3(-3.0, 0.5, 20.0),
                    look: v3(0.3, 0.0, 1.0),
                    up: v3(0.0, 1.0, 0.0),
                },
            ],
        };
        let camera = Camera {
            eye: v3(0.0, 1.5, 0.0),
            at: v3(0.0, 1.0, 10.0),
            fov: 1.0,
            near: 0.1,
            far: 1000.0,
        };
        (scene, assets, camera)
    }

    #[test]
    fn the_gpu_draws_a_known_scene_as_the_rasteriser_does() {
        let mut gpu = match Renderer::headless() {
            Ok(r) => r,
            Err(e) => {
                // The CI box may have no GPU at all; say so rather than pass in silence.
                eprintln!("skipped: {e}");
                return;
            }
        };
        let (scene, assets, camera) = known_scene();
        let (w, h) = (160u32, 120u32);
        let mut frame = crate::raster::Frame::new(w as usize, h as usize, scene.fog_color);
        let soft = crate::world::draw_scene(&mut frame, &scene, &assets, &[], &camera);
        assert!(soft > 0);

        let light = Lighting::of(&scene);
        // The rasteriser draws the original's fog, so this is drawn under it too.
        gpu.set_graphics(Graphics::Original);
        gpu.set_scene(&scene);
        gpu.set_sky(None);
        let texture = gpu.offscreen(w, h);
        let view = texture.create_view(&Default::default());
        let views = [View {
            rect: [0, 0, w, h],
            camera: &camera,
            actors: &[],
        }];
        let drawn = gpu.render(
            &Target {
                view: &view,
                size: (w, h),
                frame: [0, 0, w, h],
                scale: 1,
            },
            &assets,
            &Scene3 {
                views: &views,
                poses: &[],
                overlay: &[],
                light: &light,
            },
        );
        assert_eq!(drawn.triangles, vec![soft], "both drew every triangle of both copies");
        let hard = gpu.read(&texture);
        assert_eq!(hard.len(), frame.color.len());

        // Pixel for pixel, but for the edges, where the two rasterisers' coverage rules part.
        let (mut off, mut worst) = (0, 0);
        for (a, b) in hard.chunks_exact(3).zip(frame.color.chunks_exact(3)) {
            let d = a.iter().zip(b).map(|(&a, &b)| (a as i32 - b as i32).abs()).max().unwrap_or(0);
            if d > 6 {
                off += 1;
            }
            worst = worst.max(d);
        }
        let share = off as f32 / (w * h) as f32;
        eprintln!("{off} of {} pixels differ by more than 6, worst {worst}", w * h);
        if let Ok(dir) = std::env::var("GPU_TEST_PNG") {
            let _ = crate::png::write_rgb(&std::path::Path::new(&dir).join("gpu.png"), w as usize, h as usize, &hard);
            let _ = crate::png::write_rgb(&std::path::Path::new(&dir).join("soft.png"), w as usize, h as usize, &frame.color);
        }
        assert!(share < 0.03, "{off} pixels ({:.1}%) differ by more than 6, worst {worst}", share * 100.0);
        // And it drew something besides the fog.
        assert!(hard.chunks_exact(3).filter(|p| p != &scene.fog_color).count() > 1000);
    }

    /// P57: an actor below alpha 1 is drawn over what is behind it at that alpha — the bike's put-back
    /// ghost — and at 0 not at all. The known scene's board, drawn as an actor rather than a prop, three
    /// times over an empty scene: solid, at half, and at nothing. Wherever the solid board changed the
    /// fog, the half board sits halfway between the two, but for the edges.
    #[test]
    fn a_ghost_actor_is_drawn_at_its_alpha() {
        let mut gpu = match Renderer::headless() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipped: {e}");
                return;
            }
        };
        let (mut scene, mut assets, camera) = known_scene();
        // Without its glass: a translucent pane over the board is not halfway between two draws.
        let loaded = assets.models.get_mut("board").unwrap();
        loaded.model.meshes.truncate(1);
        loaded.tris = loaded.model.triangles(0);
        let board = scene.models.remove(0);
        scene.models.clear();
        let at = M4::placement(board.pos, board.look, board.up);
        let (w, h) = (160u32, 120u32);
        let light = Lighting::of(&scene);
        gpu.set_graphics(Graphics::Original);
        gpu.set_scene(&scene);
        gpu.set_sky(None);
        let texture = gpu.offscreen(w, h);
        let view = texture.create_view(&Default::default());
        let mut draw = |alpha: Option<f32>| {
            let actors: Vec<Actor> =
                alpha.map(|alpha| Actor { model: "board", at, pose: None, lod_from: None, alpha }).into_iter().collect();
            let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &actors }];
            let target = Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 };
            gpu.render(&target, &assets, &Scene3 { views: &views, poses: &[], overlay: &[], light: &light });
            gpu.read(&texture)
        };
        let empty = draw(None);
        let solid = draw(Some(1.0));
        let half = draw(Some(0.5));
        let none = draw(Some(0.0));
        assert_eq!(none, empty, "an actor at alpha 0 was drawn");
        let (mut changed, mut halfway) = (0, 0);
        for ((e, s), g) in empty.chunks_exact(3).zip(solid.chunks_exact(3)).zip(half.chunks_exact(3)) {
            if e.iter().zip(s).all(|(&e, &s)| (e as i32 - s as i32).abs() <= 8) {
                assert!(e.iter().zip(g).all(|(&e, &g)| (e as i32 - g as i32).abs() <= 2), "the ghost drew off the board");
                continue;
            }
            changed += 1;
            if (0..3).all(|k| (g[k] as i32 - (e[k] as i32 + s[k] as i32) / 2).abs() <= 2) {
                halfway += 1;
            }
        }
        eprintln!("{halfway} of {changed} board pixels halfway between the fog and the solid board");
        if let Ok(dir) = std::env::var("GPU_TEST_PNG") {
            for (name, rgb) in [("ghost-solid.png", &solid), ("ghost-half.png", &half)] {
                let _ = crate::png::write_rgb(&std::path::Path::new(&dir).join(name), w as usize, h as usize, rgb);
            }
        }
        assert!(changed > 500, "the board covered {changed} pixels");
        assert!(halfway as f32 > 0.95 * changed as f32, "only {halfway} of {changed} pixels are halfway");
    }

    /// A scene with the disc's kind of fog: free07's, 25 to 1,000 feet.
    fn fogged() -> Lighting {
        Lighting::of(&crate::scene::Scene::parse(
            "[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=25.00\nMaximumFogDistance=1000.00\n",
        ))
    }

    /// A camera on a map's near edge, 300 feet in, looking along it.
    fn on_the_near_edge() -> Camera {
        Camera {
            eye: v3(100.0, 30.0, 300.0),
            at: v3(100.0, 20.0, 400.0),
            fov: 1.0,
            near: 2.4,
            far: 1000.0,
        }
    }

    #[test]
    fn the_original_is_the_discs_fog_and_far_clip() {
        let light = fogged();
        let span = 9000.0;
        let fog = Graphics::Original.fog(&light, span);
        assert_eq!(fog, Fog { near: 25.0, far: 1000.0, max: 1.0, reach: 1000.0 });
        assert_eq!(Graphics::Original.far_clip(&light, span), 1000.0);
        // The whole map: no reach, a far clip past the map's far corner, and a haze that never hides.
        let full = Graphics::Full.fog(&light, span);
        assert_eq!(full.reach, f32::INFINITY);
        assert!(full.max <= crate::world::HAZE_MAX && full.max < 1.0);
        assert!(full.near >= 1000.0, "the haze starts where the original's fog ends");
        assert!(Graphics::Full.far_clip(&light, span) >= span);
        assert_eq!(Graphics::default(), Graphics::Full);
        for g in Graphics::ALL {
            assert_eq!(Graphics::from_word(g.word()), Some(g));
        }
    }

    #[test]
    fn the_whole_map_sees_its_far_edge_and_culls_behind() {
        let light = fogged();
        let camera = on_the_near_edge();
        let span = 6400.0 * std::f32::consts::SQRT_2;
        // Ground chunks 200 feet across: one at the far edge of a 6,400-foot map, one behind the camera.
        let (far_edge, behind) = (v3(100.0, 0.0, 6300.0), v3(100.0, 0.0, 100.0));
        let radius = 145.0;
        // Each of four split views, as a quarter of 4K.
        let full = Cull::of(&camera, 1920, 1080, Graphics::Full, &light, span);
        assert!(full.sees(far_edge, radius));
        assert!(!full.sees(behind, radius));
        let original = Cull::of(&camera, 1920, 1080, Graphics::Original, &light, span);
        assert!(!original.sees(far_edge, radius), "past the disc's far clip");
        assert!(!original.sees(behind, radius));
        assert!(original.sees(v3(100.0, 0.0, 900.0), radius));
    }

    /// A flat chunk of `cells` cells `size` feet each at chunk (`x`, `z`).
    fn flat_chunk(cells: u32, size: f32, x: u32, z: u32) -> GroundChunk {
        let side = cells as f32 * size;
        let (x0, z0) = (x as f32 * side, z as f32 * side);
        let vertices = (0..=cells)
            .flat_map(|j| (0..=cells).map(move |i| (i, j)))
            .map(|(i, j)| GroundVertex {
                pos: [x0 + i as f32 * size, 0.0, z0 + j as f32 * size],
                normal: [0.0, 1.0, 0.0],
            })
            .collect();
        GroundChunk {
            vertices,
            cells,
            at: (x, z),
            missing: Vec::new(),
            centre: v3(x0 + side / 2.0, 0.0, z0 + side / 2.0),
            radius: side * std::f32::consts::FRAC_1_SQRT_2,
        }
    }

    #[test]
    fn the_gpu_draws_the_far_edge_under_full_and_not_under_original() {
        let mut gpu = match Renderer::headless() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipped: {e}");
                return;
            }
        };
        // A 6,400-foot map of 200-foot chunks with three put down: behind the camera, ahead of it, and
        // at the far edge.
        let chunks = [flat_chunk(8, 25.0, 0, 0), flat_chunk(8, 25.0, 0, 2), flat_chunk(8, 25.0, 0, 31)];
        let scene = crate::scene::Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=25.00\nMaximumFogDistance=1000.00\n");
        let light = Lighting::of(&scene);
        gpu.set_scene(&scene);
        gpu.set_sky(None);
        gpu.set_ground(&chunks, None, [150.0, 118.0, 86.0]).unwrap();
        let camera = on_the_near_edge();
        let (w, h) = (320u32, 180u32);
        let texture = gpu.offscreen(w, h);
        let view = texture.create_view(&Default::default());
        let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
        let assets = Assets::default();
        let mut drawn = |gpu: &mut Renderer, graphics| {
            gpu.set_graphics(graphics);
            let d = gpu.render(
                &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
                &assets,
                &Scene3 { views: &views, poses: &[], overlay: &[], light: &light },
            );
            d.ground_levels.iter().sum::<usize>()
        };
        assert_eq!(drawn(&mut gpu, Graphics::Full), 2, "the chunk ahead and the far edge's");
        assert_eq!(drawn(&mut gpu, Graphics::Original), 1, "the chunk ahead only");
        // And back: the per-view index list follows the setting.
        assert_eq!(drawn(&mut gpu, Graphics::Full), 2);
    }

    /// Every shader, written out as the GLSL ES 3.00 wgpu's WebGL2 backend hands the browser (P48):
    /// WebGL2 has no storage buffers, no compute and no textures read without a sampler in GLSL's
    /// sense, and a shader that needs any of them fails to translate here. The ground's is written
    /// with every paint layer WebGL2 can sample beside the light map, and every blend a pass can ask.
    #[test]
    fn every_shader_translates_for_webgl2() {
        use naga::back::glsl;
        let layer = || Texture { width: 1, height: 1, rgba: vec![0; 4] };
        let max = wgpu::Limits::downlevel_webgl2_defaults().max_sampled_textures_per_shader_stage as usize;
        let passes = (0..max - 1)
            .map(|i| crate::splat::Pass {
                texture: i,
                scale: [0.5, 0.25],
                angle: 0.3,
                turn: 0.3f32.sin_cos(),
                reads: (true, true),
                rgba: [128.0; 4],
                blend: [None, Some([0, 1, 0, 1]), Some([1, 2, 1, 0]), Some([2, 0, 2, 2])][i % 4],
                fixed: 64.0,
            })
            .collect();
        let shadow = crate::splat::Shadow { tiles: vec![layer()], across: 1, x0: -10.0, x1: 10.0, z0: -10.0, z1: 10.0 };
        let paint = Splat { textures: (0..max - 1).map(|_| layer()).collect(), passes, shadow: Some(shadow) };
        let shaders = [
            ("mesh", MESH_WGSL.to_string()),
            ("sky", SKY_WGSL.to_string()),
            ("place", PLACE_WGSL.to_string()),
            ("ground, painted", ground_wgsl(Some(&paint), [200.0, 180.0, 150.0])),
            ("ground, bare", ground_wgsl(None, [200.0, 180.0, 150.0])),
        ];
        let mut written = 0;
        let mut translate = |name: &str, source: &str| -> Result<(), String> {
            let module = naga::front::wgsl::parse_str(source).unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
            let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
                .validate(&module)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(source)));
            let options = glsl::Options {
                version: glsl::Version::Embedded { version: 300, is_webgl: true },
                writer_flags: glsl::WriterFlags::ADJUST_COORDINATE_SPACE | glsl::WriterFlags::FORCE_POINT_SIZE,
                ..Default::default()
            };
            for entry in &module.entry_points {
                assert!(
                    matches!(entry.stage, naga::ShaderStage::Vertex | naga::ShaderStage::Fragment),
                    "{name}: a {:?} shader, which WebGL2 does not have",
                    entry.stage
                );
                let pipeline = glsl::PipelineOptions { shader_stage: entry.stage, entry_point: entry.name.clone(), multiview: None };
                let mut out = String::new();
                glsl::Writer::new(&mut out, &module, &info, &options, &pipeline, Default::default())
                    .and_then(|mut w| w.write())
                    .map_err(|e| format!("{name}, {}: not GLSL ES 3.00: {e}", entry.name))?;
                assert!(out.starts_with("#version 300 es"), "{name}, {}: {}", entry.name, &out[..40.min(out.len())]);
                assert!(!out.contains("buffer "), "{name}, {}: a storage buffer", entry.name);
                written += 1;
            }
            Ok(())
        };
        for (name, source) in &shaders {
            translate(name, source).unwrap();
        }
        // The check bites: a storage buffer, which WebGPU has and WebGL2 does not, is refused.
        let storage = "@group(0) @binding(0) var<storage, read> palette: array<vec4f>;\n\
                       @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f { return palette[i]; }\n";
        assert!(translate("storage", storage).is_err());
        // mesh: vs + its fragment entries; sky, place, and the two grounds: one of each at least.
        assert!(written >= 2 * shaders.len(), "only {written} entry points written");
    }
}
