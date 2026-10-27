//! V97: the ground's layers are as soft as the original's. The original reads each layer at the mip
//! level its own paint camera picks (`MipMap_1` levels, `K` the bias), and that camera stands at
//! least 75 feet over the ground, so a layer's sharp edge fades over several of its texels there,
//! in the software paint and in a GPU frame alike — not over the one a bilinear read gives.

use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex, Renderer, Scene3, Target, View};
use game_render::pst::Texture;
use game_render::scene::Scene;
use game_render::splat::{PAINT_NEAREST, Splat, paint_depth};
use game_render::v3;
use game_render::world::{Assets, Camera, Lighting};

/// One layer, a repeat every 64 feet (a foot a texel), mipped as `free09`'s layers are.
const PSM: &str = "[General]\n\
    TotalShaderPasses = 1\n\
    [ShaderPass_1]\n\
    TextureMap_1 = Edge\n\
    alphaBlendingOnOff = 0\n\
    MipMap_1 = 2\n\
    MagFilter = 1\n\
    MinFilter = 5\n\
    K = -65\n\
    [ShaderPass_1_Animator_1]\n\
    Algorithm = UVAnimation\n\
    UScale = 0.015625\n\
    VScale = 0.015625\n\
    Angle = 0\n";

/// Black on the left half, white on the right: an edge at 32 feet (and one at the wrap).
fn edge() -> Splat {
    Splat::parse(PSM, |_| {
        let rgba = (0..64 * 64).flat_map(|i| if i % 64 < 32 { [0, 0, 0, 255] } else { [255, 255, 255, 255] }).collect();
        Some(Texture { width: 64, height: 64, rgba })
    })
}

/// How many feet `grey` takes to climb from 10% to 90% of the way between its ends, read every
/// `step` feet.
fn fade(grey: &[f32], step: f32) -> f32 {
    let (lo, hi) = (grey[0], grey[grey.len() - 1]);
    let at = |share: f32| grey.iter().position(|&g| (g - lo) / (hi - lo) >= share).unwrap() as f32 * step;
    at(0.9) - at(0.1)
}

#[test]
fn a_layer_edge_fades_over_the_originals_width_not_one_texel() {
    let splat = edge();
    let mip = splat.passes[0].mip.as_ref().expect("the pass names its mips");
    assert_eq!(mip.levels, 2);
    // The nearest ground to the paint camera reads level 1.7 on free09's K: about 3 texels wide.
    let level = mip.level(PAINT_NEAREST);
    assert!((1.6..1.8).contains(&level), "level {level}");
    let across = |depth: f32| -> Vec<f32> { (0..=160).map(|i| splat.paint_at(24.0 + i as f32 * 0.1, 10.0, depth)[0] as f32).collect() };
    let sharp = fade(&across(0.0), 0.1);
    let soft = fade(&across(PAINT_NEAREST), 0.1);
    println!("software: {sharp} ft at full size, {soft} ft at level {level}");
    assert!(sharp <= 1.0, "a full-size read fades over one texel: {sharp}");
    assert!(soft >= 0.8 * 2f32.powf(level), "the original's width is about {} ft: {soft}", 2f32.powf(level));
    // Farther than the nearest it is the last level's, four texels: about 3 feet from 10% to 90%, sampled to the pixel.
    let far = fade(&across(400.0), 0.1);
    assert!(far >= 2.5, "{far}");

    // The GPU: a flat ground under the edge, looked straight down at from 300 feet, where the screen
    // magnifies the layer (half a foot a pixel) so only the paint camera's level can soften it.
    let mut gpu = match Renderer::headless() {
        Ok(g) => g,
        Err(e) => return println!("SKIPPED the GPU half: {e}"),
    };
    let (cells, size) = (16u32, 16.0f32);
    let chunks: Vec<GroundChunk> = (0..2u32)
        .flat_map(|z| (0..2u32).map(move |x| (x, z)))
        .map(|(x, z)| {
            let side = cells as f32 * size;
            let (x0, z0) = (x as f32 * side, z as f32 * side);
            GroundChunk {
                vertices: (0..=cells)
                    .flat_map(|j| (0..=cells).map(move |i| (i, j)))
                    .map(|(i, j)| GroundVertex { pos: [x0 + i as f32 * size, 0.0, z0 + j as f32 * size], normal: [0.0, 1.0, 0.0] })
                    .collect(),
                cells,
                at: (x, z),
                missing: Vec::new(),
                centre: v3(x0 + side / 2.0, 0.0, z0 + side / 2.0),
                radius: side * std::f32::consts::FRAC_1_SQRT_2,
            }
        })
        .collect();
    let look = Scene::parse("[Fog]\nColorRGB=0,0,0\nMinimumFogDistance=50000.00\nMaximumFogDistance=90000.00\n");
    let light = Lighting::of(&look);
    gpu.set_scene(&look);
    gpu.set_sky(None);
    let camera = Camera { eye: v3(96.0, 300.0, 127.0), at: v3(96.0, 0.0, 128.0), fov: 0.9, near: 1.0, far: 2000.0 };
    let depth = paint_depth(camera.eye, camera.at.sub(camera.eye).norm(), v3(96.0, 0.0, 128.0));
    println!("GPU: paint depth {depth} ft, level {}", mip.level(depth));
    gpu.set_ground_painted(&chunks, GroundPaint::Splat(&splat), [150.0, 118.0, 86.0]).unwrap();
    let (w, h) = (512u32, 512u32);
    let texture = gpu.offscreen(w, h);
    let view = texture.create_view(&Default::default());
    let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
    gpu.render(
        &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
        &Assets::default(),
        &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light },
    );
    let rgb = gpu.read(&texture);
    let vp = camera.view_projection(w as usize, h as usize);
    let grey: Vec<f32> = (0..=160)
        .map(|i| {
            let c = vp.clip(v3(88.0 + i as f32 * 0.1, 0.0, 128.0));
            let (sx, sy) = ((c[0] / c[3] + 1.0) / 2.0 * w as f32, (1.0 - c[1] / c[3]) / 2.0 * h as f32);
            rgb[(sy as usize * w as usize + sx as usize) * 3] as f32
        })
        .collect();
    let gpu_fade = fade(&grey, 0.1);
    println!("GPU: {gpu_fade} ft");
    assert!(gpu_fade >= 2.5, "the GPU's edge fades over {gpu_fade} ft, not the last level's four texels");
}
