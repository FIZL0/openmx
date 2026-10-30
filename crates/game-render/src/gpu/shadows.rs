//! The sun's shadow maps on the GPU (V108, [`crate::shadow`]): each view's maps drawn from the sun,
//! depth only, before the views, and read by the ground's and the models' shaders through the
//! view's uniforms ([`SHADOW_WGSL`]).
//!
//! At our qualities ([`ShadowQuality::Low`], [`ShadowQuality::High`]) a view has two or three maps
//! nested along its look ([`ShadowQuality::cascades`]), into which what casts ([`shadow::casts`])
//! near casts: the actors, and under [`Shadows::All`] the props and the plants too. At
//! [`ShadowQuality::Original`] each racer — the actors whose spheres touch, made one — has its own
//! small map ([`shadow::ORIGINAL_SIZE`]) fitted round it, as the original's projected shadow is, and
//! only the ground receives it. Under [`Shadows::All`] the whole static world, its props and plants,
//! is drawn once into one more map ([`World`]) when it loads, read where the view's maps do not
//! reach; it never follows the camera, so nothing in it comes or goes.

use super::{Batch, DEPTH, actor_sphere, InstanceRaw, MeshVertex, Renderer, VIEW_STRIDE, View, ViewUniform};
use crate::mat::{M4, V3, v3};
use crate::shadow::{self, Caster, ShadowQuality, Shadows, Tint};

/// The most maps one view reads: the view's uniforms hold this many.
pub(super) const MAPS: usize = 8;

/// The biggest actor that casts the original's shadow, its radius in feet: a racer and their
/// machine, not the train.
const ORIGINAL_MOST: f32 = 20.0;

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
    /// texels it filters over, and for the original's the share of its depth it fades over
    /// ([`shadow::original_box`]; ours keep their reach there).
    layer: [f32; 4],
    /// The actors that cast into it (the original's, one racer), or `None` for everything near.
    actors: Option<Vec<usize>>,
    /// The baked world map's ([`World`]), drawn this frame: read by no pass of its own.
    world: bool,
}

impl Map {
    fn matrix(&self) -> M4 {
        shadow::light_box(self.sun, self.centre, self.half, self.back, self.ahead, self.origin)
    }

    fn holds(&self, c: V3, r: f32) -> bool {
        shadow::in_box(self.sun, self.centre, self.half, self.back, self.ahead, c, r)
    }
}

/// The baked world map ([`Shadows::All`]): its texture, and the box it was last drawn in with what
/// was drawn into it ([`World::key`]), so it is drawn again only when that changes.
pub(super) struct World {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    size: u32,
    /// The box it holds: its sun, centre, half across, and depth back and ahead.
    held: Option<(V3, V3, f32, f32, f32)>,
    /// What it was drawn from: the world's generation, its side, the sun, its centre, and how many
    /// props and plant models had reached the GPU (they come a frame or more after the scene).
    key: Option<[u64; 6]>,
    /// The world's middle and radius by its props' and plants' places, for the generation `.0`.
    bounds: Option<(u64, V3, f32)>,
    /// Bumped whenever the world's props or plants change ([`Renderer::set_scene`] and the rest).
    pub(super) generation: u64,
}

fn world_texture(device: &wgpu::Device, size: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sun world map"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2),
        ..Default::default()
    });
    (texture, view)
}

/// The shadow maps and how they are drawn.
pub(super) struct SunMaps {
    pub(super) mode: Shadows,
    pub(super) quality: ShadowQuality,
    pub(super) world: World,
    /// The passes planned this frame with a sun: those that read the baked world map.
    planned: Vec<usize>,
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

/// The view layout's entries for the maps: the array, its comparing sampler, and the baked world map.
pub(super) fn layout_entries() -> [wgpu::BindGroupLayoutEntry; 3] {
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
        wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Depth,
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
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
        let (world_tex, world_view) = world_texture(device, 1);
        SunMaps {
            mode: Shadows::default(),
            quality: ShadowQuality::default(),
            world: World { texture: world_tex, view: world_view, size: 1, held: None, key: None, bounds: None, generation: 0 },
            planned: Vec::new(),
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

    /// Whether pass `p` has maps `caster` casts into: ours, under the setting ([`shadow::casts`]).
    /// The original's take their racers alone, whatever it is.
    pub(super) fn takes(&self, p: usize, caster: Caster) -> bool {
        shadow::casts(self.mode, caster) && self.maps.iter().any(|m| m.pass == p && m.actors.is_none() && !m.world)
    }

    /// The baked world map's box and what the shader reads with it, while it is read.
    fn world_held(&self) -> Option<((V3, V3, f32, f32, f32), [f32; 4])> {
        let held = self.world.held.filter(|_| self.mode == Shadows::All)?;
        let (_, _, half, back, ahead) = held;
        let texel = 2.0 * half / self.world.size as f32;
        Some((held, [1.0, 1.5 * texel / (back + ahead), 1.0 / self.world.size as f32, 0.0]))
    }

    /// How many maps this frame draws.
    pub(super) fn count(&self) -> usize {
        self.maps.len()
    }

    /// Pass `p`'s shadow uniforms, its view's coordinates starting at `origin`: the mode (0 none, 1
    /// the original's, 2 Low's, 3 High's), how many maps it reads and the first, and how far ours
    /// reach; the tint; each map's [`Map::layer`] and matrix; and the baked world map's.
    pub(super) fn fields(&self, p: usize, origin: V3) -> SunFields {
        let mut f = SunFields { maps: [0.0; 4], tint: [0.0; 4], layer: [[0.0; 4]; MAPS], m: [[[0.0; 4]; 4]; MAPS], world: [0.0; 4], world_m: [[0.0; 4]; 4] };
        let first = self.maps.iter().position(|m| m.pass == p && !m.world);
        let world = self.world_held().filter(|_| self.planned.contains(&p));
        if first.is_none() && world.is_none() {
            return f;
        }
        let first = first.unwrap_or(0);
        let mine = &self.maps[first.min(self.maps.len())..];
        let count = mine.iter().take_while(|x| x.pass == p && !x.world).count().min(MAPS);
        for (k, map) in mine[..count].iter().enumerate() {
            f.layer[k] = map.layer;
            f.m[k] = map.matrix().0;
        }
        if let Some(((sun, centre, half, back, ahead), layer)) = world {
            f.world = layer;
            f.world_m = shadow::light_box(sun, centre, half, back, ahead, origin).0;
        }
        let reach = if count > 0 { mine[count - 1].layer[3] } else { 0.0 };
        let (mode, tint, reach) = match self.quality {
            // The strength squared: in PCSX2 free06's shadow (25,25,25 at 0.7) leaves 0.63 of the
            // dirt, as 0.7 x 0.7 does and 0.7 does not. Read as the intensity weighing both the
            // caster's alpha and the receiver's; the V108 journal has the measurement.
            ShadowQuality::Original => (1.0, [self.tint.rgb[0], self.tint.rgb[1], self.tint.rgb[2], self.tint.alpha * self.tint.alpha], 0.0),
            // Ours keep how far they reach in the kind slot ([`Renderer::plan_sun_maps`]).
            ShadowQuality::Low => (2.0, [self.levels.0, self.levels.1, 0.0, 0.0], reach),
            ShadowQuality::High => (3.0, [self.levels.0, self.levels.1, 0.0, 0.0], reach),
        };
        f.maps = [mode, count as f32, first as f32, reach];
        f.tint = tint;
        f
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

/// One pass's shadow uniforms ([`SunMaps::fields`]), as [`ViewUniform`] holds them.
pub(super) struct SunFields {
    pub(super) maps: [f32; 4],
    pub(super) tint: [f32; 4],
    pub(super) layer: [[f32; 4]; MAPS],
    pub(super) m: [[[f32; 4]; 4]; MAPS],
    pub(super) world: [f32; 4],
    pub(super) world_m: [[f32; 4]; 4],
}

impl Renderer {
    /// What casts the sun's shadows and how they are drawn, from the next frame (V108, user
    /// 2026-10-30).
    pub fn set_shadows(&mut self, shadows: Shadows, quality: ShadowQuality) {
        (self.sun.mode, self.sun.quality) = (shadows, quality);
    }

    /// The shadows set, as [`Renderer::set_shadows`] took them.
    pub fn shadows(&self) -> (Shadows, ShadowQuality) {
        (self.sun.mode, self.sun.quality)
    }

    /// The colour and strength Original's shadows lie in, off the scene's sun (V108).
    pub fn shadow_tint(&self) -> Tint {
        self.sun.tint
    }

    /// The last frame's maps, each its centre and how many copies were cast into it (for the tests).
    #[cfg(test)]
    pub(super) fn sun_map_centres(&self) -> Vec<(V3, usize)> {
        let cast = |l: usize| self.sun.scratch.iter().filter(|(k, _)| (k >> 48) as usize == l).count();
        self.sun.maps.iter().enumerate().map(|(l, m)| (m.centre, cast(l))).collect()
    }

    /// This frame's maps, before its uniforms are written: for each view's own pass (not a wrapped
    /// world's copy's) under the setting, ours along its look or the original's round each racer;
    /// and under [`Shadows::All`] the baked world map, when what it holds has changed.
    pub(super) fn plan_sun_maps(&mut self, views: &[View], passes: &[(usize, Option<[i32; 2]>, usize)], frame_light: &crate::world::Lighting, poses: &[Vec<M4>]) {
        self.sun.maps.clear();
        self.sun.scratch.clear();
        self.sun.planned.clear();
        if self.sun.mode == Shadows::Off {
            return;
        }
        let quality = self.sun.quality;
        let most = self.device.limits().max_texture_dimension_2d;
        let size = quality.size(views.len(), cfg!(target_arch = "wasm32"), most);
        let mut maps = Vec::new();
        // The first view's sun and eye, which the baked world map is drawn by.
        let mut first_sun = None;
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
            self.sun.planned.push(p);
            first_sun.get_or_insert((sun, origin));
            match quality {
                ShadowQuality::Original => {
                    let mut spheres = Vec::new();
                    let mut who = Vec::new();
                    for (i, a) in view.actors.iter().enumerate() {
                        let Some(Some(m)) = self.model_ids.get(a.model).copied() else { continue };
                        if a.alpha < 1.0 && !a.casts {
                            continue;
                        }
                        // Round its `at`, as the original fits it, unless its pose carries it off
                        // there: the wreck body is drawn with the tumbling bike's `at` (365f8e9e's
                        // [`actor_sphere`]), and a box round the bike lost a body thrown 25-40 ft.
                        let radius = self.models[m].radius;
                        let (bind, r) = actor_sphere(&a.at, &[], radius);
                        let pose = a.pose.and_then(|p| poses.get(p)).map_or(&[][..], |p| p.as_slice());
                        let (posed, posed_r) = actor_sphere(&a.at, pose, radius);
                        let (c, r) = if posed.sub(bind).len() > r { (posed, posed_r) } else { (bind, r) };
                        if !(r > 0.0) || r > ORIGINAL_MOST {
                            continue;
                        }
                        spheres.push((c, r));
                        who.push(i);
                    }
                    // The nearest racers first: with more than [`MAPS`] in reach, which got a map was
                    // the list's order, and a racer's shadow came and went as others joined.
                    let mut racers = shadow::groups(&spheres);
                    racers.sort_by(|a, b| a.0.sub(camera.eye).len().total_cmp(&b.0.sub(camera.eye).len()));
                    for (c, r, held) in racers.into_iter().take(MAPS) {
                        // The port's reach, not the original's 100 ft ([`shadow::PORTED_RANGE`]).
                        let fade = shadow::ported_fade(c.sub(camera.eye).len());
                        if fade <= 0.0 {
                            continue;
                        }
                        // Deep enough along the sun for the whole of a long shadow's footprint.
                        let (half, back, ahead, depth_fade) = shadow::original_box(sun, r);
                        let texel = 2.0 * half / size as f32;
                        maps.push(Map {
                            pass: p,
                            origin,
                            sun,
                            centre: c,
                            half,
                            back,
                            ahead,
                            // The filter steps two texels: the original blurs its 128x128 shadow, and in PCSX2
                            // its edge is that soft, not one texel's.
                            layer: [fade, 2.0 * texel / (back + ahead), 2.0 / size as f32, depth_fade],
                            actors: Some(held.into_iter().map(|k| who[k]).collect()),
                            world: false,
                        });
                    }
                }
                _ => {
                    let [_, _, w, h] = view.rect;
                    let aspect = w as f32 / h.max(1) as f32;
                    let forward = camera.at.sub(camera.eye).norm();
                    let (_, far) = self.fog(&light);
                    let cascades = quality.cascades();
                    let reach = cascades[cascades.len() - 1].min(far);
                    for (near, far) in shadow::slices(cascades, reach) {
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
                            world: false,
                        });
                    }
                }
            }
        }
        self.sun.maps = maps;
        let wanted = self.sun.maps.len() as u32;
        let mut rebind = false;
        if self.sun.size != size || (self.sun.layers.len() as u32) < wanted {
            let (texture, array, layers) = maps_texture(&self.device, size, wanted.max(self.sun.layers.len() as u32 * u32::from(self.sun.size == size)));
            (self.sun.texture, self.sun.array, self.sun.layers, self.sun.size) = (texture, array, layers, size);
            rebind = true;
        }
        if self.sun.mode == Shadows::All {
            if let Some((sun, eye)) = first_sun {
                rebind |= self.plan_world_map(sun, eye, quality.world_size(if cfg!(target_arch = "wasm32") { most.min(shadow::WORLD_SIZE / 2) } else { most }));
            }
        }
        if rebind {
            self.view_bind = super::view_bind_group(&self.device, &self.view_layout, &self.view_buffer, &self.palette, &self.sun);
        }
    }

    /// The baked world map ([`Shadows::All`]), `size` texels a side, for a sun along `sun` and the
    /// first view's `eye`: drawn this frame (one more map, after the views', read by none of them as
    /// their own) if what it would hold is not what it holds. It holds the whole world, its props and
    /// plants (not the ground: the light maps hold the hills' shadows already), in a box about the
    /// world's middle; a world wider than [`shadow::WORLD_HALF_MOST`] about a point the eye is near,
    /// moved in steps of half that. Whether its texture is new, so the views' bind group must be.
    fn plan_world_map(&mut self, sun: V3, eye: V3, size: u32) -> bool {
        let generation = self.sun.world.generation;
        if self.sun.world.bounds.is_none_or(|(g, _, _)| g != generation) {
            let mut lo = v3(f32::MAX, f32::MAX, f32::MAX);
            let mut hi = v3(f32::MIN, f32::MIN, f32::MIN);
            let mut take = |c: V3| {
                (lo.x, lo.y, lo.z) = (lo.x.min(c.x), lo.y.min(c.y), lo.z.min(c.z));
                (hi.x, hi.y, hi.z) = (hi.x.max(c.x), hi.y.max(c.y), hi.z.max(c.z));
            };
            for prop in self.props.iter().filter(|p| !p.moved) {
                take(v3(prop.at.0[3][0], prop.at.0[3][1], prop.at.0[3][2]));
            }
            for grove in self.grove.iter().chain(self.groves.iter().map(|(_, g)| g)) {
                for plant in &grove.ecosystem.plants {
                    take(plant.pos);
                }
            }
            let bounds = if lo.x <= hi.x { (lo.add(hi).scale(0.5), hi.sub(lo).len() * 0.5 + 100.0) } else { (eye, 0.0) };
            self.sun.world.bounds = Some((generation, bounds.0, bounds.1));
        }
        let Some((_, middle, radius)) = self.sun.world.bounds else { return false };
        if !(radius > 0.0) {
            self.sun.world.held = None;
            return false;
        }
        let (centre, half) = if radius <= shadow::WORLD_HALF_MOST {
            (middle, radius)
        } else {
            let step = shadow::WORLD_HALF_MOST * 0.5;
            (v3((eye.x / step).round() * step, eye.y, (eye.z / step).round() * step), shadow::WORLD_HALF_MOST)
        };
        let centre = shadow::snapped(sun, centre, half, size);
        let props = self.props.iter().filter(|p| !p.moved && matches!(p.model, Some(Some(_)))).count();
        let parts = self
            .grove
            .iter()
            .chain(self.groves.iter().map(|(_, g)| g))
            .map(|g| g.parts().filter(|n| matches!(self.model_ids.get(*n), Some(Some(_)))).count())
            .sum::<usize>();
        let bits = |v: V3| ((v.x.to_bits() as u64) << 32) ^ ((v.y.to_bits() as u64) << 16) ^ v.z.to_bits() as u64;
        let key = [generation, size as u64, bits(sun.norm()), bits(centre), props as u64, parts as u64];
        let mut fresh = false;
        if self.sun.world.size != size {
            let (texture, view) = world_texture(&self.device, size);
            (self.sun.world.texture, self.sun.world.view, self.sun.world.size) = (texture, view, size);
            self.sun.world.key = None;
            fresh = true;
        }
        if self.sun.world.key == Some(key) {
            return fresh;
        }
        // Deep enough along the sun for the whole world's height, and a tall prop's shadow.
        let (back, ahead) = (half + shadow::REACH_BACK, half + shadow::REACH_BACK);
        self.sun.world.key = Some(key);
        self.sun.world.held = Some((sun, centre, half, back, ahead));
        let l = self.sun.maps.len();
        self.sun.maps.push(Map {
            pass: usize::MAX,
            origin: centre,
            sun,
            centre,
            half,
            back,
            ahead,
            layer: [0.0; 4],
            actors: None,
            world: true,
        });
        let map = self.sun.maps[l].clone();
        let mut casts = Vec::new();
        for prop in self.props.iter().filter(|p| !p.moved) {
            let Some(Some(m)) = prop.model else { continue };
            let model = &self.models[m];
            let c = v3(prop.at.0[3][0], prop.at.0[3][1], prop.at.0[3][2]);
            if map.holds(c, model.radius) {
                casts.push((m, InstanceRaw { m: prop.at.0, palette: [model.bind_palette, model.nodes], cover: 1.0 }));
            }
        }
        let mut plants = Vec::new();
        for grove in self.grove.iter().chain(self.groves.iter().map(|(_, g)| g)) {
            let species = grove.ecosystem.species.len();
            let ids: Vec<Option<usize>> = grove.parts().map(|n| self.model_ids.get(n).copied().flatten()).collect();
            plants.clear();
            grove.casters(centre, half * 1.5 + back, &mut plants);
            for d in &plants {
                let Some(m) = ids[d.part_index(species)] else { continue };
                let model = &self.models[m];
                let row = |r: usize| v3(d.at.0[r][0], d.at.0[r][1], d.at.0[r][2]);
                let radius = model.radius * row(0).len().max(row(1).len()).max(row(2).len());
                if map.holds(row(3), radius) {
                    casts.push((m, InstanceRaw { m: d.at.0, palette: [model.bind_palette, model.nodes], cover: 1.0 }));
                }
            }
        }
        for (m, inst) in casts {
            self.sun.scratch.push((((l as u64) << 48) | ((m as u64) << 16), inst));
        }
        fresh
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
            let target = if sun.maps[l].world { &sun.world.view } else { &sun.layers[l] };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sun map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: target,
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
/// is, relative to the view's origin, 0 to 1. Low's maps, and the original's, are read over the 3x3
/// texels about the point, compared (soft edges; the original blurs its map once). High's are read
/// as PCSS reads them ([`shadow::penumbra`]): a search about the point for casters, compared at
/// [`shadow::BLOCKER_STEPS`]' heights over it rather than read (a comparing sampler is all WebGL2
/// gives a depth map), says how far over it they stand, and the edge is filtered that wide. Ours
/// take the first map that holds the point, handing over to the next across the last
/// [`shadow::CASCADE_BLEND`] of it ([`shadow::cascade_share`]), and fade out smoothly over the last
/// [`shadow::REACH_FADE`] of their reach ([`shadow::reach_fade`]), the baked world map
/// ([`Shadows::All`]) taking over what they leave; the original's take the darkest, and fade with
/// how far under the caster the point is ([`shadow::original_box`]).
pub(super) const SHADOW_WGSL: &str = r#"
@group(0) @binding(2) var sun_map: texture_depth_2d_array;
@group(0) @binding(3) var sun_cmp: sampler_comparison;
@group(0) @binding(4) var sun_world_map: texture_depth_2d;

fn sun_box(uv: vec2f, l: i32, z: f32, step: f32) -> f32 {
  var lit = 0.0;
  for (var y = -1; y <= 1; y = y + 1) {
    for (var x = -1; x <= 1; x = x + 1) {
      lit = lit + textureSampleCompareLevel(sun_map, sun_cmp, uv + vec2f(f32(x), f32(y)) * step, l, z);
    }
  }
  return lit / 9.0;
}

fn sun_soft(uv: vec2f, l: i32, z: f32, layer: vec4f, m: mat4x4f) -> f32 {
  // The map's half width and depth in feet, off its matrix ([`shadow::light_box`]).
  let half = 1.0 / max(length(vec3f(m[0].x, m[1].x, m[2].x)), 1e-6);
  let span = 1.0 / max(length(vec3f(m[0].z, m[1].z, m[2].z)), 1e-6);
  let texel = 2.0 * half * layer.z;
  let search = max(PENUMBRA_MOST, 2.0 * texel) / (2.0 * half);
  var near = 0.0;
  var mid = 0.0;
  var far = 0.0;
  var q = uv;
SEARCH_TAPS
  near = 1.0 - near / SEARCH_COUNT;
  mid = 1.0 - mid / SEARCH_COUNT;
  far = 1.0 - far / SEARCH_COUNT;
  if (near <= 0.0) {
    return textureSampleCompareLevel(sun_map, sun_cmp, uv, l, z + layer.y);
  }
  let height = (max(near - mid, 0.0) * 1.0 + max(mid - far, 0.0) * 8.0 + far * 30.0) / near;
  let r = clamp(height * PENUMBRA, texel, max(PENUMBRA_MOST, texel)) / (2.0 * half);
  let bias = z + layer.y * max(1.0, 0.5 * r / layer.z);
  var lit = 0.0;
FILTER_TAPS
  return lit / FILTER_COUNT;
}

fn sun_world(p: vec3f) -> f32 {
  if (view.sun_world.x < 0.5) {
    return 0.0;
  }
  let c = rows(view.sun_world_m, p);
  let edge = max(abs(c.x), abs(c.y));
  if (edge >= 1.0 || c.z <= 0.0 || c.z >= 1.0) {
    return 0.0;
  }
  let uv = vec2f(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
  var lit = 0.0;
  for (var y = -1; y <= 1; y = y + 1) {
    for (var x = -1; x <= 1; x = x + 1) {
      lit = lit + textureSampleCompareLevel(sun_world_map, sun_cmp, uv + vec2f(f32(x), f32(y)) * view.sun_world.z, c.z + view.sun_world.y);
    }
  }
  return (1.0 - lit / 9.0) * (1.0 - smoothstep(0.95, 1.0, edge));
}

fn sun_hidden(p: vec3f) -> f32 {
  let count = u32(view.sun_maps.y);
  let first = i32(view.sun_maps.z);
  let ours = view.sun_maps.x > 1.5;
  let soft = view.sun_maps.x > 2.5;
  var hidden = 0.0;
  var left = 1.0;
  for (var k = 0u; k < count; k = k + 1u) {
    let layer = view.sun_layer[k];
    let c = rows(view.sun_m[k], p);
    let uv = vec2f(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if (any(uv <= vec2f(0.0)) || any(uv >= vec2f(1.0)) || c.z <= 0.0 || c.z >= 1.0) {
      continue;
    }
    var lit = 0.0;
    if (soft) {
      lit = sun_soft(uv, first + i32(k), c.z, layer, view.sun_m[k]);
    } else {
      lit = sun_box(uv, first + i32(k), c.z + layer.y, layer.z);
    }
    var h = (1.0 - lit) * layer.x;
    if (ours) {
      var w = 1.0;
      if (k + 1u < count) {
        w = 1.0 - clamp((max(abs(c.x), abs(c.y)) - (1.0 - CASCADE_BLEND)) / CASCADE_BLEND, 0.0, 1.0);
      }
      hidden = hidden + left * w * h;
      left = left * (1.0 - w);
      if (left <= 0.0) {
        break;
      }
      continue;
    }
    if (layer.w > 0.0) {
      h = h * clamp(c.z / layer.w, 0.0, 1.0);
    }
    hidden = max(hidden, h);
  }
  let world = sun_world(p);
  if (ours) {
    let far = max(view.sun_maps.w, 1.0);
    let fade = 1.0 - smoothstep(far * (1.0 - REACH_FADE), far, length(p - (view.eye.xyz - view.origin.xyz)));
    // What the view's maps cover they say alone; the world map says the rest.
    let w = world * (1.0 - (1.0 - left) * fade);
    return 1.0 - (1.0 - hidden * fade) * (1.0 - w);
  }
  return max(hidden, world);
}
"#;

/// `n` taps over the unit disc, evenly (a Vogel spiral): High's search and filter.
fn disc_taps(n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let r = ((i as f32 + 0.5) / n as f32).sqrt();
            let a = i as f32 * 2.399_963;
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

/// High's search and filter taps.
const SEARCH: usize = 8;
const FILTER: usize = 16;

/// [`SHADOW_WGSL`] with its constants and High's taps written in.
pub(super) fn shadow_wgsl() -> String {
    let f = |x: f32| format!("{x:?}");
    let search: String = disc_taps(SEARCH)
        .into_iter()
        .map(|(a, b)| {
            format!(
                "  q = uv + vec2f({}, {}) * search;\n  near = near + textureSampleCompareLevel(sun_map, sun_cmp, q, l, z + layer.y);\n  mid = mid + textureSampleCompareLevel(sun_map, sun_cmp, q, l, z + {} / span);\n  far = far + textureSampleCompareLevel(sun_map, sun_cmp, q, l, z + {} / span);\n",
                f(a),
                f(b),
                f(shadow::BLOCKER_STEPS[0]),
                f(shadow::BLOCKER_STEPS[1])
            )
        })
        .collect();
    let filter: String = disc_taps(FILTER)
        .into_iter()
        .map(|(a, b)| format!("  lit = lit + textureSampleCompareLevel(sun_map, sun_cmp, uv + vec2f({}, {}) * r, l, bias);\n", f(a), f(b)))
        .collect();
    SHADOW_WGSL
        .replace("SEARCH_TAPS\n", &search)
        .replace("FILTER_TAPS\n", &filter)
        .replace("SEARCH_COUNT", &f(SEARCH as f32))
        .replace("FILTER_COUNT", &f(FILTER as f32))
        .replace("PENUMBRA_MOST", &f(shadow::PENUMBRA_MOST))
        .replace("PENUMBRA", &f(shadow::PENUMBRA))
        .replace("CASCADE_BLEND", &f(shadow::CASCADE_BLEND))
        .replace("REACH_FADE", &f(shadow::REACH_FADE))
}
