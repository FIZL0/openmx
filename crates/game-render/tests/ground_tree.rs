//! V65: the GPU's ground costs what is in view, not what it holds.
//!
//! The renderer picks what to draw of each ground piece by walking its quadtree
//! (`game_render::ground_tree`), and says in [`Drawn::ground_visits`] how many of its nodes it looked
//! at. A hilly ground and one four times as wide, seen from the same place under Far Fog, look at
//! about as many; under Whole Map the wide one's far branches go whole, so it looks at far fewer nodes
//! than it has chunks. That no two levels drawn crack apart is the tree's own test
//! (`ground_tree::tests::no_crack_between_the_levels_drawn`, on V32's stitching).

use game_render::gpu::{Drawn, GroundChunk, GroundPaint, GroundVertex, Renderer, Scene3, Target, View};
use game_render::scene::Scene;
use game_render::v3;
use game_render::world::{Assets, Camera, Graphics, Lighting};

const CELLS: u32 = 4;
const SIZE: f32 = 9.0;

/// A ground `n` chunks a side of rolling hills.
fn hills(n: u32) -> Vec<GroundChunk> {
    let side = CELLS as f32 * SIZE;
    let height = |x: f32, z: f32| 30.0 * (x / 500.0).sin() * (z / 700.0).cos() + 2.0 * (x / 41.0).sin() * (z / 29.0).sin();
    let mut out = Vec::new();
    for cz in 0..n {
        for cx in 0..n {
            let (x0, z0) = (cx as f32 * side, cz as f32 * side);
            let vertices: Vec<GroundVertex> = (0..=CELLS)
                .flat_map(|j| (0..=CELLS).map(move |i| (i, j)))
                .map(|(i, j)| {
                    let (x, z) = (x0 + i as f32 * SIZE, z0 + j as f32 * SIZE);
                    GroundVertex { pos: [x, height(x, z), z], normal: [0.0, 1.0, 0.0] }
                })
                .collect();
            let (lo, hi) = vertices.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v.pos[1]), hi.max(v.pos[1])));
            out.push(GroundChunk {
                vertices,
                cells: CELLS,
                at: (cx, cz),
                missing: Vec::new(),
                centre: v3(x0 + side / 2.0, (lo + hi) / 2.0, z0 + side / 2.0),
                radius: (side * side / 2.0 + (hi - lo) * (hi - lo) / 4.0).sqrt(),
            });
        }
    }
    out
}

/// One frame of `chunks` from a camera by the ground's first corner looking across it.
fn frame(gpu: &mut Renderer, chunks: &[GroundChunk], graphics: Graphics) -> Drawn {
    let scene = Scene::parse("[Fog]\nColorRGB=90,100,120\nMinimumFogDistance=100.00\nMaximumFogDistance=200.00\n");
    let light = Lighting::of(&scene);
    gpu.set_scene(&scene);
    gpu.set_sky(None);
    gpu.set_graphics(graphics);
    gpu.set_ground_painted(chunks, GroundPaint::Bare, [150.0, 118.0, 86.0]).unwrap();
    let camera = Camera { eye: v3(300.0, 60.0, 280.0), at: v3(900.0, 20.0, 900.0), fov: 0.9, near: 1.0, far: 100_000.0 };
    let (w, h) = (320u32, 180u32);
    let texture = gpu.offscreen(w, h);
    let view = texture.create_view(&Default::default());
    let views = [View { rect: [0, 0, w, h], camera: &camera, actors: &[] }];
    gpu.render(
        &Target { view: &view, size: (w, h), frame: [0, 0, w, h], scale: 1 },
        &Assets::default(),
        &Scene3 { views: &views, poses: &[], overlay: &[], sprites: &[], light: &light },
    )
}

#[test]
fn the_nodes_looked_at_grow_with_the_view_not_the_ground() {
    let mut gpu = match Renderer::headless() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let (small, wide) = (hills(48), hills(192));
    let far_small = frame(&mut gpu, &small, Graphics::Far);
    let far_wide = frame(&mut gpu, &wide, Graphics::Far);
    let drawn = |d: &Drawn| d.ground_levels.iter().sum::<usize>();
    assert!(drawn(&far_small) > 0 && drawn(&far_wide) > 0, "{far_small:?} {far_wide:?}");
    // Far Fog's 800 feet hold the same ground on both: the wide one's extra fifteen sixteenths are
    // culled at a handful of branches' roots.
    assert_eq!(drawn(&far_small), drawn(&far_wide), "the same chunks in reach");
    assert!(
        far_wide.ground_visits <= far_small.ground_visits + 32,
        "Far Fog: {} nodes looked at on the small ground, {} on the wide",
        far_small.ground_visits,
        far_wide.ground_visits
    );
    // Farther Fog (V95), twice Far's reach: 1600 feet, past the small ground's edge but inside one
    // twice as wide, so that one and the wide one hold the same ground, and more of it than Far's.
    let mid = hills(96);
    let farther_mid = frame(&mut gpu, &mid, Graphics::Farther);
    let farther_wide = frame(&mut gpu, &wide, Graphics::Farther);
    assert_eq!(drawn(&farther_mid), drawn(&farther_wide), "the same chunks in Farther Fog's reach");
    assert!(drawn(&farther_wide) > drawn(&far_wide), "Farther draws past Far: {} against {}", drawn(&farther_wide), drawn(&far_wide));
    assert!(
        farther_wide.ground_visits <= farther_mid.ground_visits + 32,
        "Farther Fog: {} nodes looked at on the middle ground, {} on the wide",
        farther_mid.ground_visits,
        farther_wide.ground_visits
    );
    println!(
        "Farther Fog: {} chunks for {} nodes on {} chunks, {} nodes on {}",
        drawn(&farther_mid),
        farther_mid.ground_visits,
        mid.len(),
        farther_wide.ground_visits,
        wide.len()
    );
    // Whole Map draws all of what is in front of the camera, a branch at a time where it is far.
    let whole = frame(&mut gpu, &wide, Graphics::Full);
    println!(
        "Far Fog: {} chunks for {} nodes on {} chunks, {} nodes on {}; Whole Map: {} chunks for {} nodes",
        drawn(&far_small),
        far_small.ground_visits,
        small.len(),
        far_wide.ground_visits,
        wide.len(),
        drawn(&whole),
        whole.ground_visits
    );
    assert!(drawn(&whole) > 4 * drawn(&far_wide), "Whole Map draws past the fog: {} chunks against {}", drawn(&whole), drawn(&far_wide));
    assert!(
        whole.ground_visits * 8 < drawn(&whole),
        "Whole Map: {} nodes looked at for {} chunks drawn",
        whole.ground_visits,
        drawn(&whole)
    );
}
