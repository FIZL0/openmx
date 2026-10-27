//! MD29: a world that wraps round. Ride off its east edge and come in at its west.
//!
//! The world is two generated tiles in a row along `x`, 1,134 feet wide, over ground that repeats
//! every 1,134 feet, so its far edge meets its near one. The same two tiles laid twice over, 2,268
//! feet and not wrapped, are the ride's control: the second pair is where the wrapped ride's
//! queries go when it is past the seam.
//!
//! What is proved:
//!
//! * up to and including the tick the bike crosses the seam, the wrapped ride is the wide one, bit
//!   for bit, and that tick's shift is exact: every world point it moves is the wide ride's less
//!   the width, to the bit;
//! * after the shift the ride carries on as the wide one does, within 0.05 ft over three seconds.
//!   Not bit for bit: the wrapped ride's positions are near 0 and the wide one's past
//!   1,134 feet, where a float holds fewer of its bits, so the same sums round differently. Two rides
//!   a width apart on a world with no seam at all part the same way, at the same rate;
//! * the edge throw never fires on the wrapped axis, through lap after lap, and fires on the axis
//!   that does not wrap as it always has;
//! * riding west over the near seam comes in at the far one;
//! * the edge test asks only the axes that do not wrap, over generated points.
//!
//! The stock bike with `tests/ride.rs`'s wreck limits rides it, so nothing is off a disc.

use game_sim::biplane::Area;
use game_sim::off_map;
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::trick::Wreck;
use game_sim::tiles::{generate, Piece, Reach, Source, Tile, Tiled};
use game_sim::track::TerrainLayout;
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::sync::Arc;

/// The wrapped world's width: two tiles of 63 cells at 9 feet.
const WIDTH: f32 = 1134.0;
const SPACING: f32 = 9.0;
const DT: F = F(1.0 / 30.0);

/// The ground: gentle rolls that repeat every [`WIDTH`] feet along `x`.
fn relief(x: f64, z: f64) -> f64 {
    let w = f64::from(WIDTH);
    40.0 + 0.8 * (x * 2.0 * std::f64::consts::TAU / w).sin() * (z / 90.0).cos()
}

/// The wreck limits the stock bike needs to ride (`tests/ride.rs`'s).
const RIDE_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

struct Memory(Vec<Vec<u8>>);

impl Source for Memory {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        Ok(Piece { terrain: self.0[index].clone(), shapes: Vec::new() })
    }
}

/// `copies` × two tiles along `x`, each 64 × 64 samples at 9 feet, keeping 63 cells each way.
fn world(copies: u32, wrap: bool) -> Tiled {
    let mut tiles = Vec::new();
    let mut files = Vec::new();
    for k in 0..2 * copies {
        let at = k * 567;
        let sp = f64::from(SPACING);
        files.push(generate::terrain("t", (4, 4), SPACING, |x, z| relief(f64::from(at) + f64::from(x) * sp, f64::from(z) * sp), |_, _| 0));
        tiles.push(Tile {
            name: format!("t{k}"),
            at: [F(at as f32), F::ZERO],
            layout: TerrainLayout { width: 4, breadth: 4, width_scale: F(SPACING), breadth_scale: F(SPACING), ..TerrainLayout::default() },
            spacing: F(SPACING),
            levels: 1,
            keep: [0, 0, 63, 63],
            first_owner: 0,
        });
    }
    let tiled = Tiled::new(tiles, Box::new(Memory(files)), Reach::DESKTOP).unwrap();
    tiled.wrapped([wrap, false]).unwrap()
}

/// The gas held, steering straight.
fn gas() -> Controls {
    Controls { gas: F(1.0), ..Controls::default() }
}

/// The stand-in bike with a steering scale, the drag that lets it reach the original's top speed,
/// and the wreck limits of a bike being ridden (`tests/ride.rs`'s `ridden_bike`).
fn ridden_bike() -> String {
    let mut out = stock::BIKE.to_string();
    for (key, with) in [
        (
            "XAxisRateLimit = 6\n",
            "XAxisRateLimit = 6\nSteerScale1 = 1.0\nSteerScale2 = 0.05\nSteerSpeed1 = 5\nSteerSpeed2 = 50\n",
        ),
        ("Drag = 0.5", "Drag = 0.037"),
    ] {
        assert!(out.contains(key), "game_sim::stock::BIKE no longer has `{}`", key.trim_end());
        out = out.replace(key, with);
    }
    format!("{out}{RIDE_LIMITS}")
}

/// A bike on [`relief`] at `x`, `z`, heading along `x` (`east`) or against it, wrecks on.
fn sim(x: f32, z: f32, east: bool) -> Sim {
    sim_on(relief, x, z, east)
}

/// [`sim`] on the ground `height`.
fn sim_on(height: fn(f64, f64) -> f64, x: f32, z: f32, east: bool) -> Sim {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let y = height(f64::from(x), f64::from(z)) as f32 + 1.0;
    let facing = Vf::new(if east { 1.0 } else { -1.0 }, 0.0, 0.0, 0.0);
    let rider = Rider::new(&phy, stock::mounts(&geom), Vf::new(x, y, z, 1.0), facing, Set::default());
    Sim::with_rider(0, tables, phy, rider)
}

/// `s` without the put-back query's broad-phase list. The list's reach is cut to the cells of the
/// world's broad-phase space, which is laid over the world's extent, and the two worlds' extents
/// differ (the wide one is twice as wide; the wrapped one's runs a width past each seam). It is a
/// cache: what it holds within that reach is the same either way, and with no props it holds nothing.
fn uncached(s: &Sim) -> Sim {
    let mut s = s.clone();
    s.rider.ride.back_search = None;
    s
}

fn bits(v: Vf) -> [u32; 3] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits()]
}

/// `v` moved back along `x` by the width, exactly (`v.x` is at or past it).
fn less_width(v: Vf) -> Vf {
    Vf::new((F(v.x()) - F(WIDTH)).0, v.y(), v.z(), v.w())
}

/// Every world point [`game_sim::rider::Rider::shift`] moves, by name.
fn places(s: &Sim) -> Vec<(&'static str, Vf)> {
    let r = &s.rider.ride;
    vec![
        ("cofg", r.chassis.cofg_world),
        ("origin", r.origin),
        ("front mount", r.front.w.mount_world),
        ("front probe end", r.front.w.probe_end_world),
        ("front contact", r.front.w.contact_point),
        ("front load", r.front.w.load_point),
        ("rear mount", r.rear.w.mount_world),
        ("rear probe end", r.rear.w.probe_end_world),
        ("rear contact", r.rear.w.contact_point),
        ("rear load", r.rear.w.load_point),
    ]
}

#[test]
fn a_ride_over_the_seam_is_the_wide_worlds_ride_and_the_shift_is_exact() {
    let (mut wrapped, mut wide) = (world(1, true), world(2, false));
    let (mut a, mut b) = (sim(980.0, 283.0, true), sim(980.0, 283.0, true));
    let mut crossed = None;
    for tick in 0..900 {
        let fa = a.step(DT, &gas(), &mut wrapped);
        let fb = b.step(DT, &gas(), &mut wide);
        assert_eq!(fb.seam, [0, 0], "the wide world does not wrap");
        assert!(a.rider.state.wreck.is_none() && b.rider.state.wreck.is_none(), "tick {tick}: a wreck");
        if fa.seam == [0, 0] {
            let why = game_sim::hash::first_difference(&uncached(&a), &uncached(&b));
            assert!(why.is_none(), "tick {tick}: the wrapped ride parted from the wide one before the seam: {why:?}");
            continue;
        }
        assert_eq!(fa.seam, [1, 0], "tick {tick}");
        // The tick the bike crossed: the same ride, and every world point one width back, exactly.
        for ((name, p), (_, q)) in places(&a).into_iter().zip(places(&b)) {
            assert_eq!(bits(p), bits(less_width(q)), "tick {tick}: {name} {p:?} against {q:?}");
        }
        assert_eq!(bits(a.rider.ride.chassis.vel), bits(b.rider.ride.chassis.vel));
        assert_eq!(bits(a.rider.ride.chassis.quat), bits(b.rider.ride.chassis.quat));
        crossed = Some(tick);
        break;
    }
    let crossed = crossed.expect("the bike never reached the seam");
    // Three seconds on: the same ride, a width apart, within 0.05 ft. It drifts along x by about
    // 3.5e-4 ft a tick (0.032 ft over the 90 ticks), as two rides a width apart on a world with no
    // seam drift too ([`a_ride_a_width_further_out_drifts_as_far_without_a_seam`]).
    let mut worst = 0.0f32;
    for tick in 0..90 {
        let fa = a.step(DT, &gas(), &mut wrapped);
        b.step(DT, &gas(), &mut wide);
        assert_eq!(fa.seam, [0, 0], "tick {}: crossed again", crossed + 1 + tick);
        let (p, q) = (a.rider.ride.chassis.cofg_world, b.rider.ride.chassis.cofg_world);
        let gap = ((p.x() + WIDTH - q.x()).abs()).max((p.y() - q.y()).abs()).max((p.z() - q.z()).abs());
        worst = worst.max(gap);
    }
    assert!(worst < 0.05, "the wrapped ride parted from the wide one by {worst} feet");
    let v = a.rider.ride.chassis.vel;
    assert!(v.x() > 10.0, "the bike stopped: {v:?}");
}

#[test]
fn lap_after_lap_the_edge_throw_never_fires_on_the_wrapped_axis() {
    let mut wrapped = world(1, true);
    let mut s = sim(300.0, 283.0, true);
    let mut seams = 0;
    for tick in 0..3000 {
        let f = s.step(DT, &gas(), &mut wrapped);
        assert_ne!(f.wreck, Some(Wreck::CANNONBALL), "tick {tick}: thrown at x {}", s.rider.ride.chassis.cofg_world.x());
        seams += i32::from(f.seam[0]);
        if s.rider.state.wreck.is_none() {
            let x = s.rider.ride.chassis.cofg_world.x();
            assert!((0.0..WIDTH).contains(&x), "tick {tick}: riding at x {x}, outside the world");
        }
    }
    assert!(seams >= 2, "only {seams} seams crossed, at {:?}", s.rider.ride.chassis.cofg_world);
    // The same ride on the world unwrapped is thrown at its east edge.
    let mut edged = world(1, false);
    let mut s = sim(300.0, 283.0, true);
    let thrown = (0..3000).any(|_| s.step(DT, &gas(), &mut edged).wreck == Some(Wreck::CANNONBALL));
    assert!(thrown, "the unwrapped world never threw the bike");
}

#[test]
fn riding_west_over_the_near_seam_comes_in_at_the_far_one() {
    let mut wrapped = world(1, true);
    let mut s = sim(150.0, 283.0, false);
    let mut crossed = false;
    for tick in 0..900 {
        let before = s.rider.ride.chassis.cofg_world;
        let f = s.step(DT, &gas(), &mut wrapped);
        assert!(s.rider.state.wreck.is_none(), "tick {tick}: a wreck");
        let after = s.rider.ride.chassis.cofg_world;
        if f.seam == [-1, 0] {
            assert!(before.x() < 30.0 && after.x() > WIDTH - 30.0, "tick {tick}: {before:?} to {after:?}");
            crossed = true;
        }
        if crossed && tick % 10 == 0 {
            // On the ground the far side has, not falling through it.
            let (g, _) = wrapped.ground(after);
            assert!((after.y() - g.y()).abs() < 4.0, "tick {tick}: {} over ground at {}", after.y(), g.y());
        }
    }
    assert!(crossed, "the bike never reached the near seam");
}

#[test]
fn the_edge_test_asks_only_the_axes_that_do_not_wrap() {
    let area = Area { width: F(2000.0), depth: F(1500.0) };
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut unit = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 40) as f32 / (1u64 << 24) as f32
    };
    for _ in 0..20_000 {
        let p = Vf::new(unit() * 2600.0 - 300.0, 0.0, unit() * 2100.0 - 300.0, 1.0);
        let bone = Vf::new(unit() * 2600.0 - 300.0, 1.0, unit() * 2100.0 - 300.0, 1.0);
        let mid = |v: Vf, x: bool, z: bool| Vf::new(if x { 1000.0 } else { v.x() }, v.y(), if z { 750.0 } else { v.z() }, 1.0);
        for wrap in [[false, false], [true, false], [false, true], [true, true]] {
            let want = off_map::outside(&area, mid(bone, wrap[0], wrap[1]), mid(p, wrap[0], wrap[1]));
            assert_eq!(off_map::outside_wrapped(&area, wrap, bone, p), want, "{wrap:?} {bone:?} {p:?}");
        }
    }
}

/// The control for the drift after the seam: the same ride on the wide world, from 300 ft and from
/// a width further out, with no seam anywhere. Its ground is the same to the bit, and still the ride
/// further out falls behind along x tick by tick, its sums rounding to a coarser float.
#[test]
fn a_ride_a_width_further_out_drifts_as_far_without_a_seam() {
    let mut w = world(2, false);
    let (mut a, mut b) = (sim(300.0, 283.0, true), sim(300.0 + WIDTH, 283.0, true));
    let mut gap = 0.0f32;
    for _ in 0..90 {
        a.step(DT, &gas(), &mut w);
        b.step(DT, &gas(), &mut w);
        let (p, q) = (a.rider.ride.chassis.cofg_world, b.rider.ride.chassis.cofg_world);
        gap = p.x() + WIDTH - q.x();
        assert!((p.z() - q.z()).abs() < 1e-3 && (p.y() - q.y()).abs() < 1e-3, "{p:?} against {q:?}");
    }
    assert!(gap > 0.01 && gap < 0.05, "the rides a width apart drifted {gap} ft");
}

#[test]
fn a_point_folds_into_the_world_and_a_point_inside_is_left_alone() {
    let w = world(1, true);
    assert_eq!(w.wraps(), [true, false]);
    assert_eq!(w.extent()[0], F(WIDTH));
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..20_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let x = F((seed >> 40) as f32 / (1u64 << 24) as f32 * WIDTH);
        assert_eq!(w.fold_axis(0, x).0.to_bits(), x.0.to_bits(), "{x:?} is inside");
        // A point a lap on folds back by the width exactly: what the rider's shift does east.
        let on = x + F(WIDTH);
        assert_eq!(w.fold_axis(0, on).0.to_bits(), (on - F(WIDTH)).0.to_bits(), "{on:?} a lap on");
        let far = F(x.0 - 3.0 * WIDTH);
        let folded = w.fold_axis(0, far);
        assert!(folded >= F::ZERO && folded < F(WIDTH), "{far:?} folded to {folded:?}");
        // The axis that does not wrap is never folded.
        assert_eq!(w.fold_axis(1, far).0.to_bits(), far.0.to_bits());
    }
    assert_eq!(w.laps(F(WIDTH), F(10.0)), [1, 0]);
    assert_eq!(w.laps(F(-0.5), F(-10.0)), [-1, 0]);
}

#[test]
fn only_a_whole_number_of_feet_wraps() {
    // One tile of 63 cells at 4.5 feet: 283.5 feet each way, which a shift could not move exactly.
    let tile = Tile {
        name: "half".into(),
        at: [F::ZERO, F::ZERO],
        layout: TerrainLayout { width: 4, breadth: 4, width_scale: F(4.5), breadth_scale: F(4.5), ..TerrainLayout::default() },
        spacing: F(4.5),
        levels: 1,
        keep: [0, 0, 63, 63],
        first_owner: 0,
    };
    let make = || {
        let bytes = generate::terrain("half", (4, 4), 4.5, |_, _| 40.0, |_, _| 0);
        Tiled::new(vec![tile.clone()], Box::new(Memory(vec![bytes])), Reach::DESKTOP).unwrap()
    };
    assert!(make().wrapped([true, false]).is_err());
    assert!(make().wrapped([false, true]).is_err());
    assert!(make().wrapped([false, false]).is_ok());
}

/// Ground that does not come round: it climbs 30 feet from west to east and rolls along `z`.
fn climb(x: f64, z: f64) -> f64 {
    40.0 + 30.0 * x / f64::from(WIDTH) + 2.0 * (z / 50.0).sin()
}

/// Two tiles over [`climb`], 1,134 × 567 feet, and the seam joins that close it on both axes.
fn closed() -> (Tiled, [f32; 2]) {
    let tile = |k: u32| {
        let at = k * 567;
        let sp = f64::from(SPACING);
        let bytes = generate::terrain("c", (4, 4), SPACING, |x, z| climb(f64::from(at) + f64::from(x) * sp, f64::from(z) * sp), |_, _| 0);
        let t = Tile {
            name: format!("c{k}"),
            at: [F(at as f32), F::ZERO],
            layout: TerrainLayout { width: 4, breadth: 4, width_scale: F(SPACING), breadth_scale: F(SPACING), ..TerrainLayout::default() },
            spacing: F(SPACING),
            levels: 1,
            keep: [0, 0, 63, 63],
            first_owner: 0,
        };
        (t, bytes)
    };
    let (mut tiles, mut files): (Vec<Tile>, Vec<Vec<u8>>) = (0..2).map(tile).unzip();
    let open = Tiled::new(tiles.clone(), Box::new(Memory(files.clone())), Reach::DESKTOP).unwrap();
    let [w0, d0] = open.extent().map(|v| v.0);
    let ground = |x: f64, z: f64| {
        // Just inside the far edges, which no tile owns.
        let (x, z) = ((x as f32).min(w0 - 0.01), (z as f32).min(d0 - 0.01));
        open.at(F(x), F(z)).map_or((0.0, 0), |h| (f64::from(h.y.0), h.surface))
    };
    let joins = generate::seam_joins([w0 as u32, d0 as u32], [true, true], 9, &ground).unwrap();
    for (t, b) in joins {
        tiles.push(t);
        files.push(b);
    }
    let world = Tiled::new(tiles, Box::new(Memory(files)), Reach::DESKTOP).unwrap().wrapped([true, true]).unwrap();
    (world, [w0, d0])
}

#[test]
fn seam_joins_close_a_world_whose_ground_does_not_come_round() {
    let (w, [w0, d0]) = closed();
    let strip = (generate::SEAM_CELLS * 9) as f32;
    let [w1, d1] = w.extent().map(|v| v.0);
    assert_eq!([w1, d1], [w0 + strip, d0 + strip], "a strip on each axis, and nothing more");
    let y = |x: f32, z: f32| w.at(F(x), F(z)).unwrap_or_else(|| panic!("no ground at {x}, {z}")).y.0;
    let mut v = 0.5f32;
    while v < d1.max(w1) {
        // Each seam and each strip's near edge meet within a raw height unit and a sample's slope.
        if v < d1 {
            assert!((y(w1 - 0.01, v) - y(0.0, v)).abs() < 0.1, "the x seam at z {v}");
            assert!((y(w0 + 0.01, v) - y(w0 - 0.01, v)).abs() < 0.1, "the x strip's near edge at z {v}");
        }
        if v < w1 {
            assert!((y(v, d1 - 0.01) - y(v, 0.0)).abs() < 0.1, "the z seam at x {v}");
            if v < w0 {
                assert!((y(v, d0 + 0.01) - y(v, d0 - 0.01)).abs() < 0.1, "the z strip's near edge at x {v}");
            }
        }
        v += 4.5;
    }
    assert!((y(w1 - 0.01, d1 - 0.01) - y(0.0, 0.0)).abs() < 0.1, "the corner");
}

#[test]
fn a_ride_east_crosses_a_made_seam_and_is_not_thrown() {
    let (mut w, _) = closed();
    let mut s = sim_on(climb, 600.0, 283.0, true);
    let mut seams = 0;
    for tick in 0..1500 {
        let f = s.step(DT, &gas(), &mut w);
        assert_eq!(f.wreck, None, "tick {tick}: a wreck at {:?}", s.rider.ride.chassis.cofg_world);
        seams += i32::from(f.seam[0]);
    }
    assert!(seams >= 1, "never crossed, at {:?}", s.rider.ride.chassis.cofg_world);
}
