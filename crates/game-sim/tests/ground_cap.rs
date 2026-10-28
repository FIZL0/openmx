//! MD33: one terrain's field and edge stop at the samples its tree holds.
//!
//! A generated tile ([`game_sim::tiles::generate`]) is a one-level tree of N 16-sample cells, and its
//! scene says `TerrainWidth=N`, counting root cells as a world's file does. Read alone as a track, the
//! scene's `N × 256` put the edge sixteen times past the ground, so the edge test never threw and a bike
//! ridden off the field fell for ever. Where the tree is the scene's size, as on every disc map
//! (`game-render`'s `terrain_tree_extent.rs`), the edge is the original's to the bit.

use game_sim::biplane::Area;
use game_sim::tiles::generate;
use game_sim::track::{Terrain, TerrainLayout};
use game_sim::world::{Ground, World};
use ps2_float::{F, Vf};

fn layout(cells: (u32, u32), spacing: f32) -> TerrainLayout {
    TerrainLayout {
        width: cells.0,
        breadth: cells.1,
        width_scale: F(spacing),
        breadth_scale: F(spacing),
        ..TerrainLayout::default()
    }
}

/// Eight by five cells under `TerrainWidth=8`, `TerrainBreadth=5`: the field is 128 × 80 samples and the
/// edge is there, not at 2048 × 1280.
#[test]
fn a_tile_under_its_world_scene_has_its_edge_at_its_ground() {
    let bytes = generate::terrain("cap", (8, 5), 3.0, |_, _| 10.0, |_, _| 1);
    let terrain = Terrain::parse(&bytes).unwrap();
    assert_eq!(terrain.samples(), Some([128, 80]));
    let ground = Ground::new(&terrain, &layout((8, 5), 3.0));
    assert_eq!((ground.samples_x, ground.samples_z), (128, 80));
    let area = ground.area().unwrap();
    assert_eq!((area.width.0, area.depth.0), (384.0, 240.0));
    // Inside the field, ground; past the last sample, the edge test throws.
    assert!(ground.at(F(300.0), F(200.0)).is_some());
    assert!(area.outside(Vf::new(390.0, 0.0, 100.0, 1.0), F::ZERO));
    assert!(area.outside(Vf::new(100.0, 0.0, 250.0, 1.0), F::ZERO));
    assert!(!area.outside(Vf::new(380.0, 0.0, 230.0, 1.0), F::ZERO));
}

/// A full one-level tree under `TerrainWidth=1` (MX vs. ATV's maps, MD12) and the two-level case's
/// scene-sized trees keep the scene's edge, word for word as the original's arithmetic makes it.
#[test]
fn a_tree_the_scenes_size_keeps_the_scenes_edge() {
    for spacing in [2.0f32, 3.280840, 4.5, 9.0] {
        let bytes = generate::terrain("full", (16, 16), spacing, |_, _| 10.0, |_, _| 1);
        let terrain = Terrain::parse(&bytes).unwrap();
        let l = layout((1, 1), spacing);
        let ground = Ground::new(&terrain, &l);
        assert_eq!((ground.samples_x, ground.samples_z), (256, 256));
        let want = Area::new(F(1.0), F(1.0), l.width_scale, l.breadth_scale);
        let got = ground.area().unwrap();
        assert_eq!((got.width.0.to_bits(), got.depth.0.to_bits()), (want.width.0.to_bits(), want.depth.0.to_bits()));
    }
}
