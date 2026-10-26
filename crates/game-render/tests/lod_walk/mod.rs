//! A walk from far to near on the GPU, judged patch by patch (V85): a texture whose level changes
//! as the camera nears it should sharpen over the walk, not in one frame. Each frame is cut into
//! patches fixed on the thing walked to (a card's texture, a stretch of ground), so the camera's
//! own motion moves nothing from one patch to another; a patch's coverage, mean colour and detail
//! (the spread of its pixels' brightness) are then compared frame to frame.
//!
//! Shared by `texture_lod.rs` and `ecosystem.rs` (the disc's bush on a card).

#![allow(dead_code)]

use game_render::gpu::{Renderer, Scene3, Target, View};
use game_render::mat::{M4, V3, v3};
use game_render::psg::{Mesh, Model, Node, Strip, Vertex};
use game_render::scene::{Placement, Scene};
use game_render::world::{Assets, Camera, Lighting, Loaded, Surface};
use game_render::scene::Shading;
use game_render::Texture;

/// The frame walked in, as wide as a 4:3 ride's.
pub const W: u32 = 1280;
pub const H: u32 = 960;
/// Each frame of a walk is this much nearer than the one before: 1%, so at 60 frames a second a
/// level's octave takes a second and a bit.
pub const STEP: f32 = 0.99;
/// A patch is judged only while it covers this many pixels: a smaller one is a handful of pixels,
/// and its numbers are the rasteriser's rounding more than the texture.
pub const MIN_AREA: u32 = 1500;

/// What a patch of one frame drew.
#[derive(Clone, Copy, Debug, Default)]
pub struct Patch {
    /// Pixels of the frame in the patch; a patch the frame's edge cuts is not judged.
    pub area: u32,
    pub cut: bool,
    pub covered: u32,
    sum: [f64; 3],
    luma: f64,
    luma2: f64,
}

impl Patch {
    pub fn coverage(&self) -> f32 {
        self.covered as f32 / self.area.max(1) as f32
    }
    pub fn mean(&self) -> [f32; 3] {
        self.sum.map(|s| (s / self.covered.max(1) as f64) as f32)
    }
    /// The spread of the covered pixels' brightness: how sharp the patch looks.
    pub fn detail(&self) -> f32 {
        let n = self.covered.max(1) as f64;
        let m = self.luma / n;
        (self.luma2 / n - m * m).max(0.0).sqrt() as f32
    }
}

/// The most any judged patch changed between two frames of a walk, and where (the distance of
/// the nearer frame).
#[derive(Clone, Copy, Debug, Default)]
pub struct Steps {
    pub coverage: (f32, f32),
    pub colour: (f32, f32),
    pub detail: (f32, f32),
}

impl Steps {
    fn take(slot: &mut (f32, f32), v: f32, d: f32) {
        if v > slot.0 {
            *slot = (v, d);
        }
    }
}

/// The worst steps of a walk, its frames' patches given with their distances.
pub fn worst(frames: &[(f32, Vec<Patch>)]) -> Steps {
    let mut s = Steps::default();
    let trace = std::env::var_os("V85_TRACE").is_some();
    for pair in frames.windows(2) {
        let ((_, a), (d, b)) = (&pair[0], &pair[1]);
        let before = s;
        s = Steps::default();
        for (a, b) in a.iter().zip(b) {
            if a.area < MIN_AREA || b.area < MIN_AREA || a.cut || b.cut {
                continue;
            }
            Steps::take(&mut s.coverage, (a.coverage() - b.coverage()).abs(), *d);
            // Colour and detail only where both frames drew enough of the patch to have one.
            if a.covered * 4 < a.area || b.covered * 4 < b.area {
                continue;
            }
            let (ma, mb) = (a.mean(), b.mean());
            Steps::take(&mut s.colour, (0..3).map(|k| (ma[k] - mb[k]).abs()).fold(0.0, f32::max), *d);
            Steps::take(&mut s.detail, (a.detail() - b.detail()).abs(), *d);
        }
        if trace {
            println!("trace {d:7.2} coverage {:.3} colour {:5.2} detail {:5.2}", s.coverage.0, s.colour.0, s.detail.0);
        }
        for (slot, was) in [(&mut s.coverage, before.coverage), (&mut s.colour, before.colour), (&mut s.detail, before.detail)] {
            if was.0 > slot.0 {
                *slot = was;
            }
        }
    }
    s
}

/// The inverse of a view's projection, for [`ray`].
pub fn unprojection(camera: &Camera) -> [[f64; 4]; 4] {
    let m = camera.view_projection(W as usize, H as usize).0;
    let mut a = [[0.0f64; 8]; 4];
    for r in 0..4 {
        for c in 0..4 {
            a[r][c] = m[r][c] as f64;
        }
        a[r][4 + r] = 1.0;
    }
    for c in 0..4 {
        let p = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs())).unwrap();
        a.swap(c, p);
        let k = a[c][c];
        for x in &mut a[c] {
            *x /= k;
        }
        for r in 0..4 {
            if r != c {
                let f = a[r][c];
                for x in 0..8 {
                    a[r][x] -= f * a[c][x];
                }
            }
        }
    }
    std::array::from_fn(|r| std::array::from_fn(|c| a[r][4 + c]))
}

/// The ray through pixel (`x`, `y`)'s middle: a point on it and its direction.
pub fn ray(inv: &[[f64; 4]; 4], x: u32, y: u32) -> ([f64; 3], [f64; 3]) {
    let nx = (x as f64 + 0.5) / W as f64 * 2.0 - 1.0;
    let ny = 1.0 - (y as f64 + 0.5) / H as f64 * 2.0;
    let at = |nz: f64| {
        let c = [nx, ny, nz, 1.0];
        let p: [f64; 4] = std::array::from_fn(|k| (0..4).map(|r| c[r] * inv[r][k]).sum());
        [p[0] / p[3], p[1] / p[3], p[2] / p[3]]
    };
    let (a, b) = (at(0.1), at(0.5));
    (a, [b[0] - a[0], b[1] - a[1], b[2] - a[2]])
}

/// Where a ray meets the plane through `origin` spanned by `u` and `v` (unit and square to each
/// other): its place along each, or `None` when it does not meet it ahead.
pub fn on_plane((p, d): ([f64; 3], [f64; 3]), origin: V3, u: V3, v: V3) -> Option<(f32, f32)> {
    let n = u.cross(v);
    let (n, o) = ([n.x as f64, n.y as f64, n.z as f64], [origin.x as f64, origin.y as f64, origin.z as f64]);
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let den = dot(n, d);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = dot(n, [o[0] - p[0], o[1] - p[1], o[2] - p[2]]) / den;
    if t <= 0.0 {
        return None;
    }
    let hit = [p[0] + d[0] * t - o[0], p[1] + d[1] * t - o[1], p[2] + d[2] * t - o[2]];
    Some((dot(hit, [u.x as f64, u.y as f64, u.z as f64]) as f32, dot(hit, [v.x as f64, v.y as f64, v.z as f64]) as f32))
}

/// A frame's patches: `patch_of` gives a pixel's patch, `covered` whether the thing drew it.
pub fn patches(rgb: &[u8], n: usize, patch_of: impl Fn(u32, u32) -> Option<usize>, covered: impl Fn(&[u8]) -> bool) -> Vec<Patch> {
    let mut out = vec![Patch::default(); n];
    for y in 0..H {
        for x in 0..W {
            let Some(k) = patch_of(x, y) else { continue };
            let px = &rgb[((y * W + x) * 3) as usize..][..3];
            let p = &mut out[k];
            p.area += 1;
            p.cut |= x == 0 || y == 0 || x == W - 1 || y == H - 1;
            if covered(px) {
                p.covered += 1;
                let l = 0.299 * px[0] as f64 + 0.587 * px[1] as f64 + 0.114 * px[2] as f64;
                for c in 0..3 {
                    p.sum[c] += px[c] as f64;
                }
                (p.luma, p.luma2) = (p.luma + l, p.luma2 + l * l);
            }
        }
    }
    out
}

/// A renderer drawing into a `W` x `H` frame it reads back, or `None` (with why) where there is no
/// GPU.
pub struct Shooter {
    pub gpu: Renderer,
    target: game_render::gpu::wgpu::Texture,
}

impl Shooter {
    pub fn new() -> Option<Shooter> {
        let mut gpu = match Renderer::headless() {
            Ok(r) => r,
            Err(why) => {
                println!("SKIPPED: {why}");
                return None;
            }
        };
        let target = gpu.offscreen(W, H);
        gpu.set_graphics(game_render::world::Graphics::Full);
        gpu.set_sky(None);
        Some(Shooter { gpu, target })
    }

    pub fn shoot(&mut self, assets: &Assets, light: &Lighting, camera: &Camera) -> Vec<u8> {
        let view = self.target.create_view(&Default::default());
        let views = [View { rect: [0, 0, W, H], camera, actors: &[] }];
        let target = Target { view: &view, size: (W, H), frame: [0, 0, W, H], scale: 1 };
        self.gpu.render(&target, assets, &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light });
        self.gpu.read(&self.target)
    }
}

/// The backdrop a card is shot against: a magenta no texture of leaves comes near.
pub const BACKDROP: [u8; 3] = [255, 0, 255];

/// A scene of nothing but fog far out of the way, its colour [`BACKDROP`].
pub fn bare_scene() -> Scene {
    Scene::parse("[Fog]\nColorRGB=255,0,255\nMinimumFogDistance=50000.00\nMaximumFogDistance=90000.00\n")
}

/// A square card `side` feet across, its texture `texture` of `assets` drawn by `surface`, unlit:
/// the name it is kept under, its own a texture (the GPU keeps a model's materials by its name).
pub fn card(assets: &mut Assets, texture: usize, surface: Surface, side: f32) -> String {
    let verts = [(0.0f32, 0.0f32), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
        .iter()
        .map(|&(u, v)| Vertex {
            pos: v3((u - 0.5) * side, (0.5 - v) * side, 0.0),
            normal: v3(0.0, 0.0, -1.0),
            uv: [u, v],
            color: [128, 128, 128, 128],
            node: 0,
            adc: false,
        })
        .collect();
    let model = Model {
        nodes: vec![Node {
            name: "root".into(),
            local: M4::IDENTITY,
            parent: None,
            center: v3(0.0, 0.0, 0.0),
            half_extents: v3(side / 2.0, side / 2.0, 0.1),
            radius: side,
        }],
        materials: vec!["card".into()],
        meshes: vec![Mesh { material: 0, group: 0, strips: vec![Strip { verts }] }],
        groups: 1,
        group_distance: vec![f32::MAX],
    };
    let tris = model.triangles(0);
    let surface = Surface { shading: Shading::Prelit, ..surface };
    let name = format!("card {texture}");
    assets.models.insert(name.clone(), Loaded { model, tris, textures: vec![Some(texture)], surfaces: vec![surface], radius: side });
    name
}

/// Where the card stands, and the way it faces (away from the camera).
const CARD_AT: V3 = V3 { x: 10.0, y: 5.0, z: 20.0 };
fn card_look() -> V3 {
    v3(0.6, -0.1, 0.8).norm()
}

/// The walks' lens: 0.9 radians top to bottom, the ride camera's.
pub const FOV: f32 = 0.9;

/// How far off a card `side` feet across is `pixels` across in the frame.
pub fn card_distance(side: f32, pixels: f32) -> f32 {
    side * (H as f32 / 2.0 / (FOV / 2.0).tan()) / pixels
}

/// Walk up to a card `side` feet across of `assets`' texture `texture`, drawn by `surface`, from
/// where it is `far` pixels across to `near`, [`STEP`] a frame, cut into `grid` x `grid` patches of
/// its texture: each frame's distance and patches.
pub fn walk_card(shooter: &mut Shooter, assets: &mut Assets, texture: usize, surface: Surface, side: f32, (far, near): (f32, f32), grid: usize) -> Vec<(f32, Vec<Patch>)> {
    let (far, near) = (card_distance(side, far), card_distance(side, near));
    let name = card(assets, texture, surface, side);
    let mut scene = bare_scene();
    scene.models = vec![Placement { model: name, pos: CARD_AT, look: card_look(), up: v3(0.0, 1.0, 0.0) }];
    shooter.gpu.set_scene(&scene);
    let light = Lighting::of(&scene);
    let look = card_look();
    // The card's own right and down, as its texture's u and v run.
    let m = M4::placement(CARD_AT, look, v3(0.0, 1.0, 0.0));
    let (u, v) = (m.dir(v3(1.0, 0.0, 0.0)).norm(), m.dir(v3(0.0, -1.0, 0.0)).norm());
    let corner = CARD_AT.sub(u.scale(side / 2.0)).sub(v.scale(side / 2.0));
    let mut out = Vec::new();
    let mut d = far;
    while d >= near {
        let camera = Camera { eye: CARD_AT.sub(look.scale(d)), at: CARD_AT, fov: FOV, near: 0.25, far: 20000.0 };
        let rgb = shooter.shoot(assets, &light, &camera);
        let inv = unprojection(&camera);
        let patch_of = |x: u32, y: u32| {
            let (a, b) = on_plane(ray(&inv, x, y), corner, u, v)?;
            let (a, b) = (a / side, b / side);
            ((0.0..1.0).contains(&a) && (0.0..1.0).contains(&b)).then(|| (b * grid as f32) as usize * grid + (a * grid as f32) as usize)
        };
        let covered = |p: &[u8]| p.iter().zip(BACKDROP).map(|(&a, b)| (a as i32 - b as i32).abs()).sum::<i32>() > 30;
        out.push((d, patches(&rgb, grid * grid, patch_of, covered)));
        d *= STEP;
    }
    out
}

/// A leafy cut-out, made up: clumps of leaves with holes between them, each leaf's texels varied in
/// brightness texel by texel, as a bush's atlas is. Seeded, so every run is the same picture.
pub fn leaves(size: usize) -> Texture {
    let mut seed = 0x9e37_79b9u32;
    let mut rand = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let mut rgba = vec![0u8; size * size * 4];
    // Holes are the dark green an artist leaves in them.
    for px in rgba.chunks_exact_mut(4) {
        px.copy_from_slice(&[30, 50, 20, 0]);
    }
    let blobs = size * size / 40;
    for _ in 0..blobs {
        let (cx, cy) = ((rand() % size as u32) as i32, (rand() % size as u32) as i32);
        let r = 1 + (rand() % 3) as i32;
        let tone = 60 + (rand() % 120) as i32;
        for y in -r..=r {
            for x in -r..=r {
                if x * x + y * y > r * r {
                    continue;
                }
                let (px, py) = ((cx + x).rem_euclid(size as i32) as usize, (cy + y).rem_euclid(size as i32) as usize);
                let jitter = (rand() % 50) as i32 - 25;
                let g = (tone + jitter).clamp(0, 255);
                rgba[(py * size + px) * 4..][..4].copy_from_slice(&[(g / 3) as u8, g as u8, (g / 4) as u8, 255]);
            }
        }
    }
    Texture { width: size, height: size, rgba }
}

/// How a bush's leaves are drawn: the original's high test (120 of 128, as free06's bushes have
/// it), clamped, its coverage kept by its levels (V58).
pub fn leaf_surface() -> Surface {
    Surface { cutoff: 240, depth_cutoff: 0, clamp: [true; 2], keep_coverage: true, ..Surface::default() }
}
