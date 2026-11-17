//! P346: the GPU's own time for a frame (V122) comes back for each way a frame is drawn — one pass,
//! the views at a low render scale and then the frame, and over a picture already there — now that
//! its timestamps ride on the frame's own passes. Before, they were written from empty passes, which
//! Metal does not time, and every iPhone log read "gpu no frame timed".

use game_render::gpu::{Renderer, Scene3, Target, View};
use game_render::mat::v3;
use game_render::scene::Scene;
use game_render::world::{Assets, Camera, Lighting};

#[test]
fn every_way_of_drawing_a_frame_is_timed() {
    let mut gpu = match Renderer::headless() {
        Ok(g) => g,
        Err(why) => {
            println!("SKIPPED: {why}");
            return;
        }
    };
    if !gpu.timed() {
        println!("SKIPPED: this adapter has no timestamps");
        return;
    }
    let assets = Assets::default();
    let scene = Scene::parse("[Fog]\nColorRGB=61,68,76\nMinimumFogDistance=5000.00\nMaximumFogDistance=9000.00\n");
    let light = Lighting::of(&scene);
    gpu.set_scene(&scene);
    gpu.set_sky(None);
    let (w, h) = (160u32, 120u32);
    let texture = gpu.offscreen(w, h);
    let view = texture.create_view(&Default::default());
    let camera = Camera { eye: v3(0.0, 0.0, 0.0), at: v3(0.0, 0.0, 10.0), fov: 0.9, near: 0.5, far: 2000.0 };

    for (way, scale, over) in [("one pass", 1.0, false), ("a low render scale", 0.5, false), ("over a picture", 1.0, true)] {
        gpu.render_scale = scale;
        gpu.gpu_times();
        let mut times = Vec::new();
        for _ in 0..8 {
            let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
            let target = Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 };
            let frame = Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light };
            match over {
                true => gpu.render_over(&target, &assets, &frame),
                false => gpu.render(&target, &assets, &frame),
            };
            // Reading the picture back waits for the GPU, so the frame's times are in by the next look.
            gpu.read(&texture);
            times.extend(gpu.gpu_times());
        }
        println!("{way}: {} frames timed, {:?}", times.len(), times.first());
        assert!(!times.is_empty(), "no frame drawn {way} came back timed");
        assert!(times.iter().all(|(t, _)| t.as_nanos() > 0), "a frame drawn {way} timed as nothing");
    }
}
