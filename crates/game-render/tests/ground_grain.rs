//! V101: the ground's finest grain. The original reads its layers into a 512x832 paint target and
//! the ground reads that back, so on top of the layer's own mip level (V97) the target's texel
//! softens the paint a little; and ground seen edge-on is not blurred along its depth past that
//! floor. Before, V97's floor scaled both of a pixel's screen gradients by one factor, which on ground
//! seen at a grazing angle stretched the long one, along the depth, many times past the floor.

use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex, Renderer, Scene3, Target, View};
use game_render::pst::Texture;
use game_render::scene::Scene;
use game_render::splat::{PAINT_NEAREST, Splat, paint_depth, paint_texel};
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

/// Black on the left half, white on the right: edges at x 32 and 64 feet, and every 32 on.
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
fn the_paint_target_adds_its_texel_to_the_layers_level() {
    let splat = edge();
    let mip = splat.passes[0].mip.as_ref().expect("the pass names its mips");
    // A target texel on the nearest ground is about a third of a foot: 512 columns over a 109°
    // view, 832 rows over its height tipped 47° to the ground.
    let near = paint_texel(PAINT_NEAREST);
    println!("target texel {near} ft at {PAINT_NEAREST} ft, {} ft at 400", paint_texel(400.0));
    assert!((0.2..0.5).contains(&near), "{near}");
    for depth in [PAINT_NEAREST, 100.0, 400.0] {
        let (layer, both) = (mip.level(depth), mip.level_through_target(depth, 1.0));
        assert!(both >= layer && both <= mip.levels as f32, "depth {depth}: {layer} -> {both}");
    }
    // A layer a foot a texel at the nearest: its own level (1.7, three texels) outweighs the target's.
    let lift = mip.level_through_target(PAINT_NEAREST, 1.0) - mip.level(PAINT_NEAREST);
    assert!(lift < 0.1, "{lift}");
    // A layer of a tenth of a foot a texel spans three target texels, which take it to the last level.
    assert_eq!(mip.level_through_target(PAINT_NEAREST, 0.1), mip.levels as f32);
}

#[test]
fn ground_seen_edge_on_is_not_blurred_along_its_depth_past_the_floor() {
    let splat = edge();
    let mip = splat.passes[0].mip.as_ref().expect("the pass names its mips");
    let mut gpu = match Renderer::headless() {
        Ok(g) => g,
        Err(e) => return println!("SKIPPED: {e}"),
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
    // A rider's eye 6 feet up, looking along x at the edge at 96 feet, 40 feet off: the ground meets
    // the view at 8.5°, so a pixel spans half a foot along the depth and a fourteenth across it.
    let camera = Camera { eye: v3(56.0, 6.0, 128.0), at: v3(96.0, 0.0, 128.0), fov: 0.9, near: 1.0, far: 2000.0 };
    let depth = paint_depth(camera.eye, camera.at.sub(camera.eye).norm(), v3(96.0, 0.0, 128.0));
    let floor = mip.level(depth).exp2();
    println!("paint depth {depth} ft, level {}, floor {floor} ft", mip.level(depth));
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
    let grey: Vec<f32> = (0..=320)
        .map(|i| {
            let c = vp.clip(v3(80.0 + i as f32 * 0.1, 0.0, 128.0));
            let (sx, sy) = ((c[0] / c[3] + 1.0) / 2.0 * w as f32, (1.0 - c[1] / c[3]) / 2.0 * h as f32);
            assert!(sx >= 0.0 && sy >= 0.0 && sx < w as f32 && sy < h as f32, "({sx}, {sy}) off the frame");
            rgb[(sy as usize * w as usize + sx as usize) * 3] as f32
        })
        .collect();
    let along = fade(&grey, 0.1);
    println!("the edge fades over {along} ft along the depth");
    // Still the floor's softness, not a pixel's half foot ...
    assert!(along >= 0.5 * floor, "{along} ft: the floor is {floor} ft");
    // ... and not V97's one factor on both gradients, which stretched it to some 20 feet here.
    assert!(along <= 2.0 * floor, "{along} ft along the depth, past twice the {floor}-ft floor");
}
