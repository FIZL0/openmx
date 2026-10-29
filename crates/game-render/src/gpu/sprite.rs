//! The overlay's sprites (V30): quads the GPU draws over the views, each a texture or a colour, so a
//! HUD made of pictures, shapes and text costs the CPU a few hundred vertices a frame instead of a
//! frame-sized image drawn twice, read back and re-uploaded ([`super::Patch`]).
//!
//! A sprite is four corners in the frame's pixels, the texels they take their picture from, and a
//! shape that cuts it: the whole quad, a disc, a capsule, or the speedometer's tapered needle. The
//! shapes' edges are smoothed by their distance in pixels, which is what the CPU's four samples a
//! pixel approximate. The pictures are uploaded once ([`super::Renderer::set_images`]).

use super::{PlaceUniform, depth_state, sampler_entry, texture_entry, uniform_entry, upload_texture};
use crate::pst::Texture;
use bytemuck::{Pod, Zeroable};

/// What cuts a sprite's quad, in the sprite's own `local` coordinates (pixels of the frame).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// The whole quad.
    Plain,
    /// A disc of `radius` round `local`'s origin.
    Disc { radius: f32 },
    /// A `size` rectangle from `local`'s origin whose shorter sides are half circles, `tint` left of
    /// `split` and `right` from there on.
    Capsule { size: [f32; 2], split: f32, right: [u8; 4] },
    /// A bar along `local`'s x from `-tail` to `tip`, `tail_half` either side of the axis at the tail
    /// end narrowing to `tip_half` at the tip.
    Needle { tail: f32, tip: f32, tail_half: f32, tip_half: f32 },
    /// The whole quad, its light added to what is under it by its alpha rather than laid over it:
    /// the menus' glow.
    Glow,
    /// A `size` rectangle from `local`'s origin with corners of `radius`, `tint` at its top shading to
    /// `bottom` at its foot — or, with `ring` above 0, only the band that wide inside its edge: the
    /// menus' pills, tabs and boxes (P118).
    Round { size: [f32; 2], radius: f32, ring: f32, bottom: [u8; 4] },
    /// Type out of a distance atlas (P118): the picture's alpha is 0.5 on the outline and moves by
    /// 0.5 over the atlas' spread, and `range` is the frame pixels that spread covers here, so the
    /// edge is a pixel wide at any size. The colour is the tint's alone.
    Text { range: f32 },
}

/// One quad of the overlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    /// The corners, in the frame's pixels, going round the quad.
    pub corners: [[f32; 2]; 4],
    /// Each corner's place in the image, in texels.
    pub uv: [[f32; 2]; 4],
    /// Each corner in the shape's own coordinates.
    pub local: [[f32; 2]; 4],
    /// The texel centres the picture is read between (left, top, right, bottom), so a glyph does not
    /// bleed into its neighbour in an atlas.
    pub clamp: [f32; 4],
    /// Which of [`super::Renderer::set_images`]'s pictures, or `None` for plain colour.
    pub image: Option<usize>,
    /// What the picture is multiplied by, alpha included.
    pub tint: [u8; 4],
    pub shape: Shape,
}

impl Sprite {
    /// An image's `src` texels (left, top, right, bottom) over the axis-aligned frame rectangle
    /// `rect` (left, top, right, bottom), multiplied by `tint`.
    pub fn image(image: usize, texture: &Texture, src: [f32; 4], rect: [f32; 4], tint: [u8; 4]) -> Sprite {
        let [x0, y0, x1, y1] = rect;
        let [u0, v0, u1, v1] = src;
        Sprite {
            corners: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
            uv: [[u0, v0], [u1, v0], [u1, v1], [u0, v1]],
            local: [[0.0; 2]; 4],
            clamp: clamp_of(texture, src),
            image: Some(image),
            tint,
            shape: Shape::Plain,
        }
    }
}

/// The texel centres `bilinear` keeps a read of `src` between: the same rule as the CPU's.
pub fn clamp_of(t: &Texture, src: [f32; 4]) -> [f32; 4] {
    let lo_x = src[0].min(src[2]).max(0.0);
    let hi_x = (src[0].max(src[2]) - 1.0).min(t.width as f32 - 1.0).max(lo_x);
    let lo_y = src[1].min(src[3]).max(0.0);
    let hi_y = (src[1].max(src[3]) - 1.0).min(t.height as f32 - 1.0).max(lo_y);
    [lo_x, lo_y, hi_x, hi_y]
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    local: [f32; 2],
    clamp: [f32; 4],
    params: [f32; 4],
    /// The shape's kind, and the capsule's split.
    kind: [f32; 2],
    tint: [u8; 4],
    right: [u8; 4],
}

const ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
    0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32x4, 4 => Float32x4, 5 => Float32x2,
    6 => Unorm8x4, 7 => Unorm8x4
];

/// What the renderer keeps to draw sprites.
pub(super) struct Sprites {
    pipeline: wgpu::RenderPipeline,
    /// [`Shape::Glow`]'s: the same, adding.
    glow: wgpu::RenderPipeline,
    image_layout: wgpu::BindGroupLayout,
    place: wgpu::Buffer,
    place_bind: wgpu::BindGroup,
    /// Plain colour's picture: one white texel.
    white: wgpu::BindGroup,
    images: Vec<wgpu::BindGroup>,
    buffer: wgpu::Buffer,
    capacity: u64,
    vertices: Vec<Vertex>,
    /// This frame's runs: an image (`None` for white), whether they glow, and the vertices drawn
    /// with them.
    runs: Vec<(Option<usize>, bool, std::ops::Range<u32>)>,
}

impl Sprites {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat, sampler: &wgpu::Sampler) -> Sprites {
        let place_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite place"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite image"),
            entries: &[texture_entry(0, true, wgpu::TextureViewDimension::D2), sampler_entry(1)],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite"),
            source: wgpu::ShaderSource::Wgsl(SPRITE_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite"),
            bind_group_layouts: &[Some(&place_layout), Some(&image_layout)],
            immediate_size: 0,
        });
        let over = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let adding = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline_of = |blend: wgpu::BlendComponent| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRIBUTES,
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(depth_state(false, wgpu::CompareFunction::Always)),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState { color: blend, alpha: blend }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let (pipeline, glow) = (pipeline_of(over), pipeline_of(adding));
        let place = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite place"),
            size: std::mem::size_of::<SpritePlace>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let place_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite place"),
            layout: &place_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: place.as_entire_binding(),
            }],
        });
        let white_view = upload_texture(device, queue, &Texture { width: 1, height: 1, rgba: vec![255; 4] });
        let white = image_bind(device, &image_layout, &white_view, sampler);
        let capacity = 256;
        let buffer = vertex_buffer(device, capacity);
        Sprites {
            pipeline,
            glow,
            image_layout,
            place,
            place_bind,
            white,
            images: Vec::new(),
            buffer,
            capacity,
            vertices: Vec::new(),
            runs: Vec::new(),
        }
    }

    pub(super) fn set_images(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, sampler: &wgpu::Sampler, images: &[&Texture]) {
        self.images = images
            .iter()
            .map(|t| image_bind(device, &self.image_layout, &upload_texture(device, queue, t), sampler))
            .collect();
    }

    /// Lay this frame's sprites out for [`Sprites::draw`]; `false` when there are none to draw.
    pub(super) fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, place: PlaceUniform, size: (u32, u32), sprites: &[super::Sprite]) -> bool {
        self.vertices.clear();
        self.runs.clear();
        for s in sprites {
            let image = s.image.filter(|&i| i < self.images.len());
            let (kind, params, split, right) = match s.shape {
                Shape::Plain | Shape::Glow => (0.0, [0.0; 4], 0.0, s.tint),
                Shape::Disc { radius } => (1.0, [radius, 0.0, 0.0, 0.0], 0.0, s.tint),
                Shape::Capsule { size, split, right } => (2.0, [size[0], size[1], 0.0, 0.0], split, right),
                Shape::Needle { tail, tip, tail_half, tip_half } => (3.0, [tail, tip, tail_half, tip_half], 0.0, s.tint),
                Shape::Text { range } => (4.0, [range, 0.0, 0.0, 0.0], 0.0, s.tint),
                Shape::Round { size, radius, ring, bottom } => (5.0, [size[0], size[1], radius, ring], 0.0, bottom),
            };
            let at = self.vertices.len() as u32;
            for i in [0, 1, 2, 0, 2, 3] {
                self.vertices.push(Vertex {
                    pos: s.corners[i],
                    uv: s.uv[i],
                    local: s.local[i],
                    clamp: s.clamp,
                    params,
                    kind: [kind, split],
                    tint: s.tint,
                    right,
                });
            }
            let glow = s.shape == Shape::Glow;
            match self.runs.last_mut() {
                Some((was, glowed, run)) if *was == image && *glowed == glow => run.end = at + 6,
                _ => self.runs.push((image, glow, at..at + 6)),
            }
        }
        if self.vertices.is_empty() {
            return false;
        }
        if self.vertices.len() as u64 > self.capacity {
            self.capacity = (self.vertices.len() as u64).next_power_of_two();
            self.buffer = vertex_buffer(device, self.capacity);
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.vertices));
        let place = SpritePlace {
            size: [size.0.max(1) as f32, size.1.max(1) as f32],
            origin: place.origin,
            scale: place.scale,
            _pad: [0.0; 3],
        };
        queue.write_buffer(&self.place, 0, bytemuck::bytes_of(&place));
        true
    }

    /// Draw what [`Sprites::prepare`] laid out, into a pass over the whole target.
    pub(super) fn draw(&self, pass: &mut wgpu::RenderPass) -> usize {
        pass.set_bind_group(0, &self.place_bind, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        for (image, glow, run) in &self.runs {
            pass.set_pipeline(if *glow { &self.glow } else { &self.pipeline });
            let bind = image.map_or(&self.white, |i| &self.images[i]);
            pass.set_bind_group(1, bind, &[]);
            pass.draw(run.clone(), 0..1);
        }
        self.runs.len()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SpritePlace {
    size: [f32; 2],
    origin: [f32; 2],
    scale: f32,
    _pad: [f32; 3],
}

fn vertex_buffer(device: &wgpu::Device, capacity: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sprites"),
        size: capacity * std::mem::size_of::<Vertex>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn image_bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, view: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sprite image"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

/// Sprites: placed in the frame's pixels, read between their texel centres, cut by their shape.
pub(super) const SPRITE_WGSL: &str = r#"
struct Place {
  size: vec2f,
  origin: vec2f,
  scale: f32,
  // to SpritePlace's 32 bytes: WebGL2 binds a uniform in whole 16-byte rows (P48)
  pad0: f32,
  pad1: f32,
  pad2: f32,
}
@group(0) @binding(0) var<uniform> place: Place;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct In {
  @location(0) pos: vec2f,
  @location(1) uv: vec2f,
  @location(2) local: vec2f,
  @location(3) clamp: vec4f,
  @location(4) params: vec4f,
  @location(5) kind: vec2f,
  @location(6) tint: vec4f,
  @location(7) right: vec4f,
}

struct Out {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) local: vec2f,
  @location(2) clamp: vec4f,
  @location(3) params: vec4f,
  @location(4) kind: vec2f,
  @location(5) tint: vec4f,
  @location(6) right: vec4f,
}

@vertex fn vs(i: In) -> Out {
  var o: Out;
  let at = (place.origin + i.pos * place.scale) / place.size * 2.0 - 1.0;
  o.clip = vec4f(at.x, -at.y, 0.5, 1.0);
  o.uv = i.uv;
  o.local = i.local;
  o.clamp = i.clamp;
  o.params = i.params;
  o.kind = i.kind;
  o.tint = i.tint;
  o.right = i.right;
  return o;
}

@fragment fn fs(i: Out) -> @location(0) vec4f {
  let size = vec2f(textureDimensions(image));
  let texel = (clamp(i.uv - 0.5, i.clamp.xy, i.clamp.zw) + 0.5) / size;
  let raw = textureSampleLevel(image, samp, texel, 0.0);
  var c = raw * i.tint;
  let p = i.local;
  let kind = i.kind.x;
  var cover = 1.0;
  if (kind > 4.5) {
    // The rounded box: its signed distance, the band inside the edge for a ring, and the shade.
    let half = i.params.xy * 0.5;
    let rad = min(i.params.z, min(half.x, half.y));
    let q = abs(p - half) - (half - vec2f(rad, rad));
    let d = length(max(q, vec2f(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - rad;
    var inside = -d;
    if (i.params.w > 0.0) {
      inside = min(-d, d + i.params.w);
    }
    cover = clamp(inside + 0.5, 0.0, 1.0);
    let k = clamp(p.y / max(i.params.y, 1.0), 0.0, 1.0);
    c = mix(i.tint, i.right, k);
  } else if (kind > 3.5) {
    // Type: the atlas' distance, in frame pixels, smoothed over one.
    cover = clamp((raw.a - 0.5) * 2.0 * i.params.x + 0.5, 0.0, 1.0);
    c = i.tint;
  } else if (kind > 2.5) {
    // The needle: inside its taper and between its two ends.
    let k = (p.x + i.params.x) / (i.params.x + i.params.y);
    let half = i.params.z + k * (i.params.w - i.params.z);
    let inside = min(half - abs(p.y), min(p.x + i.params.x, i.params.y - p.x));
    cover = clamp(inside + 0.5, 0.0, 1.0);
  } else if (kind > 1.5) {
    // The capsule: within its radius of the segment between its two round ends.
    let rad = min(i.params.x, i.params.y) * 0.5;
    let a = vec2f(rad, rad);
    let b = select(vec2f(rad, i.params.y - rad), vec2f(i.params.x - rad, rad), i.params.x >= i.params.y);
    let q = clamp(p, min(a, b), max(a, b));
    cover = clamp(rad - length(p - q) + 0.5, 0.0, 1.0);
    if (p.x >= i.kind.y) {
      c = i.right;
    }
  } else if (kind > 0.5) {
    cover = select(0.0, 1.0, dot(p, p) <= i.params.x * i.params.x);
  }
  return vec4f(c.rgb, c.a * cover);
}
"#;
