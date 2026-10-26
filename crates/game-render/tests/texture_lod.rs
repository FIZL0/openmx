//! V85: a sharper level of a texture fades in as the camera nears it, it does not pop in. Walked
//! from far to near on the GPU, a frame 1% nearer than the last, no patch of a leaf card or of the
//! ground changes its coverage, colour or detail by more than a named bound from one frame to the
//! next — through every distance where the texture's level changes. Presentation only.

mod lod_walk;

use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex};
use game_render::v3;
use game_render::world::{Assets, Camera, Lighting};
use lod_walk::*;

/// Of a cut-out's patches, the most one may change from a frame to the next, walked in 1% steps:
/// its share of pixels drawn, its mean colour (0..255 a channel) and its detail (the spread of its
/// brightness, 0..255). The dithered levels' worst frames over the walk: coverage 0.028, colour
/// 1.9, detail 1.8 (at 15.6 ft, where the card is 256 pixels across and its texels fall four to a
/// pixel; reading one level at a time stepped the same there). Reading one level at a time (V58),
/// the detail stepped 6.5, 4.5 and 2.6 where the level was 2.5, 1.5 and 0.5.
pub const CUT_OUT_COVERAGE: f32 = 0.04;
pub const CUT_OUT_COLOUR: f32 = 2.5;
pub const CUT_OUT_DETAIL: f32 = 2.2;

/// The ground's: it is read between two levels already (trilinear), and steps 0.2 at most.
pub const GROUND_COLOUR: f32 = 1.0;
pub const GROUND_DETAIL: f32 = 1.0;

/// Walked up to a 1024-texel leaf card (an upscaled bush's size, V84) from where it is 120 pixels
/// across to where it is 1300 (its level going from past 3 to under 0), no patch of the card steps.
#[test]
fn a_leaf_card_walked_from_far_to_near_sharpens_without_a_step() {
    let Some(mut shooter) = Shooter::new() else { return };
    let mut assets = Assets::default();
    assets.textures.push(leaves(1024));
    let frames = walk_card(&mut shooter, &mut assets, 0, leaf_surface(), 4.0, (120.0, 1300.0), 4);
    judge_cut_out("made-up leaves", &frames);
}

pub fn judge_cut_out(what: &str, frames: &[(f32, Vec<Patch>)]) {
    let s = worst(frames);
    println!(
        "{what}: worst step a frame: coverage {:.3} at {:.1} ft, colour {:.2} at {:.1} ft, detail {:.2} at {:.1} ft",
        s.coverage.0, s.coverage.1, s.colour.0, s.colour.1, s.detail.0, s.detail.1
    );
    assert!(frames.iter().any(|(_, p)| p.iter().any(|p| p.area >= MIN_AREA && !p.cut && p.covered > 0)), "{what}: judged at all");
    assert!(s.coverage.0 <= CUT_OUT_COVERAGE, "{what}: coverage steps {:.3} at {:.1} ft", s.coverage.0, s.coverage.1);
    assert!(s.colour.0 <= CUT_OUT_COLOUR, "{what}: colour steps {:.2} at {:.1} ft", s.colour.0, s.colour.1);
    assert!(s.detail.0 <= CUT_OUT_DETAIL, "{what}: detail steps {:.2} at {:.1} ft", s.detail.0, s.detail.1);
}

/// A flat chunk of `cells` cells `size` feet each at chunk (`x`, `z`).
fn flat_chunk(cells: u32, size: f32, x: u32, z: u32) -> GroundChunk {
    let side = cells as f32 * size;
    let (x0, z0) = (x as f32 * side, z as f32 * side);
    let vertices = (0..=cells)
        .flat_map(|j| (0..=cells).map(move |i| (i, j)))
        .map(|(i, j)| GroundVertex { pos: [x0 + i as f32 * size, 0.0, z0 + j as f32 * size], normal: [0.0, 1.0, 0.0] })
        .collect();
    GroundChunk { vertices, cells, at: (x, z), missing: Vec::new(), centre: v3(x0 + side / 2.0, 0.0, z0 + side / 2.0), radius: side * std::f32::consts::FRAC_1_SQRT_2 }
}

/// Ridden at a spot on painted ground (a mod map's page, four texels a foot, every texel its own
/// brightness) from 150 feet to 8, eye 6 feet up, no patch of the ground about the spot steps.
#[test]
fn a_ground_spot_walked_from_far_to_near_sharpens_without_a_step() {
    let Some(mut shooter) = Shooter::new() else { return };
    let page = |x: u32, y: u32| {
        let h = (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)).wrapping_mul(0x9e37_79b9) >> 26;
        let g = 60 + h as u8 * 2;
        [g, g / 2 + 40, g / 3 + 20]
    };
    let trn = game_sim::tiles::generate::terrain_painted("v85", (16, 16), 1.0, |_, _| 0.0, |_, _| 1, Some(&page));
    let t = game_sim::track::Terrain::parse(&trn).expect("the generated terrain reads");
    let pages = game_render::terrain::Pages::of(&t, 1.0, (16, 16)).expect("it has its page");
    let chunks: Vec<GroundChunk> = (0..8).flat_map(|z| (0..8).map(move |x| flat_chunk(32, 1.0, x, z))).collect();
    let scene = bare_scene();
    shooter.gpu.set_scene(&scene);
    shooter.gpu.set_ground_painted(&chunks, GroundPaint::Pages(&pages), [150.0, 118.0, 86.0]).unwrap();
    let light = Lighting::of(&scene);
    let spot = v3(128.0, 0.0, 128.0);
    let along = v3(0.6, 0.0, 0.8);
    let (side, grid) = (16.0f32, 8usize);
    let corner = spot.sub(v3(side / 2.0, 0.0, side / 2.0));
    let mut frames = Vec::new();
    let mut d = 150.0f32;
    while d >= 8.0 {
        let camera = Camera { eye: spot.sub(along.scale(d)).add(v3(0.0, 6.0, 0.0)), at: spot, fov: 0.9, near: 0.25, far: 20000.0 };
        let rgb = shooter.shoot(&Assets::default(), &light, &camera);
        let inv = unprojection(&camera);
        let patch_of = |x: u32, y: u32| {
            let (a, b) = on_plane(ray(&inv, x, y), corner, v3(1.0, 0.0, 0.0), v3(0.0, 0.0, 1.0))?;
            let (a, b) = (a / side, b / side);
            ((0.0..1.0).contains(&a) && (0.0..1.0).contains(&b)).then(|| (b * grid as f32) as usize * grid + (a * grid as f32) as usize)
        };
        frames.push((d, patches(&rgb, grid * grid, patch_of, |_| true)));
        d *= STEP;
    }
    let s = worst(&frames);
    println!("ground: worst step a frame: colour {:.2} at {:.1} ft, detail {:.2} at {:.1} ft", s.colour.0, s.colour.1, s.detail.0, s.detail.1);
    assert!(frames.iter().any(|(_, p)| p.iter().any(|p| p.area >= MIN_AREA && !p.cut)), "judged at all");
    assert!(s.colour.0 <= GROUND_COLOUR, "ground: colour steps {:.2} at {:.1} ft", s.colour.0, s.colour.1);
    assert!(s.detail.0 <= GROUND_DETAIL, "ground: detail steps {:.2} at {:.1} ft", s.detail.0, s.detail.1);
}
