//! V106: riding over a freeride's ruts, the ground's rut paint shows no band where its sharp near
//! target gives way to the coarse whole-map one, and nothing jumps when the near target follows the
//! rider to another square.
//!
//! The ground is a generated one: a flat grey layer under one big screen-space mesh as wide as a
//! freeride field (6912 feet, so the whole-map target is as coarse as free06's), its texture a fine
//! noise like the disc's tyre tracks. A camera at a rider's height rides straight along it a foot a
//! frame, and each frame is cut into patches fixed on the ground; a patch's mean colour and detail
//! (the spread of its brightness) should change by little from one frame to the next wherever it is.
//! The same rides over free06's own paint and ruts need the user's disc.
//!
//! `V106_TRACE=1` prints each frame's worst step; `V106_SHOT=<dir>` writes every 40th frame there as a
//! `.ppm`, or the frames `V106_FRAMES=a,b,…` names.

mod lod_walk;

use game_render::disc::{Disc, Res};
use game_render::ground_mesh::{Flat, GroundMeshes};
use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex};
use game_render::pst::Texture;
use game_render::splat::{Pass, Splat};
use game_render::v3;
use game_render::world::{Assets, Camera, Lighting};
use lod_walk::{H, Shooter, W, on_plane, ray, unprojection};

/// The field's side, as free06's and free09's.
const FIELD: f32 = 6912.0;
/// The ruts' texture repeats every this many feet, and has this many texels a side: 8 a foot,
/// twice the near target's 4.
const REPEAT: f32 = 32.0;
const NOISE: usize = 256;
/// Patches lie across this many feet either side of the ride and this far ahead of where it starts.
const ACROSS: f32 = 480.0;
const AHEAD: (f32, f32) = (40.0, 1400.0);
/// A patch is judged only while it covers this many pixels.
const MIN_PIXELS: u32 = 400;

/// The most a judged ground patch's mean colour and detail may change between frames a foot apart,
/// over the generated ruts. Before V106 the near target's edge stepped a patch's detail by 3.3 when
/// it jumped, and by 7.4 seen from high up (colour 1.0); with the fades, 0.5 at worst (colour 0.2).
pub const BAND: Bounds = Bounds { colour: 2.0, detail: 1.0 };
/// The same over free06's own paint, whose hard edges shimmer under the GPU's filtering as the
/// camera moves, before V106 and since alike, with no target changing: 2.7 in colour and 1.9 in
/// detail 340 feet ahead of the low eye, 2.5 in detail 1008 feet ahead of the high one. Before V106
/// the near target's jump stepped a patch's detail by 4.4 under the low eye.
pub const BAND_FREE06: Bounds = Bounds { colour: 3.5, detail: 3.0 };

pub struct Bounds {
    pub colour: f32,
    pub detail: f32,
}

/// Noise in clumps of a few texels, as a tyre track's tread is: each texel the mean of a 3x3 of
/// random ones, so it is fine, but not finer than a texel.
fn tread() -> Texture {
    let mut seed = 0x9e37_79b9u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    let raw: Vec<u8> = (0..NOISE * NOISE).map(|_| (next() >> 24) as u8).collect();
    let at = |x: usize, y: usize| raw[(y % NOISE) * NOISE + x % NOISE] as u32;
    let rgba = (0..NOISE * NOISE)
        .flat_map(|i| {
            let (x, y) = (i % NOISE + NOISE, i / NOISE + NOISE);
            let sum: u32 = (0..9).map(|k| at(x + k % 3 - 1, y + k / 3 - 1)).sum();
            let g = (40 + (sum / 9) * 3 / 4) as u8;
            [g, (g as u32 * 7 / 8) as u8, (g as u32 * 3 / 4) as u8, 255]
        })
        .collect();
    Texture { width: NOISE, height: NOISE, rgba }
}

/// The ground paint: one flat grey layer, and over all of the field the tread, laid on by the
/// modulate blend every disc mesh has.
fn rutted() -> Splat {
    let psm = "[General]\nTotalShaderPasses = 1\n[ShaderPass_1]\nTextureMap_1 = Grey\nalphaBlendingOnOff = 0\n\
        [ShaderPass_1_Animator_1]\nAlgorithm = UVAnimation\nUScale = 0.015625\nVScale = 0.015625\nAngle = 0\n";
    let mut splat = Splat::parse(psm, |_| Some(Texture { width: 4, height: 4, rgba: vec![128; 64] }));
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
    let r = FIELD / REPEAT;
    let corner = [[0.0, 0.0], [FIELD, 0.0], [FIELD, FIELD], [0.0, FIELD]];
    let uv = [[0.0, 0.0], [r, 0.0], [r, r], [0.0, r]];
    let tri = |a: usize, b: usize, c: usize| Flat::new([corner[a], corner[b], corner[c]], [uv[a], uv[b], uv[c]], [[128.0; 4]; 3], pass.clone());
    splat.meshes = GroundMeshes::of(vec![tread()], vec![tri(0, 1, 2), tri(0, 2, 3)]);
    assert!(splat.meshes.is_some());
    splat
}

/// A flat chunk of `cells` cells `size` feet each at chunk (`x`, `z`).
fn flat_chunk(cells: u32, size: f32, x: u32, z: u32) -> GroundChunk {
    let side = cells as f32 * size;
    let (x0, z0) = (x as f32 * side, z as f32 * side);
    let vertices = (0..=cells)
        .flat_map(|j| (0..=cells).map(move |i| (i, j)))
        .map(|(i, j)| GroundVertex { pos: [x0 + i as f32 * size, 0.0, z0 + j as f32 * size], normal: [0.0, 1.0, 0.0] })
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

/// What a patch of one frame drew, each pixel weighted by a tent that is 1 in the patch's middle
/// and 0 at its edges, so a row of pixels crossing its edge as the camera moves weighs nothing and
/// a patch's numbers move with the camera smoothly even where the paint has hard edges.
#[derive(Clone, Copy, Default)]
struct Patch {
    area: u32,
    cut: bool,
    weight: f64,
    sum: [f64; 3],
    luma: f64,
    luma2: f64,
}

impl Patch {
    fn mean(&self) -> [f32; 3] {
        self.sum.map(|s| (s / self.weight.max(1e-9)) as f32)
    }
    /// The weighted spread of its brightness: how sharp it looks.
    fn detail(&self) -> f32 {
        let m = self.luma / self.weight.max(1e-9);
        (self.luma2 / self.weight.max(1e-9) - m * m).max(0.0).sqrt() as f32
    }
}

/// The worst change of any judged patch between consecutive frames: (step, its frame, the patch's
/// distance ahead then), for colour and for detail, the eye a foot farther on each frame.
fn worst(frames: &[Vec<Patch>], patch: (f32, f32), rows: usize) -> ((f32, usize, f32), (f32, usize, f32)) {
    let (mut colour, mut detail) = ((0.0f32, 0, 0.0f32), (0.0f32, 0, 0.0f32));
    for (f, pair) in frames.windows(2).enumerate() {
        for (k, (a, b)) in pair[0].iter().zip(&pair[1]).enumerate() {
            if a.area < MIN_PIXELS || b.area < MIN_PIXELS || a.cut || b.cut {
                continue;
            }
            let ahead = AHEAD.0 + (k % rows) as f32 * patch.1 + patch.1 / 2.0 - (f as f32 + 1.0);
            let (ma, mb) = (a.mean(), b.mean());
            let c = (0..3).map(|i| (ma[i] - mb[i]).abs()).fold(0.0, f32::max);
            let d = (a.detail() - b.detail()).abs();
            if c > colour.0 {
                colour = (c, f + 1, ahead);
            }
            if d > detail.0 {
                detail = (d, f + 1, ahead);
            }
        }
    }
    (colour, detail)
}

/// Ride the rutted ground a foot a frame along +z, the eye `high` feet up looking at the ground
/// `looking` feet ahead, judged in patches `patch` feet across and along; the worst steps.
/// From (`x`, `start`), 160 feet: with `start` 208 feet past a multiple of 256 the near target
/// follows it across two squares and the mid one across one (both at frame 48), and their fades
/// pass over every patch's distance.
fn ride(shooter: &mut Shooter, light: &Lighting, (x, start): (f32, f32), high: f32, looking: f32, patch: (f32, f32)) -> ((f32, usize, f32), (f32, usize, f32)) {
    let length = 160usize;
    let rows = ((AHEAD.1 - AHEAD.0) / patch.1) as usize;
    let cols = (2.0 * ACROSS / patch.0) as usize;
    let mut frames = Vec::new();
    for f in 0..=length {
        let z = start + f as f32;
        let camera = Camera { eye: v3(x, high, z), at: v3(x, 0.0, z + looking), fov: 0.9, near: 1.0, far: 4000.0 };
        let rgb = shooter.shoot(&Assets::default(), light, &camera);
        let inv = unprojection(&camera);
        let mut patches = vec![Patch::default(); cols * rows];
        for py in 0..H {
            for px in 0..W {
                let Some((u, v)) = on_plane(ray(&inv, px, py), v3(x - ACROSS, 0.0, start + AHEAD.0), v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0)) else { continue };
                let (u, v) = (u / patch.0, v / patch.1);
                let (c, r) = (u.floor(), v.floor());
                if c < 0.0 || r < 0.0 || c as usize >= cols || r as usize >= rows {
                    continue;
                }
                let tent = |t: f32| 1.0 - (2.0 * t - 1.0).abs();
                let w = (tent(u - c) * tent(v - r)) as f64;
                let rgb = &rgb[((py * W + px) * 3) as usize..][..3];
                let l = 0.299 * rgb[0] as f64 + 0.587 * rgb[1] as f64 + 0.114 * rgb[2] as f64;
                let p = &mut patches[c as usize * rows + r as usize];
                p.area += 1;
                p.cut |= px == 0 || py == 0 || px == W - 1 || py == H - 1;
                p.weight += w;
                for k in 0..3 {
                    p.sum[k] += w * rgb[k] as f64;
                }
                (p.luma, p.luma2) = (p.luma + w * l, p.luma2 + w * l * l);
            }
        }
        frames.push(patches);
        let wanted = std::env::var("V106_FRAMES").map_or(f % 40 == 0, |v| v.split(',').any(|w| w == f.to_string()));
        if let Some(dir) = std::env::var_os("V106_SHOT").filter(|_| wanted) {
            let path = std::path::PathBuf::from(dir).join(format!("v106-{high}-{f:03}.ppm"));
            let mut out = format!("P6 {W} {H} 255\n").into_bytes();
            out.extend_from_slice(&rgb);
            std::fs::write(&path, out).unwrap();
        }
    }
    if std::env::var_os("V106_TRACE").is_some() {
        for f in 1..frames.len() {
            let (c, d) = worst(&frames[f - 1..=f], patch, rows);
            println!("trace {high} {f:3} colour {:5.2} at {:4.0} ft, detail {:5.2} at {:4.0} ft", c.0, c.2 - f as f32 + 1.0, d.0, d.2 - f as f32 + 1.0);
        }
    }
    worst(&frames, patch, rows)
}

/// Flat 512-foot chunks under a ride from (`x`, `start`) and everything it looks at.
fn ground_under(x: f32, start: f32) -> Vec<GroundChunk> {
    let (cx, cz) = ((x / 512.0) as u32, (start / 512.0) as u32);
    (cz.saturating_sub(1)..=cz + 4).flat_map(|z| (cx.saturating_sub(2)..=cx + 2).map(move |x| flat_chunk(32, 16.0, x, z))).collect()
}

/// Both rides over `splat` from (`x`, `start`), asserting the bounds.
fn ride_both(shooter: &mut Shooter, splat: &Splat, (x, start): (f32, f32), name: &str, bounds: Bounds) {
    let look = game_render::scene::Scene::parse("[Fog]\nColorRGB=0,0,0\nMinimumFogDistance=50000.00\nMaximumFogDistance=90000.00\n");
    let light = Lighting::of(&look);
    shooter.gpu.set_scene(&look);
    // The shell's Texture Filtering at its most, which keeps the near ruts sharp far down the ride.
    shooter.gpu.set_anisotropy(16);
    shooter.gpu.set_ground_painted(&ground_under(x, start), GroundPaint::Splat(splat), [150.0, 118.0, 86.0]).unwrap();
    // A chase camera's height, where the near target's edge is (V106's screenshot), and one high
    // over the ground where the mid target's is.
    let mut failed = Vec::new();
    for (high, looking, patch) in [(14.0, 50.0, (24.0, 48.0)), (90.0, 400.0, (64.0, 96.0))] {
        let (colour, detail) = ride(shooter, &light, (x, start), high, looking, patch);
        println!(
            "{name}, eye {high} ft: worst colour step {:.2} (frame {}, {:.0} ft ahead), detail step {:.2} (frame {}, {:.0} ft ahead)",
            colour.0, colour.1, colour.2, detail.0, detail.1, detail.2
        );
        if colour.0 > bounds.colour {
            failed.push(format!("{name}, eye {high} ft: a ground patch's colour stepped {:.2} at frame {}, {:.0} ft ahead", colour.0, colour.1, colour.2));
        }
        if detail.0 > bounds.detail {
            failed.push(format!("{name}, eye {high} ft: a ground patch's detail stepped {:.2} at frame {}, {:.0} ft ahead", detail.0, detail.1, detail.2));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("; "));
}

#[test]
fn riding_over_ruts_the_ground_shows_no_band_or_jump() {
    let Some(mut shooter) = Shooter::new() else { return };
    ride_both(&mut shooter, &rutted(), (3000.0, 2000.0), "generated ruts", BAND);
}

/// A track's two archives, as the shell opens them: `name.res` and `name_2.res`.
fn archives(disc: &mut Disc, name: &str) -> Vec<Res> {
    [format!("\\{name}.res"), format!("\\{name}_2.res")]
        .iter()
        .filter_map(|want| {
            let member = disc.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with(want.as_str()))?.name.clone();
            Some(disc.res(&member).expect("the track's archive"))
        })
        .collect()
}

/// The same rides over free06's own paint and its two hundred ruts, from where the most of its
/// meshes lie ahead. Needs the user's disc, and skips without it.
#[test]
fn riding_over_free06s_ruts_the_ground_shows_no_band_or_jump() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            println!("SKIPPED: {why}");
            return;
        }
    };
    let Some(mut shooter) = Shooter::new() else { return };
    let archives = archives(&mut disc, "free06");
    let splat = Splat::load(&archives, "free06", (FIELD, FIELD)).expect("free06's paint");
    let meshes = splat.meshes.as_ref().expect("free06's ruts");
    // How much the meshes cover, sampled every 32 feet, in the strip a ride looks over.
    let laid = |x: f32, z: f32| 1.0 - meshes.affine(x, z).1;
    let strip = |x: f32, start: f32| -> f32 {
        (0..16).flat_map(|i| (0..20).map(move |j| (x - 240.0 + i as f32 * 32.0, start + 40.0 + j as f32 * 32.0))).map(|(x, z)| laid(x, z)).sum()
    };
    let (x, start) = (4..18)
        .flat_map(|i| (2..20).map(move |j| (i as f32 * 256.0 + 128.0, j as f32 * 256.0 + 208.0)))
        .filter(|&(_, z)| z + 1200.0 < FIELD)
        .max_by(|a, b| strip(a.0, a.1).total_cmp(&strip(b.0, b.1)))
        .unwrap();
    println!("free06: riding from {x}, {start}: {:.0} of 320 samples under a mesh", strip(x, start));
    ride_both(&mut shooter, &splat, (x, start), "free06", BAND_FREE06);
}
