//! V89: in split screen every view draws the roads and ruts painted into the ground (V67) from a sharp
//! near target round its own eye, not only the first view: two views far apart over a road each draw
//! it as a single view at the same spot does.
//!
//! The roads are made here, a fine checker far finer than the far target's whole-map texels, so a
//! view left on the far target shows a grey blur where the checker should be. Needs a GPU, and skips
//! with the reason when there is none.

use game_render::ground_mesh::{Flat, GroundMeshes};
use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex, Renderer, Scene3, Target, View};
use game_render::pst::Texture;
use game_render::scene::Scene;
use game_render::splat::{Pass, Splat};
use game_render::v3;
use game_render::world::{Assets, Camera, Lighting};

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

fn pass(blend: Option<[u8; 4]>) -> Pass {
    Pass { texture: 0, scale: [0.05, 0.05], angle: 0.0, turn: (0.0, 1.0), rgba: [128.0; 4], blend, fixed: 128.0, reads: (true, true) }
}

/// A road `side` feet square at `x0`, `z0`: texture 0 across it once.
fn road(x0: f32, z0: f32, side: f32) -> Vec<Flat> {
    let c = [128.0; 4];
    let (a, b, cc, d) = ([x0, z0], [x0 + side, z0], [x0 + side, z0 + side], [x0, z0 + side]);
    vec![
        Flat::new([a, b, cc], [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], [c; 3], pass(Some([0, 1, 0, 1]))),
        Flat::new([a, cc, d], [[0.0, 0.0], [1.0, 1.0], [0.0, 1.0]], [c; 3], pass(Some([0, 1, 0, 1]))),
    ]
}

/// The mean absolute difference between neighbouring pixels across a view's middle: high where the
/// checker is sharp, low where it is blurred to grey.
fn sharpness(rgb: &[u8], width: u32, [x, y, w, h]: [u32; 4]) -> f32 {
    let (mut sum, mut n) = (0.0, 0.0);
    for j in y + h / 4..y + 3 * h / 4 {
        for i in x + w / 4..x + 3 * w / 4 {
            let at = ((j * width + i) * 3) as usize;
            sum += (rgb[at] as f32 - rgb[at + 3] as f32).abs();
            n += 1.0;
        }
    }
    sum / n
}

#[test]
fn two_views_far_apart_each_draw_their_road_sharp() {
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            println!("SKIPPED: {e}");
            return;
        }
    };
    // A checker of 2-foot squares on a 128-foot road: 128 texels across, 64 checks. The far target
    // spreads 2048 texels over the 3700 feet from one road to the other, under 0.6 a foot.
    let n = 128;
    let checker = Texture {
        width: n,
        height: n,
        rgba: (0..n * n).flat_map(|k| if ((k % n) / 2 + (k / n) / 2) % 2 == 0 { [250, 250, 250, 255] } else { [10, 10, 10, 255] }).collect(),
    };
    let (side, here, there) = (128.0, [200.0, 200.0], [3800.0, 3800.0]);
    let mut tris = road(here[0], here[1], side);
    tris.extend(road(there[0], there[1], side));
    let dirt = Texture { width: 2, height: 2, rgba: [120, 100, 80, 255].repeat(4) };
    let splat = Splat { textures: vec![dirt], passes: vec![pass(None)], shadow: None, meshes: GroundMeshes::of(vec![checker], tris) };
    let chunks: Vec<GroundChunk> = [(0, 0), (7, 7)].iter().map(|&(x, z)| flat_chunk(32, 16.0, x, z)).collect();
    let look = Scene::parse("[Fog]\nColorRGB=0,0,0\nMinimumFogDistance=50000.00\nMaximumFogDistance=90000.00\n");
    let light = Lighting::of(&look);
    gpu.set_scene(&look);
    gpu.set_sky(None);
    gpu.set_ground_painted(&chunks, GroundPaint::Splat(&splat), [150.0, 118.0, 86.0]).unwrap();
    let over = |p: [f32; 2]| {
        let mid = [p[0] + side / 2.0, p[1] + side / 2.0];
        Camera { eye: v3(mid[0], 150.0, mid[1] - 1.0), at: v3(mid[0], 0.0, mid[1]), fov: 0.9, near: 1.0, far: 2000.0 }
    };
    let (a, b) = (over(here), over(there));
    let (w, h) = (1024u32, 512u32);
    let (left, right) = ([0, 0, w / 2, h], [w / 2, 0, w / 2, h]);
    let texture = gpu.offscreen(w, h);
    let mut frame = |views: &[View]| {
        let view = texture.create_view(&Default::default());
        gpu.render(
            &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
            &Assets::default(),
            &Scene3 { views, poses: &[], overlay: &[], sprites: &[], light: &light },
        );
        gpu.read(&texture)
    };
    let both = frame(&[View { rect: left, camera: &a, actors: &[] }, View { rect: right, camera: &b, actors: &[] }]);
    let only_a = frame(&[View { rect: left, camera: &a, actors: &[] }]);
    let only_b = frame(&[View { rect: right, camera: &b, actors: &[] }]);
    for (name, rect, alone) in [("player 1's", left, &only_a), ("player 2's", right, &only_b)] {
        let (shared, single) = (sharpness(&both, w, rect), sharpness(alone, w, rect));
        let [x, y, rw, rh] = rect;
        let mut worst = 0u8;
        let mut off = 0;
        for j in y..y + rh {
            for i in x..x + rw {
                let at = ((j * w + i) * 3) as usize;
                let d = (0..3).map(|k| both[at + k].abs_diff(alone[at + k])).max().unwrap();
                worst = worst.max(d);
                off += usize::from(d > 2);
            }
        }
        println!("{name} view: sharpness {shared:.1} split, {single:.1} alone; {off} pixels off by more than 2, worst {worst}");
        // A single view sees its checker sharp (about 27 here); left on the far target it is about 7.
        assert!(single > 15.0, "{name} road alone is blurred: {single}");
        assert!(off * 1000 <= (rw * rh) as usize, "{name} road in split screen: {off} pixels differ from it alone (worst {worst})");
    }
}
