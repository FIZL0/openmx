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

/// **The last sample row and column are ground** (S320). `samples_x` × `samples_z` counts cells: each
/// leaf block's 17th row and column carry the far corners, so the strip between the last sample and
/// the field's box is a cell like any other, and the terrain object's own query answers it (sx11's
/// race file has a node point there, at z 765 of 768, that the original stands at 89.97). The world's
/// ground, its put-back test and its surface answer it too, the same to the bit; past the box, nothing.
#[test]
fn the_last_sample_row_and_column_are_ground() {
    let bytes = generate::terrain("last", (16, 16), 3.0, |x, z| 80.0 + f64::from(x % 7) * 0.37 + f64::from(z % 5) * 0.61, |x, z| ((x + z) % 3) as u8);
    let terrain = Terrain::parse(&bytes).unwrap();
    let mut ground = Ground::new(&terrain, &layout((1, 1), 3.0));
    assert_eq!((ground.samples_x, ground.samples_z), (256, 256));
    let alone = Ground::new(&terrain, &layout((1, 1), 3.0));
    let field = alone.field();
    assert_eq!((field.max.x(), field.max.z()), (768.0, 768.0));
    let mut strip = 0;
    for k in 0..2000u32 {
        // Along the far z row, the far x column and their corner, and a little way inside.
        let along = (k as f32 * 0.3837) % 768.0;
        let into = 765.0 + (k % 31) as f32 * 0.0967;
        let (x, z) = match k % 3 {
            0 => (along, into),
            1 => (into, along),
            _ => (into, 765.0 + (k % 29) as f32 * 0.1031),
        };
        let at = Vf::new(x, 200.0, z, 1.0);
        let want = game_sim::contact_search::ground(&field, at);
        let (point, normal) = ground.ground(at);
        assert_eq!(point.to_bits(), want.point.to_bits(), "({x}, {z})");
        assert_eq!(normal.to_bits(), want.normal.to_bits(), "({x}, {z})");
        assert!(point.y() > 79.0, "({x}, {z}): {point:?}");
        assert!(ground.keeps(at), "({x}, {z})");
        assert!(ground.at(F(x), F(z)).is_some(), "({x}, {z})");
        strip += 1;
    }
    assert!(strip > 0);
    // At the box and past it, no ground: the edge test's own rectangle is the same.
    for (x, z) in [(768.0, 100.0), (100.0, 768.0), (768.0, 768.0), (800.0, 300.0)] {
        let at = Vf::new(x, 200.0, z, 1.0);
        assert_eq!(ground.ground(at).0.y(), -1.0e6, "({x}, {z})");
        assert!(!ground.keeps(at));
    }
}

/// **A ride in the last strip stands on it** (S320). A supercross bike's edge test is off
/// ([`game_sim::off_map::armed`]), so nothing throws it from the field's last row at a stadium's far
/// side. A bike dropped into the strip and thrown along it rides the field's flat ground there as it
/// rides a flat floor at the same height, every tick; before S320 its wheels found no ground under
/// them and it fell through. (Not to the bit: the field's triangle arithmetic and the floor's plane
/// round differently, an ulp from tick 26.)
#[test]
fn a_ride_in_the_last_strip_stands_on_it() {
    use game_sim::phy::Phy;
    use game_sim::ride::{Controls, Ride, Tables};
    use game_sim::world::Flat;

    let bytes = generate::terrain("last", (16, 16), 3.0, |_, _| 10.0, |_, _| 0);
    let terrain = Terrain::parse(&bytes).unwrap();
    let mut field = Ground::new(&terrain, &layout((1, 1), 3.0));
    let mut floor = Flat { y: F(10.0), normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    let tables = Tables::build();
    let p = Phy::read_bytes(include_str!("freeride-bike.phy.txt").as_bytes(), &tables.curve);
    // Across the strip (z 765..768), thrown along it.
    let ride = || {
        let mut r = Ride::new(&p, game_sim::stock::MOUNTS, Vf::new(300.0, 12.0, 766.5, 1.0), Vf::new(1.0, 0.0, 0.0, 0.0));
        r.chassis.vel = Vf::new(25.0, 0.0, 0.0, 0.0);
        r
    };
    let (mut on_field, mut on_floor) = (ride(), ride());
    let dt = F(1.0 / 60.0);
    for k in 0..180 {
        on_field.frame(&p, &tables, &Controls::default(), dt, &mut field);
        on_floor.frame(&p, &tables, &Controls::default(), dt, &mut floor);
        let (a, b) = (on_field.position(), on_floor.position());
        let apart = (0..3).map(|i| (a.0[i] - b.0[i]).abs()).fold(0.0f32, f32::max);
        assert!(apart < 1.0e-3, "tick {k}: on the field's last row {a:?}, on a floor {b:?}");
    }
    let at = on_field.position();
    assert!(at.z() > 765.0 && at.z() < 768.0 && at.x() > 310.0, "the bike left the strip: {at:?}");
    assert!(at.y() > 9.0 && at.y() < 14.0, "the bike is not on the ground: {at:?}");
}
