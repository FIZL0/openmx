//! The sun's shadow maps on the GPU (V108, [`crate::shadow`]): each view's maps drawn from the sun,
//! depth only, before the views, and read by the ground's and the models' shaders through the
//! view's uniforms ([`SHADOW_WGSL`]).
//!
//! Under [`Shadows::Better`] a view has two maps nested along its look ([`shadow::CASCADES`]), into
//! which everything near casts: the actors, the props and the plants the view draws. Under
//! [`Shadows::Original`] each racer — the actors whose spheres touch, made one — has its own small
//! map ([`shadow::ORIGINAL_SIZE`]) fitted round it, as the original's projected shadow is, and only
//! the ground receives it.

use super::{Batch, DEPTH, InstanceRaw, MeshVertex, Renderer, VIEW_STRIDE, View, ViewUniform};
use crate::mat::{M4, V3, v3};
use crate::shadow::{self, Shadows, Tint};

/// The most maps one view reads: the view's uniforms hold this many.
pub(super) const MAPS: usize = 8;

/// The biggest actor that casts the original's shadow, its radius in feet: a racer and their
/// machine, not the train.
const ORIGINAL_MOST: f32 = 20.0;

/// How far under its caster the original's shadow reaches, in feet past the caster's sphere: it
/// fades out over the last [`ORIGINAL_FADE`] of that.
const ORIGINAL_DROP: f32 = 40.0;
const ORIGINAL_FADE: f32 = 0.4;

/// One map this frame: the pass that reads it, and its box along the sun ([`shadow::light_box`]).
#[derive(Clone)]
pub(super) struct Map {
    pass: usize,
    origin: V3,
    sun: V3,
    centre: V3,
    half: f32,
    back: f32,
    ahead: f32,
    /// What the shader reads per map: how much of it is drawn, the depth bias, the step between the
    /// texels it filters over, and 1 for a map that fades with depth (the original's).
    layer: [f32; 4],
    /// The actors that cast into it (the original's, one racer), or `None` for everything near.
    actors: Option<Vec<usize>>,
}

impl Map {
    fn matrix(&self) -> M4 {
        shadow::light_box(self.sun, self.centre, self.half, self.back, self.ahead, self.origin)
    }

    fn holds(&self, c: V3, r: f32) -> bool {
        shadow::in_box(self.sun, self.centre, self.half, self.back, self.ahead, c, r)
    }
}

/// The shadow maps and how they are drawn.
pub(super) struct SunMaps {
    pub(super) mode: Shadows,
    /// The scene's [`Tint`] for the original's, and the light maps' levels for ours
    /// ([`shadow::light_map_levels`]).
    pub(super) tint: Tint,
    pub(super) levels: (f32, f32),
    texture: wgpu::Texture,
    pub(super) array: wgpu::TextureView,
    layers: Vec<wgpu::TextureView>,
    size: u32,
    pub(super) sampler: wgpu::Sampler,
    /// The view's uniforms and palette without the maps, for drawing into them.
    layout: wgpu::BindGroupLayout,
    bind: Option<wgpu::BindGroup>,
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    capacity: u64,
    maps: Vec<Map>,
    /// Each copy's map, model and level, as [`Renderer::scratch`]'s key with the map for the view.
    scratch: Vec<(u64, InstanceRaw)>,
    batches: Vec<Batch>,
    flat: Vec<InstanceRaw>,
}

fn maps_texture(device: &wgpu::Device, size: u32, layers: u32) -> (wgpu::Texture, wgpu::TextureView, Vec<wgpu::TextureView>) {
    // Never one layer (WebGL2 would take it for a plain 2D texture) nor a square six (a cube).
    let layers = layers.max(2).next_power_of_two();
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sun maps"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: layers },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let array = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let each = (0..layers)
        .map(|l| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: l,
                array_layer_count: Some(1),
                ..Default::default()
            })
        })
        .collect();
    (texture, array, each)
}

/// The view layout's entries for the maps: the array and its comparing sampler.
pub(super) fn layout_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2Array,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 3,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            count: None,
        },
    ]
}

impl SunMaps {
    pub(super) fn new(device: &wgpu::Device, mesh_module: &wgpu::ShaderModule, material_layout: &wgpu::BindGroupLayout) -> SunMaps {
        let (texture, array, layers) = maps_texture(device, 1, 2);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sun maps"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::GreaterEqual),
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sun caster view"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sun caster"),
            bind_group_layouts: &[Some(&layout), Some(material_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sun caster"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: mesh_module,
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
            // Depth is reversed (nearer the sun is more), so a caster is pushed away from the sun by
            // its slope: a model's own lit faces do not shadow themselves.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: -2, slope_scale: -1.5, clamp: 0.0 },
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: mesh_module,
                entry_point: Some("fs_shadow"),
                compilation_options: Default::default(),
                targets: &[],
            }),
            multiview_mask: None,
            cache: None,
        });
        let capacity = 1024;
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sun caster instances"),
            size: capacity * std::mem::size_of::<InstanceRaw>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        SunMaps {
            mode: Shadows::default(),
            tint: Tint::default(),
            levels: (0.0, shadow::SHADE_DEFAULT),
            texture,
            array,
            layers,
            size: 1,
            sampler,
            layout,
            bind: None,
            pipeline,
            instances,
            capacity,
            maps: Vec::new(),
            scratch: Vec::new(),
            batches: Vec::new(),
            flat: Vec::new(),
        }
    }

    /// The bind group the casters are drawn with, over the views' `buffer` and `palette`: made again
    /// whenever either is.
    pub(super) fn rebind(&mut self, device: &wgpu::Device, buffer: &wgpu::Buffer, palette: &wgpu::Texture) {
        let palette = palette.create_view(&Default::default());
        self.bind = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sun caster view"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<ViewUniform>() as u64),
                    }),
                },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&palette) },
            ],
        }));
    }

    /// Whether pass `p` has maps everything near casts into (ours).
    pub(super) fn takes_all(&self, p: usize) -> bool {
        self.maps.iter().any(|m| m.pass == p && m.actors.is_none())
    }

    /// How many maps this frame draws.
    pub(super) fn count(&self) -> usize {
        self.maps.len()
    }

    /// Pass `p`'s shadow uniforms: the mode, how many maps it reads and the first, and how far ours
    /// reach; the tint; and each map's [`Map::layer`] and matrix.
    pub(super) fn fields(&self, p: usize) -> ([f32; 4], [f32; 4], [[f32; 4]; MAPS], [[[f32; 4]; 4]; MAPS]) {
        let first = self.maps.iter().position(|m| m.pass == p);
        let mut layer = [[0.0; 4]; MAPS];
        let mut m = [[[0.0; 4]; 4]; MAPS];
        let Some(first) = first else { return ([0.0; 4], [0.0; 4], layer, m) };
        let mine = &self.maps[first..];
        let count = mine.iter().take_while(|x| x.pass == p).count().min(MAPS);
        for (k, map) in mine[..count].iter().enumerate() {
            layer[k] = map.layer;
            m[k] = map.matrix().0;
        }
        let (mode, tint, reach) = match self.mode {
            // The strength squared: in PCSX2 free06's shadow (25,25,25 at 0.7) leaves 0.63 of the
            // dirt, as 0.7 x 0.7 does and 0.7 does not. Read as the intensity weighing both the
            // caster's alpha and the receiver's; the V108 journal has the measurement.
            Shadows::Original => (1.0, [self.tint.rgb[0], self.tint.rgb[1], self.tint.rgb[2], self.tint.alpha * self.tint.alpha], 0.0),
            // Ours keep how far they reach in the kind slot ([`Renderer::plan_sun_maps`]).
            _ => (2.0, [self.levels.0, self.levels.1, 0.0, 0.0], mine[count - 1].layer[3]),
        };
        ([mode, count as f32, first as f32, reach], tint, layer, m)
    }

    /// Copy `inst` of `model` at level `group`, standing at `c` with radius `r`, into every map of
    /// pass `p` it reaches: an actor (its index in the view) or anything else.
    pub(super) fn cast(&mut self, p: usize, actor: Option<usize>, c: V3, r: f32, model: usize, group: usize, inst: InstanceRaw) {
        for (l, map) in self.maps.iter().enumerate().filter(|(_, m)| m.pass == p) {
            let takes = match (&map.actors, actor) {
                (Some(list), Some(a)) => list.contains(&a),
                (Some(_), None) => false,
                (None, _) => true,
            };
            if takes && map.holds(c, r) {
                let key = ((l as u64) << 48) | ((model as u64) << 16) | group.min(0xffff) as u64;
                self.scratch.push((key, inst));
            }
        }
    }
}

impl Renderer {
    /// Ours or the original's shadows, or none, from the next frame (V108).
    pub fn set_shadows(&mut self, shadows: Shadows) {
        self.sun.mode = shadows;
    }

    /// The colour and strength Original's shadows lie in, off the scene's sun (V108).
    pub fn shadow_tint(&self) -> Tint {
        self.sun.tint
    }

    /// This frame's maps, before its uniforms are written: for each view's own pass (not a wrapped
    /// world's copy's) under the setting, ours along its look or the original's round each racer.
    pub(super) fn plan_sun_maps(&mut self, views: &[View], passes: &[(usize, Option<[i32; 2]>, usize)], frame_light: &crate::world::Lighting) {
        self.sun.maps.clear();
        self.sun.scratch.clear();
        if self.sun.mode == Shadows::Off {
            return;
        }
        let size = match self.sun.mode {
            Shadows::Original => shadow::ORIGINAL_SIZE,
            _ if views.len() > 2 || cfg!(target_arch = "wasm32") => 1024,
            _ => 2048,
        };
        let mut maps = Vec::new();
        for (p, &(v, copy, _)) in passes.iter().enumerate() {
            if copy.is_some() {
                continue;
            }
            let view = &views[v];
            let light = *self.view_light(v, frame_light);
            let sun = light.sun;
            if !(sun.len() > 0.0) {
                continue;
            }
            let camera = view.camera;
            let origin = camera.eye;
            match self.sun.mode {
                Shadows::Original => {
                    let mut spheres = Vec::new();
                    let mut who = Vec::new();
                    for (i, a) in view.actors.iter().enumerate() {
                        let Some(Some(m)) = self.model_ids.get(a.model).copied() else { continue };
                        if a.alpha < 1.0 {
                            continue;
                        }
                        let row = |r: usize| v3(a.at.0[r][0], a.at.0[r][1], a.at.0[r][2]);
                        let r = self.models[m].radius * row(0).len().max(row(1).len()).max(row(2).len());
                        if !(r > 0.0) || r > ORIGINAL_MOST {
                            continue;
                        }
                        spheres.push((row(3), r));
                        who.push(i);
                    }
                    for (c, r, held) in shadow::groups(&spheres).into_iter().take(MAPS) {
                        let fade = shadow::original_fade(c.sub(camera.eye).len(), views.len());
                        if fade <= 0.0 {
                            continue;
                        }
                        let (back, ahead) = (r + 10.0, r + ORIGINAL_DROP);
                        let texel = 2.0 * r / size as f32;
                        maps.push(Map {
                            pass: p,
                            origin,
                            sun,
                            centre: c,
                            half: r,
                            back,
                            ahead,
                            // The filter steps two texels: the original blurs its 128x128 shadow, and in PCSX2
                            // its edge is that soft, not one texel's.
                            layer: [fade, 2.0 * texel / (back + ahead), 2.0 / size as f32, 1.0],
                            actors: Some(held.into_iter().map(|k| who[k]).collect()),
                        });
                    }
                }
                _ => {
                    let [_, _, w, h] = view.rect;
                    let aspect = w as f32 / h.max(1) as f32;
                    let forward = camera.at.sub(camera.eye).norm();
                    let (_, far) = self.fog(&light);
                    let reach = shadow::CASCADES[1].min(far);
                    let slices = if reach > shadow::CASCADES[0] {
                        vec![(0.0, shadow::CASCADES[0]), (shadow::CASCADES[0] * 0.8, reach)]
                    } else {
                        vec![(0.0, reach)]
                    };
                    for (near, far) in slices {
                        let (c, r) = shadow::slice_sphere(camera.eye, forward, camera.fov, aspect, near, far);
                        let centre = shadow::snapped(sun, c, r, size);
                        let (back, ahead) = (r + shadow::REACH_BACK, r);
                        let texel = 2.0 * r / size as f32;
                        // The kind slot holds how far ours reach, which `fields` hands the shader.
                        maps.push(Map {
                            pass: p,
                            origin,
                            sun,
                            centre,
                            half: r,
                            back,
                            ahead,
                            layer: [1.0, 1.5 * texel / (back + ahead), 1.0 / size as f32, reach],
                            actors: None,
                        });
                    }
                }
            }
        }
        self.sun.maps = maps;
        let wanted = self.sun.maps.len() as u32;
        if self.sun.size != size || (self.sun.layers.len() as u32) < wanted {
            let (texture, array, layers) = maps_texture(&self.device, size, wanted.max(self.sun.layers.len() as u32 * u32::from(self.sun.size == size)));
            (self.sun.texture, self.sun.array, self.sun.layers, self.sun.size) = (texture, array, layers, size);
            self.view_bind = super::view_bind_group(&self.device, &self.view_layout, &self.view_buffer, &self.palette, &self.sun);
        }
    }

    /// The maps' own uniforms, written after the passes' from slot `first`: each looks from the sun
    /// at its box, from its view's origin.
    pub(super) fn sun_map_uniforms(&self, uniforms: &mut Vec<u8>, first: usize) {
        uniforms.resize((first + self.sun.maps.len()) * VIEW_STRIDE as usize, 0);
        for (l, map) in self.sun.maps.iter().enumerate() {
            let o = map.origin;
            let u = ViewUniform {
                vp: map.matrix().0,
                eye: [o.x, o.y, o.z, 0.0],
                origin: [o.x, o.y, o.z, 0.0],
                fog: [1e9, 2e9, 0.0, 0.0],
                ..bytemuck::Zeroable::zeroed()
            };
            let at = (first + l) * VIEW_STRIDE as usize;
            uniforms[at..at + std::mem::size_of::<ViewUniform>()].copy_from_slice(bytemuck::bytes_of(&u));
        }
    }

    /// The casters gathered into batches by map and model, moved by their view's origin, and
    /// uploaded.
    pub(super) fn upload_sun_casters(&mut self) {
        let sun = &mut self.sun;
        sun.scratch.sort_unstable_by_key(|(k, _)| *k);
        sun.batches.clear();
        sun.flat.clear();
        for (i, (key, inst)) in sun.scratch.iter().enumerate() {
            let (map, model, group) = ((key >> 48) as usize, ((key >> 16) & 0xffff_ffff) as usize, (key & 0xffff) as usize);
            match sun.batches.last_mut() {
                Some(b) if b.view == map && b.model == model && b.group == group => b.count += 1,
                _ => sun.batches.push(Batch { view: map, model, group, ghost: 0, first: i as u32, count: 1 }),
            }
            let mut inst = *inst;
            let o = sun.maps[map].origin;
            inst.m[3][0] -= o.x;
            inst.m[3][1] -= o.y;
            inst.m[3][2] -= o.z;
            sun.flat.push(inst);
        }
        if sun.flat.len() as u64 > sun.capacity {
            sun.capacity = (sun.flat.len() as u64).next_power_of_two();
            sun.instances = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sun caster instances"),
                size: sun.capacity * std::mem::size_of::<InstanceRaw>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !sun.flat.is_empty() {
            self.queue.write_buffer(&sun.instances, 0, bytemuck::cast_slice(&sun.flat));
        }
    }

    /// Each map drawn, depth only, its uniforms at slot `first + map`; how many draws that took.
    pub(super) fn draw_sun_maps(&self, encoder: &mut wgpu::CommandEncoder, first: usize) -> usize {
        let sun = &self.sun;
        let Some(bind) = &sun.bind else { return 0 };
        let stride = std::mem::size_of::<InstanceRaw>() as u64;
        let mut draws = 0;
        let mut b = 0;
        for l in 0..sun.maps.len() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sun map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &sun.layers[l],
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let end = (b..sun.batches.len()).find(|&i| sun.batches[i].view != l).unwrap_or(sun.batches.len());
            let mine = &sun.batches[b..end];
            b = end;
            if mine.is_empty() {
                continue;
            }
            pass.set_pipeline(&sun.pipeline);
            pass.set_bind_group(0, bind, &[((first + l) as u64 * VIEW_STRIDE) as u32]);
            let mut bound = None;
            for batch in mine {
                let model = &self.models[batch.model];
                // Only what hides what is behind it casts: not the glows added over the picture,
                // nor the see-through runs that write no depth.
                let casts = |material: usize| {
                    let k = self.materials[material].pipeline;
                    k.depth_write && k.blend != super::Blend::Add
                };
                let ranges = model.group(batch.group);
                if !ranges.iter().any(|r| casts(r.material)) {
                    continue;
                }
                pass.set_vertex_buffer(0, model.vertices.slice(..));
                pass.set_vertex_buffer(1, sun.instances.slice(batch.first as u64 * stride..(batch.first + batch.count) as u64 * stride));
                for r in ranges.iter().filter(|r| casts(r.material)) {
                    if bound != Some(r.material) {
                        pass.set_bind_group(1, &self.materials[r.material].bind, &[]);
                        bound = Some(r.material);
                    }
                    pass.draw(r.start..r.start + r.count, 0..batch.count);
                    draws += 1;
                }
            }
        }
        draws
    }
}

/// What the ground's and the models' shaders read the maps with: how hidden from the sun a point
/// is, relative to the view's origin, 0 to 1. Each map is read over the 3x3 texels about the point,
/// compared (soft edges; the original blurs its map once). Ours take the first map that holds the
/// point and fade out over the last fifth of their reach; the original's take the darkest, and
/// fade with how far under the caster the point is.
pub(super) const SHADOW_WGSL: &str = r#"
@group(0) @binding(2) var sun_map: texture_depth_2d_array;
@group(0) @binding(3) var sun_cmp: sampler_comparison;

fn sun_hidden(p: vec3f) -> f32 {
  let count = u32(view.sun_maps.y);
  let first = i32(view.sun_maps.z);
  var hidden = 0.0;
  for (var k = 0u; k < count; k = k + 1u) {
    let layer = view.sun_layer[k];
    let c = rows(view.sun_m[k], p);
    let uv = vec2f(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if (any(uv <= vec2f(0.0)) || any(uv >= vec2f(1.0)) || c.z <= 0.0 || c.z >= 1.0) {
      continue;
    }
    var lit = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
      for (var x = -1; x <= 1; x = x + 1) {
        lit = lit + textureSampleCompareLevel(sun_map, sun_cmp, uv + vec2f(f32(x), f32(y)) * layer.z, first + i32(k), c.z + layer.y);
      }
    }
    var h = (1.0 - lit / 9.0) * layer.x;
    if (view.sun_maps.x > 1.5) {
      let far = max(view.sun_maps.w, 1.0);
      return h * clamp((far - length(p - (view.eye.xyz - view.origin.xyz))) / (0.2 * far), 0.0, 1.0);
    }
    if (layer.w > 0.5) {
      h = h * clamp(c.z / ORIGINAL_FADE, 0.0, 1.0);
    }
    hidden = max(hidden, h);
  }
  return hidden;
}
"#;

/// [`SHADOW_WGSL`] with its constants written in.
pub(super) fn shadow_wgsl() -> String {
    SHADOW_WGSL.replace("ORIGINAL_FADE", &format!("{ORIGINAL_FADE:?}"))
}
