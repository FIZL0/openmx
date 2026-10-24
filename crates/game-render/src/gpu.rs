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
//! The original's HUD is drawn as sprites ([`Sprite`], V30): quads cut from pictures uploaded once.
//! The debug HUD and the menus are still drawn by the CPU into frames ([`crate::raster::Frame`]) and
//! laid over the picture as an overlay ([`Patch`]).
//!
//! The views can be drawn at a fraction of the frame's size and stretched over it
//! ([`Renderer::render_scale`], V30), with the sprites and the overlay laid on at full size.

pub use wgpu;

mod scale;
mod sprite;
mod ground_mesh;
pub use scale::{AutoScale, MIN_SCALE};
pub use sprite::{Shape, Sprite, clamp_of};

use crate::mat::{M4, V3, v3};
use crate::psg::Model;
use crate::pst::Texture;
use crate::scene::{Scene, Shading};
use crate::sky::Sky;
use crate::splat::Splat;
use crate::terrain::{Packed, Pages, WorldPaint, lod_balance, lod_coarser, lod_errors, lod_indices, lod_indices_where, lod_level, lod_levels};
use crate::world::{Assets, Camera, Fog, Graphics, Lighting, Surface};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

mod sprites;
pub use sprites::Sprite as Particle;

mod glows;

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

/// One drawn copy of a model: where it stands, where its palette starts and how long it is, and the
/// share of its pixels it draws ([`crate::eco::covers`]; 1 for all of them).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceRaw {
    m: [[f32; 4]; 4],
    palette: [u32; 2],
    cover: f32,
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
    /// Where the view's coordinates start (MD15): the camera's eye. Everything is drawn relative to
    /// it, so a world 116,000 feet across does not jitter where it is drawn far from the origin.
    origin: [f32; 4],
    _pad: [f32; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    constant: [f32; 4],
    cutoff: f32,
    shading: u32,
    /// 1 for [`Surface::keep_coverage`]; the shader no longer reads it, the texture's levels keep
    /// the coverage (V58).
    coverage: u32,
    /// Texels at or over this alpha are not drawn: the half of a material whose failing texels are
    /// drawn apart from its passing ones ([`GpuMaterial::rest`]).
    below: f32,
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
    depth_cutoff: u8,
    pipeline: PipelineKey,
    /// Clamped at the texture's edges along u and v rather than repeated.
    clamp: [bool; 2],
    keep_coverage: bool,
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
            depth_cutoff: s.depth_cutoff,
            pipeline: PipelineKey {
                blend,
                depth_write: s.depth_write,
                ghost: false,
            },
            clamp: s.clamp,
            keep_coverage: s.keep_coverage,
        }
    }
}

struct GpuMaterial {
    bind: wgpu::BindGroup,
    pipeline: PipelineKey,
    /// A material whose texels under [`Surface::depth_cutoff`] are drawn without writing depth
    /// (V54): those texels, drawn again after the rest with depth writes off. The draw before only
    /// draws the texels that pass.
    rest: Option<(wgpu::BindGroup, PipelineKey)>,
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
/// What the ground is painted with ([`Renderer::set_ground_painted`]).
#[derive(Clone, Copy)]
pub enum GroundPaint<'a> {
    /// One dirt colour.
    Bare,
    /// A freeride track's layers and baked light (V25).
    Splat(&'a Splat),
    /// A mod track's own terrain pages and detail map, as MX vs. ATV draws its maps (MD4).
    Pages(&'a Pages),
    /// A world's maps' pages, each over its own cells and blended across the joins (MD9).
    World(&'a WorldPaint),
}

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
    /// Which page of the vertex buffers holds its corners, and where in it they start.
    page: u32,
    base: u32,
    centre: V3,
    radius: f32,
    /// Its height error at each level ([`lod_errors`]).
    errors: Vec<f32>,
    /// A chunk with holes: which cells it lacks, and its whole grid stitched each of the sixteen
    /// ways, each made the first time it is drawn that way (a world has thousands of such chunks,
    /// and all sixteen of each were most of its memory, MD17).
    own: Option<(Vec<bool>, Vec<Option<Vec<u32>>>)>,
    /// The piece of ground it came with ([`Renderer::add_ground_piece`]), and where in that piece.
    piece: u32,
    at: (u32, u32),
}

impl GpuChunk {
    /// A chunk of `cells` cells whose corners start at `base` in the vertex buffer.
    fn of(c: &GroundChunk, cells: u32, page: u32, base: u32, piece: u32) -> GpuChunk {
        let heights: Vec<f32> = c.vertices.iter().map(|v| v.pos[1]).collect();
        let own = c.missing.iter().any(|&m| m).then(|| (c.missing.clone(), vec![None; 16]));
        GpuChunk { page, base, centre: c.centre, radius: c.radius, errors: lod_errors(&heights, cells), own, piece, at: c.at }
    }
}

struct GpuGround {
    /// The chunks' corners, in pages of `per_page` chunks each but the last, which grows: no buffer
    /// can be bigger than the device allows, and a world's ground is (MD17).
    pages: Vec<wgpu::Buffer>,
    per_page: u32,
    /// How many chunks the pages have room for, and which of those places are free: place `p` is
    /// page `p / per_page`.
    capacity: u32,
    free: Vec<u32>,
    /// Each piece's chunks as they were handed over, as one number ([`Renderer::ground_piece_hash`]).
    hashes: HashMap<u32, u64>,
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
    /// The roads and ruts a freeride scene paints into the ground (V67).
    meshes: Option<ground_mesh::GpuMeshes>,
}

impl GpuGround {
    /// Free piece `piece`'s chunks' places; [`GpuGround::regrid`] after.
    fn forget(&mut self, piece: u32) {
        let corners = (self.cells + 1) * (self.cells + 1);
        let free = &mut self.free;
        self.chunks.retain(|c| {
            if c.piece == piece {
                free.push(c.page * self.per_page + c.base / corners);
            }
            c.piece != piece
        });
    }

    /// Lay the chunks out in one grid again: each piece in a column band of its own, in piece
    /// order, one empty column after it so no piece is stitched to the next. Every view's indices
    /// are built again on its next frame.
    fn regrid(&mut self) {
        let mut pieces: Vec<u32> = self.chunks.iter().map(|c| c.piece).collect();
        pieces.sort_unstable();
        pieces.dedup();
        let mut band = Vec::with_capacity(pieces.len());
        let mut column = 0usize;
        for &p in &pieces {
            band.push(column);
            column += self.chunks.iter().filter(|c| c.piece == p).map(|c| c.at.0 as usize + 1).max().unwrap_or(0) + 1;
        }
        self.across = column.saturating_sub(1);
        self.along = self.chunks.iter().map(|c| c.at.1 as usize + 1).max().unwrap_or(0);
        self.slots = vec![None; self.across * self.along];
        for (i, c) in self.chunks.iter().enumerate() {
            let x = band[pieces.binary_search(&c.piece).expect("listed")] + c.at.0 as usize;
            self.slots[c.at.1 as usize * self.across + x] = Some(i);
        }
        self.levels = vec![0; self.slots.len()];
        for v in &mut self.views {
            v.key.clear();
            v.count = 0;
            v.runs.clear();
        }
    }
}

/// Ground chunks as one number ([`Renderer::ground_piece_hash`]).
fn chunks_hash(chunks: &[GroundChunk]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for c in chunks {
        (c.cells, c.at, &c.missing).hash(&mut h);
        for v in &c.vertices {
            v.pos.map(f32::to_bits).hash(&mut h);
            v.normal.map(f32::to_bits).hash(&mut h);
        }
        [c.centre.x, c.centre.y, c.centre.z, c.radius].map(f32::to_bits).hash(&mut h);
    }
    h.finish()
}

/// A ground vertex buffer with room for `chunks` chunks of `cells` cells.
fn ground_vertex_buffer(device: &wgpu::Device, cells: u32, chunks: u32) -> wgpu::Buffer {
    let corners = u64::from((cells + 1) * (cells + 1));
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ground vertices"),
        size: u64::from(chunks.max(1)) * corners * std::mem::size_of::<GroundVertex>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

/// One view's ground: what it drew last frame, kept until the chunks it sees or their levels change.
struct GroundView {
    buffer: wgpu::Buffer,
    capacity: u64,
    /// For each chunk, the pattern it was drawn with, or `u32::MAX` for not drawn.
    key: Vec<u32>,
    count: u32,
    indices: Vec<u32>,
    /// The indices' runs a page at a time: the page, its first index and how many.
    runs: Vec<(u32, u32, u32)>,
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
        // Untouched as a whole, folded rather than stopped at the first touched byte so that it runs
        // a vector at a time (V30): a 4K layer is fifty megabytes to look through.
        let blank = |b: &[u8], w: &[u8]| b.iter().fold(0, |a, &c| a | c) == 0 && w.iter().fold(255, |a, &c| a & c) == 255;
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for y in 0..height {
            let row = y * width * 3;
            let (b, w) = (&black[row..row + width * 3], &white[row..row + width * 3]);
            if blank(b, w) {
                continue;
            }
            let touched = |x: usize| b[x * 3..x * 3 + 3] != [0; 3] || w[x * 3..x * 3 + 3] != [255; 3];
            // Only what lies outside the columns found so far can widen them.
            let left = x0.min(width);
            if !blank(&b[..left * 3], &w[..left * 3])
                && let Some(f) = (0..left).find(|&x| touched(x))
            {
                x0 = f;
            }
            if !blank(&b[x1 * 3..], &w[x1 * 3..])
                && let Some(l) = (x1..width).rev().find(|&x| touched(x))
            {
                x1 = l + 1;
            }
            (y0, y1) = (y0.min(y), y + 1);
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
    /// Drawn over the views and the overlay, in order ([`Renderer::set_images`]' pictures).
    pub sprites: &'a [Sprite],
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
    /// The crossfade: the second sky's share, and whether each sky is there ([`Renderer::set_sky_mix`]).
    sky_mix: wgpu::Buffer,
    sky_has: [bool; 2],
    sky_share: f32,
    has_sky: bool,
    place_layout: wgpu::BindGroupLayout,
    overlay_mul: wgpu::RenderPipeline,
    overlay_add: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    clamp_sampler: wgpu::Sampler,
    white: wgpu::TextureView,
    /// Each texture as uploaded, by its number and the cutoff its mips keep the coverage of (0 for
    /// plain box mips, [`Surface::keep_coverage`]'s cut-outs for theirs).
    textures: HashMap<(usize, u8), wgpu::TextureView>,
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
    /// The scene's trees and bushes (V48), drawn with the props.
    grove: Option<crate::eco::Grove>,
    /// More groves, each with its owner (MD15): a world's tiles', moved to where each tile is.
    groves: Vec<(u32, crate::eco::Grove)>,
    /// How far the world is drawn (V32), and how far apart the ground's two farthest points are.
    graphics: Graphics,
    span: f32,
    /// The frame's instances and batches, kept to reuse their allocations.
    /// Each copy's batch key, its see-through alpha's bits (0 for solid) and the copy.
    scratch: Vec<(u64, u32, InstanceRaw)>,
    batches: Vec<Batch>,
    flat: Vec<InstanceRaw>,
    blit: Option<Blit>,
    sprites: sprite::Sprites,
    upscale: scale::Upscale,
    /// The fraction of the frame's size the views are drawn at, [`MIN_SCALE`] to 1 (V30).
    pub render_scale: f32,
    /// The particle sheet and this frame's particles (P58).
    particles: Option<sprites::Sprites>,
    /// The glow textures and this frame's glows (V51).
    glows: Option<glows::Glows>,
}

/// A prop the scene places, as culling needs it.
struct Prop {
    name: String,
    model: Option<Option<usize>>,
    at: M4,
    /// Whose it is: [`SCENE`] for the scene's, or what [`Renderer::add_props`] was told (MD15).
    owner: u32,
}

/// The owner of the scene's own props and grove ([`Renderer::set_scene`]).
const SCENE: u32 = u32::MAX;

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
    /// The patches written last frame (V30): the same again — a pause standing still — is left in
    /// the textures rather than written over them, which at 4K was a quarter of a gigabyte a second.
    kept: Vec<Patch>,
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

/// A pass over `color` and `depth`, both cleared: black, and far.
fn begin_pass<'e>(encoder: &'e mut wgpu::CommandEncoder, color: &wgpu::TextureView, depth: &wgpu::TextureView, label: &str) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0.0),
                store: wgpu::StoreOp::Discard,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// A matrix as four texels, its rows in order.
fn flat(m: M4) -> [f32; 16] {
    let r = m.0;
    [
        r[0][0], r[0][1], r[0][2], r[0][3], r[1][0], r[1][1], r[1][2], r[1][3], r[2][0], r[2][1], r[2][2], r[2][3],
        r[3][0], r[3][1], r[3][2], r[3][3],
    ]
}

/// A texture's mip chain, each level half the last, by a box filter. The colour is averaged by
/// alpha, and every wholly clear texel of every level takes the colour of the level above it there
/// (V55): a cut-out's holes keep whatever colour the artist left in them (free06's bush atlas a
/// dark green), and the filtering mixed it into every edge, so a twiggy bush or a far billboard
/// came out the holes' colour rather than its own.
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
                let at = |xx: usize, yy: usize, k: usize| px[(yy.min(h - 1) * w + xx.min(w - 1)) * 4 + k] as u32;
                let four = [(2 * x, 2 * y), (2 * x + 1, 2 * y), (2 * x, 2 * y + 1), (2 * x + 1, 2 * y + 1)];
                let alpha: u32 = four.iter().map(|&(xx, yy)| at(xx, yy, 3)).sum();
                for k in 0..3 {
                    next[(y * nw + x) * 4 + k] = if alpha == 0 {
                        let s: u32 = four.iter().map(|&(xx, yy)| at(xx, yy, k)).sum();
                        ((s + 2) / 4) as u8
                    } else {
                        let s: u32 = four.iter().map(|&(xx, yy)| at(xx, yy, k) * at(xx, yy, 3)).sum();
                        ((s + alpha / 2) / alpha) as u8
                    };
                }
                next[(y * nw + x) * 4 + 3] = ((alpha + 2) / 4) as u8;
            }
        }
        out.push((nw as u32, nh as u32, next));
    }
    for level in (0..out.len().saturating_sub(1)).rev() {
        let (above, below) = out.split_at_mut(level + 1);
        let (w, _, px) = &mut above[level];
        let (cw, ch, coarse) = &below[0];
        let (w, cw, ch) = (*w as usize, *cw as usize, *ch as usize);
        for (i, texel) in px.chunks_exact_mut(4).enumerate() {
            if texel[3] == 0 {
                let (x, y) = ((i % w / 2).min(cw - 1), (i / w / 2).min(ch - 1));
                texel[..3].copy_from_slice(&coarse[(y * cw + x) * 4..(y * cw + x) * 4 + 3]);
            }
        }
    }
    out
}

fn upload_texture(device: &wgpu::Device, queue: &wgpu::Queue, t: &Texture) -> wgpu::TextureView {
    upload_texture_keeping(device, queue, t, 0)
}

/// [`mips`], with every smaller level's alpha scaled so that each part of it, read bilinearly as
/// the sampler reads it, covers as much at `cutoff` as the full-size texture does there (V58; after I. Castaño's coverage-preserving
/// alpha mips, as The Witness has them, but per tile, since one atlas holds solid bark and sparse
/// twigs side by side). A cut-out's box-filtered levels average its leaves with the holes between
/// them, and most of a far tree's or bush's averaged texels fell under the cutoff: it thinned to a
/// quarter of its near self past 500 feet. A `cutoff` of 0 leaves the levels as they are.
fn mips_keeping(t: &Texture, cutoff: u8) -> Vec<(u32, u32, Vec<u8>)> {
    let mut levels = mips(t);
    if cutoff == 0 || levels.len() < 2 {
        return levels;
    }
    let (bw, bh) = (levels[0].0 as usize, levels[0].1 as usize);
    // How many of each full-size texel's 4x4 points its filtered alpha covers.
    let base = &levels[0].2;
    let (mut passes, mut row) = (Vec::with_capacity(bw * bh), Vec::new());
    for y in 0..bh {
        filtered(base, bw, bh, (0, y, bw, y + 1), &mut row);
        passes.extend(row.chunks_exact(16).map(|t| t.iter().filter(|&&a| a >= cutoff as f32).count() as u32));
    }
    for (w, h, px) in levels.iter_mut().skip(1) {
        let (w, h) = (*w as usize, *h as usize);
        // Tiles of 4x4 texels of this level (or the whole level when it is smaller), each the
        // same part of the full-size texture.
        let (tw, th) = (w.min(KEEP_TILE), h.min(KEEP_TILE));
        // Each tile's scale is found against the level as the box filter left it (its neighbours
        // read at the same scale), and all are applied after.
        let (mut scales, mut points) = (Vec::new(), Vec::new());
        for ty in (0..h).step_by(th) {
            for tx in (0..w).step_by(tw) {
                let (x1, y1) = ((tx + tw).min(w), (ty + th).min(h));
                // The full size's share of the tile it covers.
                let (sx, sy) = (bw / w, bh / h);
                let mut hit = 0;
                for y in ty * sy..(y1 * sy).min(bh) {
                    hit += passes[y * bw + tx * sx..y * bw + (x1 * sx).min(bw)].iter().sum::<u32>();
                }
                let want = hit as f32 / 16.0 / ((x1 * sx).min(bw) - tx * sx).max(1) as f32 / ((y1 * sy).min(bh) - ty * sy).max(1) as f32;
                // The scale that brings the filtered alpha of as many of the tile's points to the
                // cutoff as the full size covers: the alpha read between texels is the texels'
                // scaled, so it is the cutoff over the point that many down. Filtered, as the
                // sampler reads the level, so a lone texel brought just to the cutoff does not
                // count as a whole one.
                filtered(px, w, h, (tx, ty, x1, y1), &mut points);
                points.sort_unstable_by(|a, b| b.total_cmp(a));
                // Points that tie (a lone texel's, a flat run's) pass or fail together: of the
                // two counts either side of the tie, the nearer, taken half way to the next value
                // so that rounding as it is stored does not tip it.
                let k = (want * points.len() as f32).round() as usize;
                let scale = match k {
                    0 => 1.0,
                    k if points[k - 1] <= 0.0 => 255.0,
                    k => {
                        let v = points[k - 1];
                        let (above, at) = (points.partition_point(|&p| p > v), points.partition_point(|&p| p >= v));
                        let below = points.get(at).copied().unwrap_or(0.0);
                        let level = match (above > 0, k - above < at - k) {
                            (true, true) => (v + points[above - 1]) / 2.0,
                            _ => (v + below) / 2.0,
                        };
                        (cutoff as f32 / level.max(1e-3)).min(255.0)
                    }
                };
                scales.push((tx, ty, x1, y1, scale));
            }
        }
        for (tx, ty, x1, y1, scale) in scales {
            for y in ty..y1 {
                for x in tx..x1 {
                    let a = &mut px[(y * w + x) * 4 + 3];
                    *a = (*a as f32 * scale).round().min(255.0) as u8;
                }
            }
        }
    }
    levels
}

/// The alpha of the tile `(x0, y0, x1, y1)` of a `w`x`h` level, read bilinearly between texels
/// as the sampler reads it, at 4x4 points a texel, into `out`.
fn filtered(px: &[u8], w: usize, h: usize, (x0, y0, x1, y1): (usize, usize, usize, usize), out: &mut Vec<f32>) {
    let a = |x: isize, y: isize| {
        let (x, y) = (x.clamp(0, w as isize - 1) as usize, y.clamp(0, h as isize - 1) as usize);
        px[(y * w + x) * 4 + 3] as f32
    };
    out.clear();
    for y in y0..y1 {
        for x in x0..x1 {
            for sy in 0..4 {
                for sx in 0..4 {
                    // The point's place among the texel centres around it.
                    let (fx, fy) = (x as f32 + (sx as f32 + 0.5) / 4.0 - 0.5, y as f32 + (sy as f32 + 0.5) / 4.0 - 0.5);
                    let (ix, iy) = (fx.floor() as isize, fy.floor() as isize);
                    let (u, v) = (fx - ix as f32, fy - iy as f32);
                    let top = a(ix, iy) * (1.0 - u) + a(ix + 1, iy) * u;
                    let bottom = a(ix, iy + 1) * (1.0 - u) + a(ix + 1, iy + 1) * u;
                    out.push(top * (1.0 - v) + bottom * v);
                }
            }
        }
    }
}

/// The share of [`filtered`]'s points of a tile whose alpha reaches `cutoff`.
fn filtered_share(px: &[u8], w: usize, h: usize, tile: (usize, usize, usize, usize), cutoff: u8) -> f32 {
    let mut points = Vec::new();
    filtered(px, w, h, tile, &mut points);
    points.iter().filter(|&&a| a >= cutoff as f32).count() as f32 / points.len().max(1) as f32
}

/// The side, in texels of its own level, of the tiles [`mips_keeping`] keeps the coverage of.
const KEEP_TILE: usize = 4;

/// A texture uploaded with [`mips_keeping`]'s levels.
fn upload_texture_keeping(device: &wgpu::Device, queue: &wgpu::Queue, t: &Texture, cutoff: u8) -> wgpu::TextureView {
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
    let levels = mips_keeping(t, cutoff);
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
fn sky_faces(device: &wgpu::Device, queue: &wgpu::Queue, sky: Option<&Sky>) -> wgpu::TextureView {
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
    texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    })
}

/// The two skies the backdrop crossfades between (MD26), and the uniform saying how far.
fn sky_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    faces: &[wgpu::TextureView; 2],
    mix: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sky"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&faces[0]),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&faces[1]),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: mix.as_entire_binding(),
            },
        ],
    })
}

/// A ground's pages as one array (mip-mapped), the page each leaf takes as an image a texel a leaf
/// (its number plus one in red, 0 for none), and the detail map, or a clear texel that leaves the
/// pages as they are.
fn upload_pages(device: &wgpu::Device, queue: &wgpu::Queue, p: &Pages) -> [wgpu::TextureView; 3] {
    let (w, h) = p.pages.first().map_or((1, 1), |t| (t.width as u32, t.height as u32));
    let first = p.pages.first().map_or_else(|| Texture { width: 1, height: 1, rgba: vec![255; 4] }, Clone::clone);
    let levels = mips(&first).len() as u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ground pages"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: p.pages.len().max(1) as u32 },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (layer, page) in p.pages.iter().enumerate().chain(p.pages.is_empty().then_some((0, &first))) {
        // Every page of a terrain is one size; one that is not is resampled to the first's, nearest.
        let page = match (page.width as u32, page.height as u32) == (w, h) {
            true => page.clone(),
            false => Texture {
                width: w as usize,
                height: h as usize,
                rgba: (0..(w * h) as usize).flat_map(|i| page.sample(((i % w as usize) as f32 + 0.5) / w as f32, ((i / w as usize) as f32 + 0.5) / h as f32)).collect(),
            },
        };
        for (level, (lw, lh, px)) in mips(&page).iter().enumerate() {
            write_layer(queue, &texture, level as u32, layer as u32, (*lw, *lh), 4, px);
        }
    }
    let pages = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let leaves = Texture {
        width: p.across.max(1) as usize,
        height: p.along.max(1) as usize,
        rgba: match p.page_of.is_empty() {
            true => vec![0; 4],
            false => p.page_of.iter().flat_map(|page| [page.map_or(0, |n| n.min(254) as u8 + 1), 0, 0, 255]).collect(),
        },
    };
    let leaves = {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ground page of each leaf"),
            size: wgpu::Extent3d { width: leaves.width as u32, height: leaves.height as u32, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        write_layer(queue, &texture, 0, 0, (leaves.width as u32, leaves.height as u32), 4, &leaves.rgba);
        texture.create_view(&Default::default())
    };
    let clear = Texture { width: 1, height: 1, rgba: vec![0; 4] };
    let detail = upload_texture(device, queue, p.detail.as_ref().unwrap_or(&clear));
    [pages, leaves, detail]
}

/// Textures as one mip-mapped array the size of the first, each other one resampled to it, nearest;
/// at least two layers, so no backend takes it for a plain 2D texture.
fn upload_array(device: &wgpu::Device, queue: &wgpu::Queue, label: &str, layers: &[&Texture]) -> wgpu::TextureView {
    let blank = Texture { width: 1, height: 1, rgba: vec![0; 4] };
    let first = layers.first().copied().unwrap_or(&blank);
    let (w, h) = (first.width.max(1) as u32, first.height.max(1) as u32);
    let count = layers.len().max(2) as u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: count },
        mip_level_count: mips(&Texture { width: w as usize, height: h as usize, rgba: vec![0; (w * h * 4) as usize] }).len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for layer in 0..count as usize {
        let t = layers.get(layer).copied().unwrap_or(first);
        let t = match (t.width as u32, t.height as u32) == (w, h) && t.rgba.len() == (w * h * 4) as usize {
            true => t.clone(),
            false => Texture {
                width: w as usize,
                height: h as usize,
                rgba: (0..(w * h) as usize).flat_map(|i| t.sample(((i % w as usize) as f32 + 0.5) / w as f32, ((i / w as usize) as f32 + 0.5) / h as f32)).collect(),
            },
        };
        for (level, (lw, lh, px)) in mips(&t).iter().enumerate() {
            write_layer(queue, &texture, level as u32, layer as u32, (*lw, *lh), 4, px);
        }
    }
    texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() })
}

/// A world's paint (MD9) for its shader: every packed map's pages in one array, their leaf images
/// stacked in one picture (a leaf's red and green are its page's layer + 1, low byte first; 0 for
/// none), and their detail maps in a second array.
fn upload_world_pages(device: &wgpu::Device, queue: &wgpu::Queue, w: &WorldPaint, packed: &[Option<Packed>]) -> [wgpu::TextureView; 3] {
    let placed: Vec<(&Pages, Packed)> = w.tiles.iter().zip(packed).filter_map(|(t, p)| Some((t.pages.as_ref()?, (*p)?))).collect();
    let pages: Vec<&Texture> = placed.iter().flat_map(|(p, _)| p.pages.iter()).collect();
    let pages = upload_array(device, queue, "world ground pages", &pages);
    let width = placed.iter().map(|(p, _)| p.across).max().unwrap_or(1).max(1) as usize;
    let height = placed.iter().map(|(p, at)| at.row + p.along).max().unwrap_or(1).max(1) as usize;
    let mut rgba = vec![0u8; width * height * 4];
    for (p, at) in &placed {
        for lz in 0..p.along as usize {
            for lx in 0..p.across as usize {
                let entry = p.page_of[lz * p.across as usize + lx].map_or(0, |n| at.first_page + u32::from(n) + 1);
                let i = ((at.row as usize + lz) * width + lx) * 4;
                rgba[i..i + 4].copy_from_slice(&[(entry & 0xff) as u8, (entry >> 8) as u8, 0, 255]);
            }
        }
    }
    let leaves = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("world ground page of each leaf"),
        size: wgpu::Extent3d { width: width as u32, height: height as u32, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    write_layer(queue, &leaves, 0, 0, (width as u32, height as u32), 4, &rgba);
    let details: Vec<&Texture> = placed.iter().filter_map(|(p, _)| p.detail.as_ref()).collect();
    let details = upload_array(device, queue, "world ground detail", &details);
    [pages, leaves.create_view(&Default::default()), details]
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
        let mut limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        // And to as many array layers as it has: a world's ground pages (MD15: a disc map's baked
        // paint is 144 of them) need more than WebGL2's 256; a device with fewer draws the maps past
        // them in dirt ([`WorldPaint::pack`]).
        limits.max_texture_array_layers = limits.max_texture_array_layers.max(adapter.limits().max_texture_array_layers);
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
            entries: &[
                texture_entry(0, true, wgpu::TextureViewDimension::D2Array),
                sampler_entry(1),
                texture_entry(2, true, wgpu::TextureViewDimension::D2Array),
                uniform_entry(3, wgpu::ShaderStages::FRAGMENT),
            ],
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
        let sky_mix = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky mix"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&sky_mix, 0, bytemuck::cast_slice(&[0.0f32; 4]));
        let none = sky_faces(&device, &queue, None);
        let sky_views = [none.clone(), none];
        let sky_bind = sky_bind_group(&device, &sky_layout, &clamp_sampler, &sky_views, &sky_mix);

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
        let sprites = sprite::Sprites::new(&device, &queue, format, &clamp_sampler);
        let upscale = scale::Upscale::new(&device, format);
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
            sky_mix,
            sky_has: [false; 2],
            sky_share: 0.0,
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
            grove: None,
            groves: Vec::new(),
            graphics: Graphics::default(),
            span: 0.0,
            scratch: Vec::new(),
            batches: Vec::new(),
            flat: Vec::new(),
            blit: None,
            sprites,
            upscale,
            render_scale: 1.0,
            particles: None,
            glows: None,
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
                owner: SCENE,
            })
            .collect();
        self.grove = scene.ecosystem.clone();
        self.groves.clear();
    }

    /// More props drawn in every frame from now on, and a grove, as `owner`'s (MD15: a world tile's,
    /// already moved to where the tile is), until [`Renderer::drop_owner`] lets them go.
    pub fn add_props(&mut self, owner: u32, props: &[crate::scene::Placement], grove: Option<crate::eco::Grove>) {
        let lod = self.grove.as_ref().or(self.groves.first().map(|(_, g)| g)).map(|g| g.lod);
        self.props.extend(props.iter().map(|p| Prop { name: p.model.clone(), model: None, at: p.transform(), owner }));
        if let Some(mut g) = grove {
            if let Some(lod) = lod {
                g.lod = lod;
            }
            self.groves.push((owner, g));
        }
    }

    /// Stop drawing what `owner` added, and let go of its models and textures on the GPU: the
    /// models named, and every material painted with one of the textures. Their places stay, empty,
    /// so every other index still means what it did; a model asked for again is uploaded again.
    pub fn drop_owner(&mut self, owner: u32, models: &[String], textures: std::ops::Range<usize>) {
        self.props.retain(|p| p.owner != owner);
        self.groves.retain(|(o, _)| *o != owner);
        for name in models {
            if let Some(Some(id)) = self.model_ids.remove(name) {
                // Its place stays, so every later model keeps its index; its vertices go.
                let m = &mut self.models[id];
                m.vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("dropped model"),
                    size: 4,
                    usage: wgpu::BufferUsages::VERTEX,
                    mapped_at_creation: false,
                });
                m.groups.clear();
            }
        }
        if textures.is_empty() {
            return;
        }
        let gone: Vec<(MaterialKey, usize)> = self
            .material_ids
            .iter()
            .filter(|(k, _)| k.texture.is_some_and(|t| textures.contains(&t)))
            .map(|(k, &id)| (*k, id))
            .collect();
        if !gone.is_empty() {
            // A bind group holds its texture; one painted plain white in its place lets it go.
            let blank = self.blank_material();
            for (key, id) in gone {
                self.material_ids.remove(&key);
                self.materials[id].bind = blank.clone();
                self.materials[id].rest = None;
            }
        }
        self.textures.retain(|&(t, _), _| !textures.contains(&t));
    }

    /// A material bind group with no texture of its own, for a dropped material's place.
    fn blank_material(&self) -> wgpu::BindGroup {
        let uniform = MaterialUniform { constant: [128.0; 4], cutoff: 0.0, shading: 0, coverage: 0, below: 256.0 };
        let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dropped material"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dropped material"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.white) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    /// Where `owner`'s props are drawn now, in world feet: what a test reads to see a world tile's
    /// props stood at its offset (MD15).
    pub fn props_of(&self, owner: u32) -> Vec<crate::mat::V3> {
        self.props.iter().filter(|p| p.owner == owner).map(|p| p.at.point(crate::mat::V3 { x: 0.0, y: 0.0, z: 0.0 })).collect()
    }

    /// How the scene's plants change from model to picture from now on (P126's Plants switch).
    pub fn set_plant_lod(&mut self, lod: crate::eco::Lod) {
        if let Some(g) = &mut self.grove {
            g.lod = lod;
        }
        for (_, g) in &mut self.groves {
            g.lod = lod;
        }
    }

    /// The sky behind every view, or none: the fog's colour instead.
    pub fn set_sky(&mut self, sky: Option<&Sky>) {
        let has = sky.is_some_and(|s| s.faces.len() >= 6);
        let view = sky_faces(&self.device, &self.queue, sky.filter(|_| has));
        self.put_skies([view.clone(), view], [has; 2]);
        self.set_sky_mix(0.0);
    }

    /// Two skies to crossfade between (MD26), a combined world's across a join: `a` shown alone until
    /// [`Renderer::set_sky_mix`] gives `b` a share. A missing one is the fog's colour, as with
    /// [`Renderer::set_sky`].
    pub fn set_skies(&mut self, a: Option<&Sky>, b: Option<&Sky>) {
        let has = [a, b].map(|s| s.is_some_and(|s| s.faces.len() >= 6));
        let views = [sky_faces(&self.device, &self.queue, a.filter(|_| has[0])), sky_faces(&self.device, &self.queue, b.filter(|_| has[1]))];
        self.put_skies(views, has);
    }

    /// How much of the second sky [`Renderer::set_skies`] gave shows, 0 (none) to 1 (all of it): a
    /// uniform's worth, cheap to set every frame.
    pub fn set_sky_mix(&mut self, share: f32) {
        self.sky_share = share.clamp(0.0, 1.0);
        let has = self.sky_has.map(|h| if h { 1.0f32 } else { 0.0 });
        self.queue.write_buffer(&self.sky_mix, 0, bytemuck::cast_slice(&[self.sky_share, has[0], has[1], 0.0]));
    }

    fn put_skies(&mut self, views: [wgpu::TextureView; 2], has: [bool; 2]) {
        self.has_sky = has[0] || has[1];
        self.sky_has = has;
        self.sky_bind = sky_bind_group(&self.device, &self.sky_layout, &self.clamp_sampler, &views, &self.sky_mix);
        self.set_sky_mix(self.sky_share);
    }

    /// The ground: its chunks, and the track's paint for it — or, without paint, the one colour `dirt`
    /// (0..255) lit by the sun.
    ///
    /// Every chunk must be the same size, a power of two, with all its corners ([`GroundChunk`]).
    pub fn set_ground(&mut self, chunks: &[GroundChunk], paint: Option<&Splat>, dirt: [f32; 3]) -> Result<(), String> {
        self.set_ground_painted(chunks, paint.map_or(GroundPaint::Bare, GroundPaint::Splat), dirt)
    }

    /// [`Renderer::set_ground`], with the ground painted by either of its two kinds of paint (MD4): a
    /// freeride track's splat, or a mod track's own terrain pages. `dirt` goes where neither reaches.
    pub fn set_ground_painted(&mut self, chunks: &[GroundChunk], paint: GroundPaint, dirt: [f32; 3]) -> Result<(), String> {
        let cells = chunks.first().map_or(1, |c| c.cells);
        if chunks.is_empty() {
            self.ground = None;
            return Ok(());
        }
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for v in chunks.iter().flat_map(|c| &c.vertices) {
            lo = v3(lo.x.min(v.pos[0]), lo.y.min(v.pos[1]), lo.z.min(v.pos[2]));
            hi = v3(hi.x.max(v.pos[0]), hi.y.max(v.pos[1]), hi.z.max(v.pos[2]));
        }
        self.set_ground_streamed(cells, paint, dirt, if lo.x <= hi.x { hi.sub(lo).len() } else { 0.0 })?;
        self.add_ground_piece(0, chunks)
    }

    /// A ground that comes and goes a piece at a time (MD17): the paint and its pipeline as
    /// [`Renderer::set_ground_painted`] makes them, no chunks yet, and `span` as the distance between
    /// the ground's two farthest points (the fog of [`Graphics::Full`] reaches across it). Chunks of
    /// `cells` cells come with [`Renderer::add_ground_piece`] and go with
    /// [`Renderer::drop_ground_piece`].
    pub fn set_ground_streamed(&mut self, cells: u32, paint: GroundPaint, dirt: [f32; 3], span: f32) -> Result<(), String> {
        let pages = match paint {
            GroundPaint::Pages(p) => Some(p),
            _ => None,
        };
        let world = match paint {
            GroundPaint::World(w) => {
                let limits = self.device.limits();
                Some((w, w.pack(limits.max_texture_array_layers, limits.max_texture_dimension_2d)))
            }
            _ => None,
        };
        let paint = match paint {
            GroundPaint::Splat(s) => Some(s),
            _ => None,
        };
        if !cells.is_power_of_two() {
            return Err(format!("ground chunks of {cells} cells: not a power of two"));
        }
        self.ground = None;
        self.span = span;
        let patterns: Vec<Vec<u32>> = (0..=lod_levels(cells))
            .flat_map(|level| (0..16).map(move |mask| lod_indices(cells, level, sides(mask))))
            .collect();
        let vertex_buffer = ground_vertex_buffer(&self.device, cells, 1);

        // The paint is a little program (`splat.rs`); here it is written out as the shader's own code.
        let layers: &[Texture] = paint.map_or(&[], |p| &p.textures);
        let max = self.device.limits().max_sampled_textures_per_shader_stage as usize;
        // The roads and ruts painted in after the passes (V67) are two more.
        let meshes = paint.and_then(|p| p.meshes.as_ref()).filter(|_| pages.is_none() && world.is_none());
        let meshes = meshes.map(|m| ground_mesh::GpuMeshes::new(&self.device, &self.queue, &self.sampler, m));
        if layers.len() + 1 + if meshes.is_some() { 2 } else { 0 } > max {
            return Err(format!(
                "the ground's paint has {} textures and this device samples {max} at once",
                layers.len()
            ));
        }
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ground"),
            source: wgpu::ShaderSource::Wgsl(match (pages, &world) {
                (Some(p), _) => ground_pages_wgsl(p, dirt),
                (None, Some((w, packed))) => ground_world_wgsl(w, packed),
                (None, None) => ground_wgsl(paint, dirt),
            }
            .into()),
        });
        let mut entries = vec![
            sampler_entry(0),
            sampler_entry(1),
            texture_entry(2, true, wgpu::TextureViewDimension::D2),
        ];
        for i in 0..layers.len() {
            entries.push(texture_entry(3 + i as u32, true, wgpu::TextureViewDimension::D2));
        }
        if pages.is_some() {
            entries.push(texture_entry(3, true, wgpu::TextureViewDimension::D2Array));
            entries.push(texture_entry(4, true, wgpu::TextureViewDimension::D2));
            entries.push(texture_entry(5, true, wgpu::TextureViewDimension::D2));
        }
        if world.is_some() {
            entries.push(texture_entry(3, true, wgpu::TextureViewDimension::D2Array));
            entries.push(texture_entry(4, true, wgpu::TextureViewDimension::D2));
            entries.push(texture_entry(5, true, wgpu::TextureViewDimension::D2Array));
        }
        if meshes.is_some() {
            entries.extend(ground_mesh::layout_entries(3 + layers.len() as u32));
        }
        let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ground"),
            entries: &entries,
        });
        let shadow = match paint.and_then(|p| p.shadow.as_ref()) {
            Some(s) => upload_shadow(&self.device, &self.queue, s),
            None => self.white.clone(),
        };
        let mut views: Vec<wgpu::TextureView> = layers.iter().map(|t| upload_texture(&self.device, &self.queue, t)).collect();
        if let Some(p) = pages {
            views.extend(upload_pages(&self.device, &self.queue, p));
        }
        if let Some((w, packed)) = &world {
            views.extend(upload_world_pages(&self.device, &self.queue, w, packed));
        }
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
        if let Some(m) = &meshes {
            let first = 3 + layers.len() as u32;
            let [near, far] = m.views();
            bind_entries.push(wgpu::BindGroupEntry { binding: first, resource: wgpu::BindingResource::TextureView(near) });
            bind_entries.push(wgpu::BindGroupEntry { binding: first + 1, resource: wgpu::BindingResource::TextureView(far) });
            bind_entries.push(wgpu::BindGroupEntry { binding: first + 2, resource: m.at.as_entire_binding() });
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
            pages: vec![vertex_buffer],
            per_page: ((self.device.limits().max_buffer_size / (u64::from((cells + 1) * (cells + 1)) * std::mem::size_of::<GroundVertex>() as u64))
                .min(u64::from(u32::MAX / ((cells + 1) * (cells + 1))))) as u32,
            capacity: 1,
            free: vec![0],
            hashes: HashMap::new(),
            levels: Vec::new(),
            chunks: Vec::new(),
            cells,
            across: 0,
            along: 0,
            slots: Vec::new(),
            patterns,
            views: Vec::new(),
            pipeline,
            bind,
            meshes,
        });
        Ok(())
    }

    /// Piece `piece` of the ground (MD17): its chunks, numbered across and along within the piece,
    /// in place of whatever the piece had. A piece's chunks are stitched to each other and to no
    /// other piece's. Needs a ground set first ([`Renderer::set_ground_streamed`]).
    pub fn add_ground_piece(&mut self, piece: u32, chunks: &[GroundChunk]) -> Result<(), String> {
        let Some(g) = &mut self.ground else { return Err("no ground to add a piece to".into()) };
        let cells = g.cells;
        let corners = ((cells + 1) * (cells + 1)) as usize;
        if let Some(c) = chunks.iter().find(|c| c.cells != cells || c.vertices.len() != corners) {
            return Err(format!("a ground chunk of {} cells and {} corners among chunks of {cells}", c.cells, c.vertices.len()));
        }
        g.forget(piece);
        g.hashes.insert(piece, chunks_hash(chunks));
        while g.free.len() < chunks.len() {
            let need = (chunks.len() - g.free.len()) as u32;
            let last = g.capacity - (g.pages.len() as u32 - 1) * g.per_page;
            let grown = if last < g.per_page {
                // The last page with room for twice as many, its old corners copied over.
                let size = (last * 2).max(last + need).min(g.per_page);
                let bigger = ground_vertex_buffer(&self.device, cells, size);
                let old = g.pages.pop().expect("a page");
                let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("ground grows") });
                encoder.copy_buffer_to_buffer(&old, 0, &bigger, 0, old.size());
                self.queue.submit([encoder.finish()]);
                g.pages.push(bigger);
                size - last
            } else {
                let size = need.next_power_of_two().min(g.per_page);
                g.pages.push(ground_vertex_buffer(&self.device, cells, size));
                size
            };
            g.free.extend((g.capacity..g.capacity + grown).rev());
            g.capacity += grown;
        }
        let stride = (corners * std::mem::size_of::<GroundVertex>()) as u64;
        for c in chunks {
            let place = g.free.pop().expect("room was made");
            let (page, at) = (place / g.per_page, place % g.per_page);
            self.queue.write_buffer(&g.pages[page as usize], u64::from(at) * stride, bytemuck::cast_slice(&c.vertices));
            g.chunks.push(GpuChunk::of(c, cells, page, at * corners as u32, piece));
        }
        g.regrid();
        Ok(())
    }

    /// Take piece `piece`'s chunks out of the ground ([`Renderer::add_ground_piece`]).
    pub fn drop_ground_piece(&mut self, piece: u32) {
        if let Some(g) = &mut self.ground {
            g.forget(piece);
            g.hashes.remove(&piece);
            g.regrid();
        }
    }

    /// Piece `piece`'s chunks as the ground holds them, as one number: every corner, hole, place and
    /// bound, so two grounds that hash the same draw the piece the same. `None` if it is not held.
    pub fn ground_piece_hash(&self, piece: u32) -> Option<u64> {
        self.ground.as_ref()?.hashes.get(&piece).copied()
    }

    /// How many chunks the ground holds now, and how many it has room for.
    pub fn ground_chunks(&self) -> (usize, usize) {
        self.ground.as_ref().map_or((0, 0), |g| (g.chunks.len(), g.capacity as usize))
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
        self.grove = None;
        self.groves.clear();
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

    /// [`Renderer::upload_all`] a slice at a time: `names` of `assets`' models, which uploaded in
    /// [`Renderer::upload_all`]'s order (sorted) number the same as it does.
    pub fn upload_models(&mut self, assets: &Assets, names: &[&String]) {
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
        let keep = if surface.keep_coverage { surface.cutoff } else { 0 };
        let view = match texture {
            Some(t) => self
                .textures
                .entry((t, keep))
                .or_insert_with(|| upload_texture_keeping(&self.device, &self.queue, &assets.textures[t], keep))
                .clone(),
            None => self.white.clone(),
        };
        let constant = match surface.shading {
            Shading::Constant(c) => c,
            _ => [128.0; 4],
        };
        let split = surface.depth_write && surface.depth_cutoff > surface.cutoff;
        let uniform = |cutoff: u8, below: f32| MaterialUniform {
            constant,
            cutoff: cutoff as f32,
            shading: key.shading.0 as u32,
            coverage: key.keep_coverage as u32,
            below,
        };
        let buffer = |uniform: MaterialUniform| {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("material"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };
        let (solid, rest) = match split {
            true => (
                buffer(uniform(surface.depth_cutoff, 256.0)),
                Some(buffer(uniform(surface.cutoff, surface.depth_cutoff as f32))),
            ),
            false => (buffer(uniform(surface.cutoff, 256.0)), None),
        };
        // The few materials that clamp (palm fronds, bark, cut-outs) get a sampler of their own;
        // everything else shares the repeating one.
        // A cut-out that keeps its coverage (leaves, V58) reads one level, not a mix of two: each
        // level covers what the full size does, but two mixed (a fine level's thin twigs with a
        // coarse level's broad, fainter ones) fall under the cutoff between them.
        let clamped = (key.clamp != [false; 2] || key.keep_coverage).then(|| {
            let mode = |on: bool| if on { wgpu::AddressMode::ClampToEdge } else { wgpu::AddressMode::Repeat };
            self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("material clamp"),
                address_mode_u: mode(key.clamp[0]),
                address_mode_v: mode(key.clamp[1]),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: if key.keep_coverage { wgpu::MipmapFilterMode::Nearest } else { wgpu::MipmapFilterMode::Linear },
                ..Default::default()
            })
        });
        let bind = |buffer: &wgpu::Buffer| self.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        let rest = rest.map(|b| (bind(&b), PipelineKey { depth_write: false, ..key.pipeline }));
        let bind = bind(&solid);
        for pipeline in std::iter::once(key.pipeline).chain(rest.as_ref().map(|r| r.1)) {
            self.pipeline(pipeline);
            self.pipeline(PipelineKey { ghost: true, ..pipeline });
        }
        self.materials.push(GpuMaterial {
            bind,
            pipeline: key.pipeline,
            rest,
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
                            5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Uint32x2, 10 => Float32
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
        let names: Vec<String> = self
            .grove
            .iter()
            .chain(self.groves.iter().map(|(_, g)| g))
            .flat_map(|g| g.parts())
            .filter(|n| !self.model_ids.contains_key(*n))
            .cloned()
            .collect();
        for name in names {
            self.model(assets, &name);
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
        let mut origins = Vec::with_capacity(views.len());
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
            // Drawn relative to the eye (MD15): the matrix looks from the origin, and every point
            // and copy is moved by the eye before it, so far from the map's origin nothing jitters.
            let from_origin = Camera { eye: v3(0.0, 0.0, 0.0), at: camera.at.sub(camera.eye), ..*camera };
            origins.push(camera.eye);
            let u = ViewUniform {
                vp: reversed(view_projection(&from_origin, w, h, far)).0,
                eye: [camera.eye.x, camera.eye.y, camera.eye.z, 0.0],
                sun: [light.sun.x, light.sun.y, light.sun.z, 0.0],
                sun_color: [light.sun_color[0], light.sun_color[1], light.sun_color[2], 0.0],
                ambient: [light.ambient[0], light.ambient[1], light.ambient[2], 0.0],
                fog_color: [light.fog_color[0] as f32, light.fog_color[1] as f32, light.fog_color[2] as f32, 0.0],
                fog: [fog.near, fog.far, if self.has_sky { 1.0 } else { 0.0 }, fog.max],
                forward: [forward.x, forward.y, forward.z, 0.0],
                right: [right.x * half_w, right.y * half_w, right.z * half_w, 0.0],
                up: [up.x * half_h, up.y * half_h, up.z * half_h, 0.0],
                origin: [camera.eye.x, camera.eye.y, camera.eye.z, 0.0],
                _pad: [0.0; 8],
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
                        cover: 1.0,
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
                self.scratch.push((key, ghost, InstanceRaw { m: a.at.0, palette, cover: 1.0 }));
            }
            // The trees and bushes: each a model near, a picture far and both between (V48). Under
            // the original's rule the pictures come in see-through as the actors' ghosts are drawn;
            // under the smooth one (V55) model and picture each draw their share of a screen
            // pattern, so they batch with every other copy of their model and need no sorting.
            for grove in self.grove.iter().chain(self.groves.iter().map(|(_, g)| g)) {
                let species = grove.ecosystem.species.len();
                let ids: Vec<Option<usize>> = grove.parts().map(|n| self.model_ids.get(n).copied().flatten()).collect();
                let mut plants = Vec::new();
                grove.draws(camera, crate::eco::horizontal_fov(camera, w as f32 / h.max(1) as f32), cull.reach, &mut plants);
                for d in &plants {
                    let Some(m) = ids[d.part_index(species)] else { continue };
                    let model = &self.models[m];
                    // The model's reach from its origin, at the plant's largest scale.
                    let row = |r: usize| v3(d.at.0[r][0], d.at.0[r][1], d.at.0[r][2]);
                    let centre = row(3);
                    let radius = model.radius * row(0).len().max(row(1).len()).max(row(2).len());
                    if !cull.sees(centre, radius) || !cull.big_enough(centre, radius, PROP_PIXELS) {
                        continue;
                    }
                    let ghost = if d.alpha < 1.0 { d.alpha.to_bits() } else { 0 };
                    let key = ((v as u64) << 48) | ((m as u64) << 16);
                    let inst = InstanceRaw { m: d.at.0, palette: [model.bind_palette, model.nodes], cover: d.cover };
                    self.scratch.push((key, ghost, inst));
                }
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
            let mut inst = *inst;
            let o = origins[view];
            inst.m[3][0] -= o.x;
            inst.m[3][1] -= o.y;
            inst.m[3][2] -= o.z;
            self.flat.push(inst);
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

        let place = PlaceUniform {
            origin: [target.frame[0] as f32, target.frame[1] as f32],
            scale: target.scale.max(1) as f32,
            _pad: 0.0,
        };
        let sprites = self.sprites.prepare(&self.device, &self.queue, place, target.size, frame.sprites);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        // The roads and ruts under the first view, drawn sharp again if it has moved (V67).
        if let (Some(m), Some(v)) = (self.ground.as_mut().and_then(|g| g.meshes.as_mut()), views.first()) {
            m.follow(&self.queue, &mut encoder, v.camera.eye);
        }
        let scale = scale::quantise(self.render_scale);
        let [fx, fy, fw, fh] = target.frame;
        if scale < 1.0 && fw > 0 && fh > 0 {
            // The views into the low texture, each view's edges scaled and rounded so they still meet;
            // then the low texture stretched over the frame, and the overlay over that at full size.
            let size = scale::scaled((fw, fh), scale);
            let (low, low_depth) = self.upscale.low(&self.device, self.format, &self.clamp_sampler, size);
            {
                let mut pass = begin_pass(&mut encoder, &low, &low_depth, "views");
                let (kx, ky) = (size.0 as f32 / fw as f32, size.1 as f32 / fh as f32);
                let s = target.scale;
                let rect = |[x, y, w, h]: [u32; 4]| {
                    let (x0, y0) = (((x * s) as f32 * kx).round() as u32, ((y * s) as f32 * ky).round() as u32);
                    let (x1, y1) = ((((x + w) * s) as f32 * kx).round() as u32, (((y + h) * s) as f32 * ky).round() as u32);
                    [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
                };
                self.draw_views(&mut pass, views, &ground_counts, rect, size, &mut drawn);
            }
            let depth = self.depth_for(target.size);
            let mut pass = begin_pass(&mut encoder, target.view, &depth, "frame");
            pass.set_scissor_rect(fx, fy, fw.min(target.size.0.saturating_sub(fx)), fh.min(target.size.1.saturating_sub(fy)));
            self.upscale.draw(&self.queue, &mut pass, target.frame);
            drawn.draws += 1;
            self.draw_overlay(&mut pass, target, frame, sprites, &mut drawn);
        } else {
            let depth = self.depth_for(target.size);
            let mut pass = begin_pass(&mut encoder, target.view, &depth, "frame");
            let s = target.scale;
            let rect = |[x, y, w, h]: [u32; 4]| [fx + x * s, fy + y * s, w * s, h * s];
            self.draw_views(&mut pass, views, &ground_counts, rect, target.size, &mut drawn);
            self.draw_overlay(&mut pass, target, frame, sprites, &mut drawn);
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
                runs: Vec::new(),
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
        if key != g.views[v].key {
            // The stitchings of chunks with holes this key asks for that are not made yet.
            let (cells, chunks) = (g.cells, &mut g.chunks);
            for (&c, &k) in g.slots.iter().flatten().zip(&key) {
                if k == u32::MAX {
                    continue;
                }
                if let Some((missing, made)) = &mut chunks[c].own {
                    made[(k % 16) as usize].get_or_insert_with(|| {
                        let row = cells as usize;
                        lod_indices_where(cells, 0, sides((k % 16) as usize), |i, j| !missing[j as usize * row + i as usize])
                    });
                }
            }
            let gv = &mut g.views[v];
            gv.indices.clear();
            gv.runs.clear();
            for page in 0..g.pages.len() as u32 {
                let first = gv.indices.len() as u32;
                let chunks = g.slots.iter().flatten();
                for (&c, &k) in chunks.zip(&key) {
                    let chunk = &g.chunks[c];
                    if k == u32::MAX || chunk.page != page {
                        continue;
                    }
                    let pattern = match &chunk.own {
                        Some((_, made)) => made[(k % 16) as usize].as_ref().expect("made above"),
                        None => &g.patterns[k as usize],
                    };
                    gv.indices.extend(pattern.iter().map(|i| i + chunk.base));
                }
                if gv.indices.len() as u32 > first {
                    gv.runs.push((page, first, gv.indices.len() as u32 - first));
                }
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
        g.views[v].count
    }

    /// Every view into `pass`, each at `rect` of its frame rectangle, inside a target of `bounds`.
    #[allow(clippy::too_many_arguments)]
    fn draw_views(
        &self,
        pass: &mut wgpu::RenderPass,
        views: &[View],
        ground_counts: &[u32],
        rect: impl Fn([u32; 4]) -> [u32; 4],
        bounds: (u32, u32),
        drawn: &mut Drawn,
    ) {
        let stride = std::mem::size_of::<InstanceRaw>() as u64;
        let mut b = 0;
        for (v, view) in views.iter().enumerate() {
            let [x, y, w, h] = rect(view.rect);
            if w == 0 || h == 0 || x + w > bounds.0 || y + h > bounds.1 {
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
                pass.set_index_buffer(gv.buffer.slice(..), wgpu::IndexFormat::Uint32);
                for &(page, first, count) in &gv.runs {
                    pass.set_vertex_buffer(0, ground.pages[page as usize].slice(..));
                    pass.draw_indexed(first..first + count, 0, 0..1);
                    drawn.draws += 1;
                }
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
                        if let Some((rest, key)) = &material.rest {
                            pass.set_pipeline(&self.pipelines[key]);
                            pass.set_bind_group(0, &self.view_bind, &offset);
                            pass.set_bind_group(1, rest, &[]);
                            (current, bound) = (Some(*key), None);
                            pass.draw(r.start..r.start + r.count, 0..batch.count);
                            drawn.draws += 1;
                        }
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
                    if let Some((rest, key)) = &material.rest {
                        let key = PipelineKey { ghost: true, ..*key };
                        pass.set_pipeline(&self.pipelines[&key]);
                        pass.set_bind_group(0, &self.view_bind, &offset);
                        pass.set_bind_group(1, rest, &[]);
                        (current, bound) = (Some(key), None);
                        pass.draw(r.start..r.start + r.count, 0..batch.count);
                        drawn.draws += 1;
                    }
                }
            }
            // The particles, over the models (P58).
            if let Some(particles) = &self.particles
                && particles.draw(pass, &self.view_bind, &offset)
            {
                drawn.draws += 1;
            }
            // The start zones' and markers' glows, over the particles (V51).
            if let Some(glows) = &self.glows {
                drawn.draws += glows.draw(pass, &self.view_bind, &offset, v);
            }
        }
    }

    /// The overlay's patches, then the sprites, over the whole frame: the sprites last, as the pause
    /// is one of them and goes over everything, a debug HUD's patch too.
    fn draw_overlay(&self, pass: &mut wgpu::RenderPass, target: &Target, frame: &Scene3, sprites: bool, drawn: &mut Drawn) {
        let [x, y, w, h] = target.frame;
        pass.set_viewport(0.0, 0.0, target.size.0 as f32, target.size.1 as f32, 0.0, 1.0);
        pass.set_scissor_rect(x, y, w.min(target.size.0.saturating_sub(x)), h.min(target.size.1.saturating_sub(y)));
        if let Some(overlay) = &self.overlay
            && !frame.overlay.is_empty()
        {
            for pipeline in [&self.overlay_mul, &self.overlay_add] {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &overlay.bind, &[]);
                pass.draw(0..3, 0..1);
                drawn.draws += 1;
            }
        }
        if sprites {
            drawn.draws += self.sprites.draw(pass);
        }
    }

    /// Sprites alone on `target`, black round and under them (V30): the front end's menus, which
    /// have no world under them.
    pub fn draw_sprites(&mut self, target: &Target, sprites: &[Sprite]) {
        let place = PlaceUniform {
            origin: [target.frame[0] as f32, target.frame[1] as f32],
            scale: target.scale.max(1) as f32,
            _pad: 0.0,
        };
        let any = self.sprites.prepare(&self.device, &self.queue, place, target.size, sprites);
        let depth = self.depth_for(target.size);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = begin_pass(&mut encoder, target.view, &depth, "sprites");
            let [x, y, w, h] = target.frame;
            pass.set_scissor_rect(x, y, w.min(target.size.0.saturating_sub(x)), h.min(target.size.1.saturating_sub(y)));
            if any {
                self.sprites.draw(&mut pass);
            }
        }
        self.queue.submit([encoder.finish()]);
    }

    /// The pictures [`Sprite::image`] counts into, uploaded now and kept: the HUD's art.
    pub fn set_images(&mut self, images: &[&Texture]) {
        self.sprites.set_images(&self.device, &self.queue, &self.clamp_sampler, images);
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
                kept: Vec::new(),
            });
        }
        let overlay = self.overlay.as_mut().expect("made above");
        let place = PlaceUniform {
            origin: [target.frame[0] as f32, target.frame[1] as f32],
            scale: target.scale.max(1) as f32,
            _pad: 0.0,
        };
        self.queue.write_buffer(&overlay.place, 0, bytemuck::bytes_of(&place));
        if overlay.kept.as_slice() == patches {
            return;
        }
        overlay.kept = patches.to_vec();
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
        self.blit_again(target);
    }

    /// Put the frame [`Renderer::blit`] last put on a target on `target` again, without sending it
    /// (V30): a menu standing still at 4K is 33 megabytes a frame less to convert and upload.
    pub fn blit_again(&mut self, target: &Target) {
        let Some(blit) = self.blit.as_ref() else { return };
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
    // The roads and ruts drawn into the paint after its passes (V67).
    let meshed = paint.is_some_and(|p| p.meshes.is_some());
    if meshed {
        s.push_str(&ground_mesh::wgsl_bindings(3 + layers as u32));
    }
    s.push_str(GROUND_VS_WGSL);
    s.push_str(
        "@fragment fn fs(i: Out) -> @location(0) vec4f {\n\
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
    if meshed {
        s.push_str(ground_mesh::APPLY_WGSL);
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

/// The ground's vertex shader, the same for every paint: the world position goes on to the paint,
/// and the sun on the stored normal and the fog are worked out per corner.
const GROUND_VS_WGSL: &str = r#"struct Out { @builtin(position) clip: vec4f, @location(0) world: vec3f, @location(1) sunlit: vec3f, @location(2) fog: f32 }
@vertex fn vs(@location(0) pos: vec3f, @location(1) normal: vec3f) -> Out {
  var o: Out;
  o.clip = to_clip(pos);
  o.world = pos;
  let l = length(normal);
  let n = select(normal, normal / l, l > 0.0);
  o.sunlit = view.ambient.rgb + view.sun_color.rgb * max(-dot(n, view.sun.xyz), 0.0);
  o.fog = fog_at(pos);
  return o;
}
"#;

/// The ground's shader for a mod track's terrain pages (MD4, [`Pages`]): the page each leaf takes
/// looked up per pixel, at the place on it the original's mapping gives, the detail map blended over
/// it, and the dirt colour where no page reaches. The page coordinates jump at every page's edge, so
/// the mip level comes from the world position, which does not.
fn ground_pages_wgsl(p: &Pages, dirt: [f32; 3]) -> String {
    let f = |v: f32| format!("{v:?}");
    let mut s = String::from(VIEW_WGSL);
    s.push_str(
        "@group(1) @binding(0) var tiled: sampler;\n\
         @group(1) @binding(1) var clamped: sampler;\n\
         @group(1) @binding(2) var shadow: texture_2d<f32>;\n\
         @group(1) @binding(3) var pages: texture_2d_array<f32>;\n\
         @group(1) @binding(4) var page_of: texture_2d<f32>;\n\
         @group(1) @binding(5) var detail: texture_2d<f32>;\n",
    );
    s.push_str(GROUND_VS_WGSL);
    s.push_str(&format!(
        "@fragment fn fs(i: Out) -> @location(0) vec4f {{
  let samples = i.world.xz / {spacing};
  let leaf = vec2i(floor(samples / 16.0));
  let leaves = vec2i({across}, {along});
  let on = all(leaf >= vec2i(0)) && all(leaf < leaves);
  let entry = i32(round(textureLoad(page_of, clamp(leaf, vec2i(0), leaves - vec2i(1)), 0).r * 255.0));
  let page = select(0, entry - 1, on && entry > 0);
  let local = (vec2f(leaf & vec2i(3)) * 16.0 + samples - vec2f(leaf) * 16.0) / 64.0;
  let along_page = samples * ({inner} / 64.0);
  let t = textureSampleGrad(pages, tiled, {border} + {inner} * local, page, dpdx(along_page), dpdy(along_page)).rgb * 255.0;
  let d = textureSample(detail, tiled, i.world.xz / {repeat});
  let painted = t + (d.rgb * 255.0 - t) * (d.a * 255.0 * 0.5 / 128.0);
  let texel = select(vec3f(255.0), painted, on && entry > 0);
  let lit = select(i.sunlit * vec3f({r}, {g}, {b}) / 255.0, i.sunlit, on && entry > 0);
  let out = clamp(texel * lit * (1.0 - i.fog) + view.fog_color.rgb * i.fog, vec3f(0.0), vec3f(255.0));
  return vec4f(out / 255.0, 1.0);
}}
",
        spacing = f(p.spacing),
        across = p.across.max(1),
        along = p.along.max(1),
        border = f(1.0 / 32.0),
        inner = f(15.0 / 16.0),
        repeat = f(p.detail_repeat),
        r = f(dirt[0]),
        g = f(dirt[1]),
        b = f(dirt[2]),
    ));
    s
}

/// The ground's shader for a world (MD9, [`WorldPaint`]): each map's pages over the cells it owns,
/// as [`ground_pages_wgsl`] draws one map's, and on a join every near map's read mirrored back
/// inside it and blended by [`crate::terrain::Regions::weights`], with the dirt's trace. Each map's
/// numbers are written into the shader, one block of code a map.
fn ground_world_wgsl(w: &WorldPaint, packed: &[Option<Packed>]) -> String {
    let f = |v: f32| format!("{v:?}");
    let mut s = String::from(VIEW_WGSL);
    s.push_str(
        "@group(1) @binding(0) var tiled: sampler;\n\
         @group(1) @binding(1) var clamped: sampler;\n\
         @group(1) @binding(2) var shadow: texture_2d<f32>;\n\
         @group(1) @binding(3) var pages: texture_2d_array<f32>;\n\
         @group(1) @binding(4) var page_of: texture_2d<f32>;\n\
         @group(1) @binding(5) var detail: texture_2d_array<f32>;\n",
    );
    s.push_str(GROUND_VS_WGSL);
    s.push_str(&format!(
        "fn inside(p: vec2f, r: vec4f) -> bool {{ return all(p >= r.xy) && all(p < r.zw); }}
fn reach(p: vec2f, r: vec4f) -> f32 {{
  let d = max(length(max(max(r.xy - p, p - r.zw), vec2f(0.0))), 0.001);
  let w = max(1.0 / d - 1.0 / {reach}, 0.0);
  return w * w;
}}
fn mirror1(v: f32, lo: f32, hi: f32) -> f32 {{
  let top = max(hi - 0.001, lo);
  return select(select(v, max(top - (v - top), lo), v > top), min(lo + (lo - v), top), v < lo);
}}
fn mirror(p: vec2f, r: vec4f) -> vec2f {{ return vec2f(mirror1(p.x, r.x, r.z), mirror1(p.y, r.y, r.w)); }}
",
        reach = f(crate::terrain::BLEND_REACH),
    ));
    let dirt = format!("vec3f({}, {}, {})", f(w.dirt[0]), f(w.dirt[1]), f(w.dirt[2]));
    let rect = |i: usize| {
        let r = w.regions.owned[i];
        format!("vec4f({}, {}, {}, {})", f(r[0]), f(r[1]), f(r[2]), f(r[3]))
    };
    // One map's colour at a world point read in its rectangle.
    for (i, t) in w.tiles.iter().enumerate() {
        let body = match (&t.pages, packed.get(i).copied().flatten()) {
            (Some(p), Some(at)) => {
                let detail = match (at.detail, &p.detail) {
                    (Some(layer), Some(_)) => format!(
                        "let d = textureSampleGrad(detail, tiled, q / {repeat}, {layer}, gx / {repeat}, gy / {repeat});
  let painted = t + (d.rgb * 255.0 - t) * (d.a * 255.0 * 0.5 / 128.0);",
                        repeat = f(p.detail_repeat),
                    ),
                    _ => "let painted = t;".to_string(),
                };
                format!(
                    "let q = mirror(xz, {rect}) - vec2f({ax}, {az});
  let samples = q / {spacing};
  let leaf = vec2i(floor(samples / 16.0));
  let leaves = vec2i({across}, {along});
  let on = all(leaf >= vec2i(0)) && all(leaf < leaves);
  let e = textureLoad(page_of, clamp(leaf, vec2i(0), leaves - vec2i(1)) + vec2i(0, {row}), 0);
  let entry = i32(round(e.r * 255.0)) + 256 * i32(round(e.g * 255.0));
  let page = select(0, entry - 1, on && entry > 0);
  let local = (vec2f(leaf & vec2i(3)) * 16.0 + samples - vec2f(leaf) * 16.0) / 64.0;
  let k = {inner} / (64.0 * {spacing});
  let t = textureSampleGrad(pages, tiled, {border} + {inner} * local, page, gx * k, gy * k).rgb * 255.0;
  {detail}
  return select({dirt}, painted, on && entry > 0);",
                    rect = rect(i),
                    ax = f(t.at[0]),
                    az = f(t.at[1]),
                    spacing = f(p.spacing),
                    across = p.across.max(1),
                    along = p.along.max(1),
                    row = at.row,
                    border = f(1.0 / 32.0),
                    inner = f(15.0 / 16.0),
                )
            }
            _ => format!("return {dirt};"),
        };
        s.push_str(&format!("fn tile{i}(xz: vec2f, gx: vec2f, gy: vec2f) -> vec3f {{\n  {body}\n}}\n"));
    }
    let n = w.tiles.len();
    s.push_str("@fragment fn fs(i: Out) -> @location(0) vec4f {\n  let xz = i.world.xz;\n  let gx = dpdx(xz);\n  let gy = dpdy(xz);\n  var owner = -1;\n");
    for i in (0..n).rev() {
        s.push_str(&format!("  if inside(xz, {}) {{ owner = {i}; }}\n", rect(i)));
    }
    s.push_str(&format!("  var texel = {dirt};\n  if owner >= 0 {{\n"));
    for i in 0..n {
        s.push_str(&format!("    if owner == {i} {{ texel = tile{i}(xz, gx, gy); }}\n"));
    }
    s.push_str(&format!("  }} else {{\n    var total = {};\n", f(crate::terrain::DIRT_WEIGHT)));
    for i in 0..n {
        s.push_str(&format!("    let w{i} = reach(xz, {});\n    total += w{i};\n", rect(i)));
    }
    s.push_str(&format!("    var c = {dirt} * ({} / total);\n", f(crate::terrain::DIRT_WEIGHT)));
    for i in 0..n {
        s.push_str(&format!("    if w{i} / total > {} {{ c += tile{i}(xz, gx, gy) * (w{i} / total); }}\n", f(1.0 / 512.0)));
    }
    s.push_str(
        "    texel = c;\n  }\n  let out = clamp(texel * i.sunlit * (1.0 - i.fog) + view.fog_color.rgb * i.fog, vec3f(0.0), vec3f(255.0));\n  return vec4f(out / 255.0, 1.0);\n}\n",
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
  origin: vec4f,
}
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var palette: texture_2d<f32>;

// A point through a row-vector matrix (`M4::clip`): the matrix's rows are uploaded as its columns.
fn rows(m: mat4x4f, p: vec3f) -> vec4f {
  return p.x * m[0] + p.y * m[1] + p.z * m[2] + m[3];
}
// `vp` looks from the origin (MD15): a point is moved by the view's origin, the camera's eye, first.
fn to_clip(p: vec3f) -> vec4f {
  return rows(view.vp, p - view.origin.xyz);
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
  origin: vec4f,
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
  coverage: u32,
  below: f32,
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
  @location(10) cover: f32,
}
struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) lit: vec3f,
  @location(2) fog: f32,
  @location(3) @interpolate(flat) cover: f32,
}

// `eco::covers`: whether a copy drawn at `cover` keeps this pixel of a 4x4 ordered dither; below 0
// the pattern's complement keeps `-cover` of them.
fn covers(at: vec2f, cover: f32) -> bool {
  var bayer = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);
  let p = vec2u(at) & vec2u(3u);
  let t = (f32(bayer[p.y * 4u + p.x]) + 0.5) / 16.0;
  return select((t >= 1.0 + cover), (t < cover), (cover >= 0.0));
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
  // The copy's place came already moved by the view's origin (MD15), as `vp` looks from it.
  o.clip = rows(view.vp, world);
  o.uv = v.uv;
  o.lit = lit;
  o.fog = clamp((distance(world, view.eye.xyz - view.origin.xyz) - view.fog.x) / max(view.fog.y - view.fog.x, 1.0), 0.0, view.fog.w);
  o.cover = v.cover;
  return o;
}

@fragment fn fs(i: Out) -> @location(0) vec4f {
  let t = textureSample(tex, samp, i.uv) * 255.0;
  // A cut-out that keeps its coverage (leaves) has it kept by its texture's mips (V58).
  let a = t.a;
  if (a < material.cutoff || a >= material.below || (i.cover < 1.0 && !covers(i.clip.xy, i.cover))) {
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
  origin: vec4f,
}
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var faces: texture_2d_array<f32>;
@group(1) @binding(1) var samp: sampler;
// The second sky, crossfaded in across a combined world's join (MD26), and how: its share, and
// whether the first and the second are there at all (the fog's colour where one is not).
@group(1) @binding(2) var others: texture_2d_array<f32>;
@group(1) @binding(3) var<uniform> blend: vec4f;

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
  let fog = view.fog_color.rgb / 255.0;
  let first = select(fog, textureSampleLevel(faces, samp, (uv + 1.0) * 0.5, face, 0.0).rgb, blend.y > 0.5);
  let second = select(fog, textureSampleLevel(others, samp, (uv + 1.0) * 0.5, face, 0.0).rgb, blend.z > 0.5);
  return vec4f(mix(first, second, blend.x), 1.0);
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

    /// A sparse cut-out (one solid texel in eight, scattered, as twigs are) and a solid block beside it in one
    /// 256x256 texture: each of its smaller levels, read bilinearly, covers within a tenth of the
    /// full size's share of each half at the cutoff (V58), where the plain box mips thin the sparse
    /// half to nothing by the third level; and a texture whose levels need no help is left as its
    /// plain mips have it.
    #[test]
    fn cut_out_mips_keep_the_full_sizes_coverage() {
        let n = 256;
        let rgba = (0..n * n)
            .flat_map(|i| {
                let (x, y) = (i % n, i / n);
                // The sparse half's texels by a hash of their place, about one in eight solid.
                let hash = ((x * 7919 + y * 104_729) as u32).wrapping_mul(0x9e37_79b9) >> 29;
                let solid = if x < n / 2 { hash == 0 } else { (n / 2 + 32..n - 32).contains(&x) && (32..n - 32).contains(&y) };
                [90, 140, 60, if solid { 255 } else { 0 }]
            })
            .collect();
        let t = Texture { width: n, height: n, rgba };
        let share = |px: &[u8], w: usize, half: usize| filtered_share(px, w, w, (half * w / 2, 0, (half + 1) * w / 2, w), 128);
        let (plain, kept) = (mips(&t), mips_keeping(&t, 128));
        let want = [0, 1].map(|half| share(&t.rgba, n, half));
        for (level, ((w, _, a), (_, _, b))) in plain.iter().zip(&kept).enumerate().skip(1).take(5) {
            let w = *w as usize;
            for half in [0, 1] {
                let (p, k) = (share(a, w, half), share(b, w, half));
                println!("level {level}, half {half}: plain {p:.3}, kept {k:.3}, full size {:.3}", want[half]);
                assert!((k - want[half]).abs() <= 0.1 * want[half], "level {level}, half {half}: {k:.3} against {:.3}", want[half]);
                if half == 0 && level >= 3 {
                    assert!(p < 0.5 * want[0], "the plain mips keep the sparse half at level {level}: {p:.3}");
                }
            }
        }
        assert_eq!(mips_keeping(&t, 0), plain);
        let start = std::time::Instant::now();
        let big = Texture { width: 512, height: 512, rgba: [t.rgba.clone(), t.rgba.clone(), t.rgba.clone(), t.rgba].concat() };
        mips_keeping(&big, 128);
        println!("a 512x512 cut-out's levels in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
    }

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

    /// [`Patch::of`]'s quick search (V30) finds the rectangle the plain one does: every row and column
    /// looked at, for layers touched in scattered spots of every size.
    #[test]
    fn an_overlay_finds_the_rectangle_the_plain_search_finds() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for _ in 0..500 {
            let (w, h) = (1 + next(40), 1 + next(30));
            let mut black = vec![0u8; w * h * 3];
            let mut white = vec![255u8; w * h * 3];
            for _ in 0..next(6) {
                let (x, y, c) = (next(w), next(h), next(3));
                match next(2) {
                    0 => black[(y * w + x) * 3 + c] = 1 + next(255) as u8,
                    _ => white[(y * w + x) * 3 + c] = next(255) as u8,
                }
            }
            let touched = |x: usize, y: usize| {
                let i = (y * w + x) * 3;
                black[i..i + 3] != [0; 3] || white[i..i + 3] != [255; 3]
            };
            let spots: Vec<(usize, usize)> = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| touched(x, y)).collect();
            let found = Patch::of(&black, &white, w, h, (3, 5)).map(|p| p.rect);
            let plain = (!spots.is_empty()).then(|| {
                let (x0, x1) = (spots.iter().map(|s| s.0).min().unwrap(), spots.iter().map(|s| s.0).max().unwrap());
                let (y0, y1) = (spots.iter().map(|s| s.1).min().unwrap(), spots.iter().map(|s| s.1).max().unwrap());
                [3 + x0 as u32, 5 + y0 as u32, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32]
            });
            assert_eq!(found, plain, "{w}x{h}, touched at {spots:?}");
        }
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
            meshes: None,
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
            depth_cutoff: 0,
            blend: true,
            additive: false,
            depth_write: false,
            clamp: [false; 2],
            keep_coverage: false,
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
            // Fog from 6 to 70 feet: the original's start is 64 feet short of the end at haziness 0.
            fog_near: 5.0,
            fog_far: 70.0,
            fog_visibility: 1.0,
            fog_haziness: 0.0,
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
            ..Scene::parse("")
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
                sprites: &[],
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
            gpu.render(&target, &assets, &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light });
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

    /// The backdrop crossfades between two skies by the second's share (MD26), a missing one is the
    /// fog's colour, and one sky alone is shown whatever the share.
    #[test]
    fn the_sky_crossfades_between_two_skies() {
        let mut gpu = match Renderer::headless() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipped: {e}");
                return;
            }
        };
        let (mut scene, assets, camera) = known_scene();
        scene.models.clear();
        let solid = |rgb: [u8; 3]| Sky {
            faces: (0..6).map(|_| Texture { width: 4, height: 4, rgba: (0..16).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect() }).collect(),
        };
        let (red, blue) = (solid([200, 40, 0]), solid([0, 80, 240]));
        let (w, h) = (64u32, 48u32);
        let light = Lighting::of(&scene);
        gpu.set_graphics(Graphics::Original);
        gpu.set_scene(&scene);
        let texture = gpu.offscreen(w, h);
        let view = texture.create_view(&Default::default());
        let draw = |gpu: &mut Renderer| {
            let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
            let target = Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 };
            gpu.render(&target, &assets, &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light });
            gpu.read(&texture)
        };
        // Every pixel within a step of `want`.
        let all = |px: &[u8], want: [f32; 3], what: &str| {
            for p in px.chunks_exact(3) {
                assert!((0..3).all(|k| (p[k] as f32 - want[k]).abs() <= 1.5), "{what}: {p:?}, not {want:?}");
            }
        };
        let between = |a: [u8; 3], b: [u8; 3], t: f32| std::array::from_fn(|k| a[k] as f32 + (b[k] as f32 - a[k] as f32) * t);
        gpu.set_skies(Some(&red), Some(&blue));
        for t in [0.0, 0.25, 0.5, 0.8, 1.0] {
            gpu.set_sky_mix(t);
            all(&draw(&mut gpu), between([200, 40, 0], [0, 80, 240], t), &format!("red to blue at {t}"));
        }
        let fog = light.fog_color;
        gpu.set_skies(Some(&red), None);
        gpu.set_sky_mix(0.5);
        all(&draw(&mut gpu), between([200, 40, 0], fog, 0.5), "red halfway to the fog");
        gpu.set_sky(Some(&blue));
        all(&draw(&mut gpu), between([0, 80, 240], [0; 3], 0.0), "one sky is the whole of it");
        gpu.set_sky_mix(0.7);
        all(&draw(&mut gpu), between([0, 80, 240], [0; 3], 0.0), "one sky whatever the share");
    }

    /// A scene with the disc's kind of fog: free07's, 25 to 1,000 feet at haziness 0.7, which the
    /// original draws from 280.8 to 1,000 feet.
    fn fogged() -> Lighting {
        Lighting::of(&crate::scene::Scene::parse(
            "[Fog]\nColorRGB=61,68,76\nVisibility=1.00\nHaziness=0.70\n\
             MinimumFogDistance=25.00\nMaximumFogDistance=1000.00\n",
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
        assert_eq!(fog, Fog { near: light.fog_near, far: 1000.0, max: 1.0, reach: 1000.0 });
        assert!((fog.near - 280.8).abs() < 1e-3, "the fog starts at {}", fog.near);
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
                &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light },
            );
            d.ground_levels.iter().sum::<usize>()
        };
        assert_eq!(drawn(&mut gpu, Graphics::Full), 2, "the chunk ahead and the far edge's");
        assert_eq!(drawn(&mut gpu, Graphics::Original), 1, "the chunk ahead only");
        // And back: the per-view index list follows the setting.
        assert_eq!(drawn(&mut gpu, Graphics::Full), 2);
    }

    /// [`known_scene`] with a HUD's worth of sprites over it (V30): a picture, plain panels, a disc,
    /// a dial's needle, a capsule bar and a glow, the kinds the HUD and the menus are drawn with.
    fn known_frame(gpu: &mut Renderer, target: &Target, assets: &Assets, scene: &Scene, camera: &Camera) -> Drawn {
        let picture = Texture { width: 8, height: 8, rgba: (0..64u8).flat_map(|i| [i * 4, 255 - i * 4, 128, 255]).collect() };
        gpu.set_images(&[&picture]);
        let (w, h) = (target.frame[2] as f32, target.frame[3] as f32);
        let mut sprites = vec![
            Sprite::image(0, &picture, [0.0, 0.0, 8.0, 8.0], [w * 0.05, h * 0.05, w * 0.2, h * 0.2], [255; 4]),
            Sprite::image(0, &picture, [2.0, 2.0, 6.0, 6.0], [w * 0.7, h * 0.1, w * 0.9, h * 0.3], [255, 255, 255, 160]),
        ];
        // A quad from (x0, y0) to (x1, y1) whose shape is placed with its origin at `o`.
        let panel = |[x0, y0, x1, y1]: [f32; 4], o: [f32; 2], tint: [u8; 4], shape: Shape| {
            let corners = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
            Sprite { corners, uv: [[0.0; 2]; 4], local: corners.map(|[x, y]| [x - o[0], y - o[1]]), clamp: [0.0; 4], image: None, tint, shape }
        };
        sprites.push(panel([0.0, h * 0.85, w, h], [0.0, h * 0.85], [0, 0, 0, 140], Shape::Plain));
        let (cx, cy, r) = (w * 0.2, h * 0.7, h * 0.1);
        sprites.push(panel([cx - r, cy - r, cx + r, cy + r], [cx, cy], [250, 200, 40, 220], Shape::Disc { radius: r }));
        let needle = Shape::Needle { tail: r * 0.2, tip: r * 0.9, tail_half: r * 0.08, tip_half: r * 0.02 };
        sprites.push(panel([cx - r, cy - r, cx + r, cy + r], [cx, cy], [255, 40, 40, 255], needle));
        let capsule = Shape::Capsule { size: [w * 0.3, h * 0.04], split: w * 0.18, right: [60, 60, 60, 200] };
        sprites.push(panel([w * 0.6, h * 0.9, w * 0.9, h * 0.94], [w * 0.6, h * 0.9], [40, 220, 90, 255], capsule));
        sprites.push(panel([w * 0.4, h * 0.4, w * 0.6, h * 0.5], [w * 0.4, h * 0.4], [255, 255, 200, 90], Shape::Glow));
        let views = [View { rect: [0, 0, target.frame[2], target.frame[3]], camera, actors: &[] }];
        let light = Lighting::of(scene);
        gpu.render(target, assets, &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &sprites, light: &light })
    }

    /// A 4K frame of a known scene and its HUD, start to finish on the GPU, inside a 120 Hz frame's
    /// time (V30) — the shell at 3840x2160 took 196 ms a frame when its HUD was drawn on the CPU.
    #[test]
    fn a_4k_frame_fits_in_a_120hz_frame() {
        let mut gpu = match Renderer::headless() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipped: {e}");
                return;
            }
        };
        let (scene, assets, camera) = known_scene();
        let (w, h) = (3840u32, 2160u32);
        gpu.set_scene(&scene);
        gpu.set_sky(None);
        let texture = gpu.offscreen(w, h);
        let view = texture.create_view(&Default::default());
        let target = Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 };
        // The first frames build the pipelines' state and the depth buffer; they are not timed.
        for _ in 0..3 {
            known_frame(&mut gpu, &target, &assets, &scene, &camera);
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        }
        let frames = 30;
        let started = std::time::Instant::now();
        for _ in 0..frames {
            let drawn = known_frame(&mut gpu, &target, &assets, &scene, &camera);
            assert!(drawn.triangles[0] > 0);
            let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        }
        let each = started.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        eprintln!("a 3840x2160 frame in {each:.2} ms");
        assert!(each < 1000.0 / 120.0, "a 3840x2160 frame took {each:.2} ms, over a 120 Hz frame's 8.33");
        // It drew the scene and the HUD, not an empty frame.
        let pixels = gpu.read(&texture);
        assert!(pixels.chunks_exact(3).filter(|p| p != &scene.fog_color).count() > (w * h / 10) as usize);
    }

    /// The views drawn at half size and stretched over the frame (V30's render scale) come out as the
    /// full-size frame does but for the softness of the stretch, with the HUD at full size over both.
    #[test]
    fn a_frame_at_half_scale_looks_like_the_full_frame() {
        let mut gpu = match Renderer::headless() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipped: {e}");
                return;
            }
        };
        let (scene, assets, camera) = known_scene();
        let (w, h) = (640u32, 360u32);
        gpu.set_scene(&scene);
        gpu.set_sky(None);
        let shot = |gpu: &mut Renderer, scale: f32| {
            gpu.render_scale = scale;
            let texture = gpu.offscreen(w, h);
            let view = texture.create_view(&Default::default());
            let target = Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 };
            known_frame(gpu, &target, &assets, &scene, &camera);
            gpu.read(&texture)
        };
        let full = shot(&mut gpu, 1.0);
        let half = shot(&mut gpu, 0.5);
        let (mut sum, mut off, mut changed) = (0u64, 0, 0);
        for (a, b) in full.chunks_exact(3).zip(half.chunks_exact(3)) {
            let d = a.iter().zip(b).map(|(&a, &b)| (a as i32 - b as i32).abs()).max().unwrap_or(0);
            sum += d as u64;
            off += (d > 40) as usize;
            changed += (d > 0) as usize;
        }
        let n = (w * h) as f64;
        let (mean, share) = (sum as f64 / n, off as f64 / n);
        eprintln!("half scale: mean {mean:.2}, {:.3}% over 40, {:.1}% changed", share * 100.0, changed as f64 / n * 100.0);
        if let Ok(dir) = std::env::var("GPU_TEST_PNG") {
            let _ = crate::png::write_rgb(&std::path::Path::new(&dir).join("full.png"), w as usize, h as usize, &full);
            let _ = crate::png::write_rgb(&std::path::Path::new(&dir).join("half.png"), w as usize, h as usize, &half);
        }
        // The stretch did happen: edges in the scene are softer.
        assert!(changed > 100, "the half-scale frame is the full one, pixel for pixel");
        // Only along the scene's edges does a pixel part by much; the flat colours, the fog and the
        // HUD, drawn at full size either way, agree.
        assert!(mean < 1.0, "mean difference {mean:.2}");
        assert!(share < 0.01, "{:.2}% of pixels differ by more than 40", share * 100.0);
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
        let paint = Splat { textures: (0..max - 1).map(|_| layer()).collect(), passes, shadow: Some(shadow), meshes: None };
        let pages = Pages {
            pages: vec![layer()],
            across: 1,
            along: 1,
            page_of: vec![Some(0)],
            spacing: 1.0,
            detail: Some(layer()),
            detail_repeat: 8.0,
        };
        let shaders = [
            ("mesh", MESH_WGSL.to_string()),
            ("sky", SKY_WGSL.to_string()),
            ("place", PLACE_WGSL.to_string()),
            ("sprite", sprite::SPRITE_WGSL.to_string()),
            ("glow", format!("{VIEW_WGSL}{}", glows::GLOW_WGSL)),
            ("upscale", scale::UPSCALE_WGSL.to_string()),
            ("ground, painted", ground_wgsl(Some(&paint), [200.0, 180.0, 150.0])),
            ("ground, bare", ground_wgsl(None, [200.0, 180.0, 150.0])),
            // With the roads and ruts (V67), whose two targets take two layers' places.
            ("ground, meshed", {
                let pass = paint.passes[1].clone();
                let flat = crate::ground_mesh::Flat::new([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], [[0.0; 2]; 3], [[128.0; 4]; 3], pass);
                let meshed = Splat {
                    textures: (0..max - 3).map(|_| layer()).collect(),
                    passes: paint.passes[..max - 3].to_vec(),
                    shadow: Some(crate::splat::Shadow { tiles: vec![layer()], across: 1, x0: -10.0, x1: 10.0, z0: -10.0, z1: 10.0 }),
                    meshes: crate::ground_mesh::GroundMeshes::of(vec![layer()], vec![flat]),
                };
                ground_wgsl(Some(&meshed), [200.0, 180.0, 150.0])
            }),
            ("ground meshes", ground_mesh::DRAW_WGSL.to_string()),
            ("ground, pages", ground_pages_wgsl(&pages, [200.0, 180.0, 150.0])),
            ("ground, world", {
                let world = WorldPaint {
                    regions: crate::terrain::Regions { owned: vec![[0.0, 0.0, 16.0, 16.0], [40.0, 0.0, 56.0, 16.0]] },
                    tiles: vec![
                        crate::terrain::PaintTile { at: [0.0, 0.0], pages: Some(pages.clone()) },
                        crate::terrain::PaintTile { at: [40.0, 0.0], pages: None },
                    ],
                    dirt: [200.0, 180.0, 150.0],
                };
                ground_world_wgsl(&world, &world.pack(256, 2048))
            }),
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
