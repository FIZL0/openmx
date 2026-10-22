//! MD8: a world of several terrains — three tiles at three spacings and two joins between them — asks
//! each tile's ground in the tile's own coordinates, so every answer is the tile alone's, bit for bit.
//!
//! The terrains are written here ([`game_sim::tiles::generate`]) from one height function in world
//! feet, so the ground runs on across the joins as a mod's blended joins do. Nothing is off a disc.

use game_sim::contact_search::{self, Heightfield};
use game_sim::tiles::{generate, Piece, Reach, Source, Tile, Tiled};
use game_sim::track::{Terrain, TerrainLayout};
use game_sim::world::{Ground, Hit, World};
use ps2_float::{F, Vf};
use std::cell::Cell;
use std::rc::Rc;

/// The world's height in feet at a world point: rolling ground, one wave across and one along.
fn relief(x: f64, z: f64) -> f64 {
    120.0 + 6.0 * (x / 97.0).sin() * (z / 61.0).cos() + 2.5 * ((x + z) / 23.0).sin()
}

/// One terrain of the test world: where it sits, its spacing and root cells, and what it keeps.
struct Spec {
    name: &'static str,
    at: [u32; 2],
    spacing: f32,
    cells: (u32, u32),
    /// `None` for a join, which keeps every cell.
    keep: Option<[u32; 4]>,
    surface: u8,
}

/// Tile `a` (9 ft), join one, tile `b` (4.5 ft), join two, tile `c` (6 ft), in a row along `x`, each
/// at a whole number of its own leaves. `c` is shallower than the rest, so the world has a hole
/// behind it.
const SPECS: [Spec; 5] = [
    Spec { name: "a", at: [0, 0], spacing: 9.0, cells: (4, 4), keep: Some([0, 0, 56, 63]), surface: 3 },
    Spec { name: "b", at: [648, 0], spacing: 4.5, cells: (8, 8), keep: Some([8, 0, 120, 127]), surface: 5 },
    Spec { name: "c", at: [1344, 0], spacing: 6.0, cells: (4, 3), keep: Some([4, 0, 63, 47]), surface: 9 },
    Spec { name: "join_ab", at: [432, 0], spacing: 9.0, cells: (2, 4), keep: None, surface: 1 },
    Spec { name: "join_bc", at: [1152, 0], spacing: 9.0, cells: (2, 4), keep: None, surface: 2 },
];

/// A terrain's file: the world's relief from its unshifted place, so a shifted world is the same
/// bytes further out.
fn bytes(s: &Spec) -> Vec<u8> {
    let sp = f64::from(s.spacing);
    let (ox, oz) = (f64::from(s.at[0]), f64::from(s.at[1]));
    generate::terrain(s.name, s.cells, s.spacing, |x, z| relief(ox + f64::from(x) * sp, oz + f64::from(z) * sp), |x, z| {
        s.surface + ((x / 16 + z / 16) % 2) as u8
    })
}

fn layout(s: &Spec) -> TerrainLayout {
    TerrainLayout {
        width: s.cells.0,
        breadth: s.cells.1,
        width_scale: F(s.spacing),
        breadth_scale: F(s.spacing),
        ..TerrainLayout::default()
    }
}

/// The test's tiles from memory, counting loads.
struct Memory {
    files: Vec<Vec<u8>>,
    loads: Rc<Cell<usize>>,
}

impl Source for Memory {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        self.loads.set(self.loads.get() + 1);
        Ok(Piece { terrain: self.files[index].clone(), shapes: Vec::new() })
    }
}

/// The world, every terrain moved by `shift` feet, and its load counter.
fn world(shift: [u32; 2], reach: Reach) -> (Tiled, Rc<Cell<usize>>) {
    let loads = Rc::new(Cell::new(0));
    let tiles = SPECS
        .iter()
        .map(|s| {
            let mut t = Tile {
                name: s.name.into(),
                at: [F((s.at[0] + shift[0]) as f32), F((s.at[1] + shift[1]) as f32)],
                layout: layout(s),
                spacing: F(s.spacing),
                levels: 1,
                keep: [0; 4],
                first_owner: 0,
            };
            t.keep = s.keep.unwrap_or_else(|| t.every_cell());
            t
        })
        .collect();
    let source = Memory { files: SPECS.iter().map(|s| bytes(s)).collect(), loads: loads.clone() };
    (Tiled::new(tiles, Box::new(source), reach).unwrap(), loads)
}

/// A little generator, so the points are the same every run.
struct Lcg(u64);
impl Lcg {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn same_hit(a: Option<Hit>, b: Option<Hit>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            a.y.0.to_bits() == b.y.0.to_bits()
                && a.surface == b.surface
                && a.normal.0.iter().zip(b.normal.0.iter()).all(|(p, q)| p.to_bits() == q.to_bits())
        }
        (None, None) => true,
        _ => false,
    }
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

/// Every owned point of every terrain, against that terrain ridden alone: [`Ground::at`] (the
/// suspension's) and the heightfield query (the wheels' surface, the wreck, the plane), bit for bit.
fn check_against_alone(shift: [u32; 2]) -> usize {
    let (tiled, _) = world(shift, Reach::DESKTOP);
    let mut rng = Lcg(0x4d44_38);
    let mut checked = 0;
    for (i, s) in SPECS.iter().enumerate() {
        let file = bytes(s);
        let terrain = Terrain::parse(&file).unwrap();
        let alone = Ground::new(&terrain, &layout(s));
        let field = alone.field();
        let t = &tiled.tiles[i];
        let [x0, z0, x1, z1] = t.keep;
        let at = [f64::from(s.at[0] + shift[0]), f64::from(s.at[1] + shift[1])];
        for _ in 0..4000 {
            // A local point inside what it keeps, put in the world by host arithmetic, rounded to a
            // float as a rider's position is.
            let lx = (f64::from(x0) + rng.unit() * f64::from(x1 - x0)) * f64::from(s.spacing);
            let lz = (f64::from(z0) + rng.unit() * f64::from(z1 - z0)) * f64::from(s.spacing);
            let (wx, wz) = ((at[0] + lx) as f32, (at[1] + lz) as f32);
            // The local point the tile sees is the world point less the offset, exactly.
            let (lx, lz) = ((f64::from(wx) - at[0]) as f32, (f64::from(wz) - at[1]) as f32);
            let Some((owner, ox, oz)) = tiled.owner(F(wx), F(wz)) else { continue };
            if owner != i {
                // A tile ahead of it in the order holds this one (a join under a tile).
                assert!(owner < i, "{}: ({wx}, {wz}) went to {}", s.name, tiled.tiles[owner].name);
                continue;
            }
            assert_eq!((ox.0.to_bits(), oz.0.to_bits()), (lx.to_bits(), lz.to_bits()), "{}: the move into the tile rounded", s.name);
            let want = alone.at(F(lx), F(lz));
            assert!(want.is_some(), "{}: ({lx}, {lz}) owned but off its terrain", s.name);
            assert!(same_hit(tiled.at(F(wx), F(wz)), want), "{}: ({wx}, {wz})", s.name);

            let world_point = Vf::new(wx, 500.0, wz, 1.0);
            let got = contact_search::ground(&tiled.field(), world_point);
            let alone_g = contact_search::ground(&field, Vf::new(lx, 500.0, lz, 1.0));
            assert_eq!(got.point.y().to_bits(), alone_g.point.y().to_bits(), "{}: ({wx}, {wz})", s.name);
            assert_eq!((got.point.x().to_bits(), got.point.z().to_bits()), (wx.to_bits(), wz.to_bits()));
            assert_eq!(bits(got.normal), bits(alone_g.normal));
            assert_eq!(bits(got.weights), bits(alone_g.weights));
            assert_eq!(got.surfaces, alone_g.surfaces);
            assert_eq!(
                contact_search::nearest_surface(&tiled.field(), world_point),
                contact_search::nearest_surface(&field, Vf::new(lx, 500.0, lz, 1.0))
            );
            checked += 1;
        }
    }
    checked
}

#[test]
fn each_tile_is_its_own_ground_to_the_bit() {
    let n = check_against_alone([0, 0]);
    assert!(n > 12_000, "only {n} points checked");
}

/// The same world 110,592 feet out on `x` and 30,000 on `z` (the ultra map's reach): the ground
/// under a world point is still the tile alone's, though each world point is a coarser float.
#[test]
fn far_from_the_origin_the_ground_is_still_each_tiles() {
    let n = check_against_alone([110_592, 29_952]);
    assert!(n > 12_000, "only {n} points checked");
}

/// The move into a tile's coordinates, `world − at`, on the EE's subtract, against exact arithmetic:
/// every float at or past a whole `at` below 2²⁴ comes out exact.
#[test]
fn the_move_into_a_tile_is_exact() {
    let mut rng = Lcg(7);
    let mut n = 0;
    for at in [0u32, 432, 648, 1152, 1344, 5_760, 110_592, 116_064, 1 << 20, (1 << 24) - 4096] {
        for _ in 0..2000 {
            let x = (f64::from(at) + rng.unit() * 6000.0) as f32;
            if f64::from(x) < f64::from(at) {
                continue;
            }
            let got = F(x) - F(at as f32);
            let want = f64::from(x) - f64::from(at);
            assert_eq!(f64::from(got.0), want, "{x} - {at}");
            n += 1;
        }
    }
    assert!(n > 10_000);
}

/// Who owns what: a tile over the join under it, the join where no tile keeps, nothing past `c`'s
/// shallow end; the edge is the far corner of everything owned.
#[test]
fn ownership_holes_and_the_edge() {
    let (tiled, _) = world([0, 0], Reach::DESKTOP);
    let name = |x: f32, z: f32| tiled.owner(F(x), F(z)).map(|(i, _, _)| tiled.tiles[i].name.as_str());
    assert_eq!(name(100.0, 100.0), Some("a"));
    assert_eq!(name(500.0, 100.0), Some("a")); // the join lies under a's last kept cells
    assert_eq!(name(510.0, 100.0), Some("join_ab"));
    assert_eq!(name(690.0, 100.0), Some("b"));
    assert_eq!(name(1190.0, 100.0), Some("join_bc"));
    assert_eq!(name(1400.0, 100.0), Some("c"));
    assert_eq!(name(1400.0, 400.0), Some("join_bc"));
    assert_eq!(name(1500.0, 400.0), None); // behind c, past the join: a hole
    assert_eq!(name(1800.0, 100.0), None); // past everything

    let area = tiled.area;
    assert_eq!(area.width.0, 1344.0 + 63.0 * 6.0);
    assert_eq!(area.depth.0, 127.0 * 4.5);

    // The hole inside the rectangle is off the world; ground near a join is not; the outer edge is
    // left to the area's own test.
    let p = |x: f32, z: f32| Vf::new(x, 0.0, z, 1.0);
    assert!(tiled.off_world(p(1500.0, 450.0), F(150.0)));
    assert!(tiled.off_world(p(1500.0, 200.0), F(150.0)), "150 feet short of the hole");
    assert!(!tiled.off_world(p(1500.0, 100.0), F(150.0)));
    assert!(!tiled.off_world(p(510.0, 300.0), F(150.0)));
    assert!(!tiled.off_world(p(100.0, 100.0), F(150.0)));
    // The ground in a hole is none, as off a track's edge.
    let mut tiled = tiled;
    assert!(tiled.at(F(1500.0), F(450.0)).is_none());
    assert_eq!(tiled.ground(p(1500.0, 450.0)).0.y(), tiled.ground(p(1800.0, 9.0)).0.y());
    assert!(tiled.ground(p(1500.0, 450.0)).0.y() < -1000.0);
}

/// Loading and dropping is memory: tiles come and go with the riders and the answers never change.
#[test]
fn streaming_never_changes_an_answer() {
    let (mut tiled, loads) = world([0, 0], Reach { load: F(300.0), drop: F(500.0) });
    assert!(tiled.loaded().is_empty(), "nothing is loaded before it is asked");
    let points: Vec<(f32, f32)> = (0..400).map(|i| (4.3 * i as f32 + 0.37, 280.0 + (i % 7) as f32)).collect();
    let before: Vec<Option<Hit>> = points.iter().map(|&(x, z)| tiled.at(F(x), F(z))).collect();
    assert_eq!(tiled.loaded().len(), 5);

    // A rider at c's far end: a and its join go.
    tiled.stream(&[Vf::new(1700.0, 0.0, 100.0, 1.0)]);
    let held = tiled.loaded();
    assert!(!held.contains(&0) && !held.contains(&3), "{held:?}");
    let log = tiled.take_log();
    assert!(log.iter().any(|l| l == "world: a dropped"), "{log:?}");
    let bytes_far = tiled.loaded_bytes();

    // Asked again, a comes back on demand with the same ground.
    let after: Vec<Option<Hit>> = points.iter().map(|&(x, z)| tiled.at(F(x), F(z))).collect();
    for (p, (a, b)) in points.iter().zip(before.iter().zip(after.iter())) {
        assert!(same_hit(*a, *b), "{p:?}");
    }
    assert!(tiled.loaded().contains(&0));
    assert!(loads.get() > 5, "a was loaded again");
    println!("terrains held near c: {bytes_far} bytes; all five: {}", tiled.loaded_bytes());
}

/// A world refuses an offset the move into its coordinates could round.
#[test]
fn a_world_refuses_offsets_that_could_round() {
    for at in [0.5f32, -9.0, 16_777_216.0] {
        let t = Tile {
            name: "x".into(),
            at: [F(at), F(0.0)],
            layout: layout(&SPECS[0]),
            spacing: F(9.0),
            levels: 1,
            keep: [0, 0, 10, 10],
            first_owner: 0,
        };
        let source = Memory { files: vec![], loads: Rc::new(Cell::new(0)) };
        assert!(Tiled::new(vec![t], Box::new(source), Reach::DESKTOP).is_err(), "{at}");
    }
}

/// The heightfield the plane and the wreck read covers the whole world.
#[test]
fn the_worlds_field_is_the_whole_world() {
    let (tiled, _) = world([0, 0], Reach::DESKTOP);
    let f = tiled.field();
    assert_eq!(f.max().x(), tiled.area.width.0);
    assert_eq!(f.max().z(), tiled.area.depth.0);
}

/// What re-reading a terrain's header costs a query (the world reads it again for each).
#[test]
fn a_query_costs_little_more_than_the_tile_alone() {
    let (tiled, _) = world([0, 0], Reach::DESKTOP);
    let s = &SPECS[1];
    let file = bytes(s);
    let terrain = Terrain::parse(&file).unwrap();
    let alone = Ground::new(&terrain, &layout(s));
    let n = 20_000;
    let pts: Vec<(f32, f32)> = (0..n).map(|i| (700.0 + (i % 400) as f32 * 1.13, 10.0 + (i / 400) as f32 * 9.7)).collect();
    tiled.at(F(700.0), F(10.0));
    let t0 = std::time::Instant::now();
    let mut sum = 0u32;
    for &(x, z) in &pts {
        sum ^= tiled.at(F(x), F(z)).map_or(0, |h| h.y.0.to_bits());
    }
    let world_ns = t0.elapsed().as_nanos() as f64 / f64::from(n);
    let t0 = std::time::Instant::now();
    for &(x, z) in &pts {
        sum ^= alone.at(F(x - 648.0), F(z)).map_or(0, |h| h.y.0.to_bits());
    }
    let alone_ns = t0.elapsed().as_nanos() as f64 / f64::from(n);
    println!("a world query {world_ns:.0} ns, the tile alone {alone_ns:.0} ns ({sum})");
}
