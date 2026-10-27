//! MD44: a world that wraps round, the rest of the simulation's shift — the plane, a wreck, and
//! bike against bike across a seam.
//!
//! The world is `tests/wrap.rs`'s: two generated tiles in a row along `x`, 1,134 feet wide, over
//! ground that repeats every 567 feet, wrapped on `x` only; the same tiles laid twice over and not
//! wrapped are the control.
//!
//! What is proved:
//!
//! * a plane flown east over the seam is the wide world's flight, bit for bit, up to and including
//!   the tick it crosses; that tick its body and every place it keeps are the wide flight's less the
//!   width, exactly, its speed, spin and attitude the same bits; and it flies on as the wide one does
//!   (the disc's plane: skips without it);
//! * a wreck thrown over the seam is moved whole on the tick its bike crosses: the wreck bike, its
//!   record, the transform's origin and the rider's point, each the wide wreck's less the width, its
//!   speeds the same bits, and it carries on to its put-back inside the world;
//! * two bikes a seam apart strike each other as two side by side do: what the bike by the near
//!   seam strikes is the same to the bit, and what the bike by the far one strikes (searching at
//!   1,133 feet, where a float holds fewer bits) is the same within rounding, a width over on its own
//!   list and brought back on the other's (the disc's collision files: skips without them);
//! * two riders a seam apart touch: the same contacts each tick as the same pair 567 feet in, away
//!   from any seam, the same ride within 0.01 ft over a second, and pushed apart as the pair without
//!   the files is not.

use game_render::disc::Disc;
use game_sim::archive::{Psr, Res};
use game_sim::bike_col::{self, BikeCol};
use game_sim::biplane::Pad;
use game_sim::flight::Flight;
use game_sim::math::Mat4;
use game_sim::phy::{self, Phy, Plane};
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::rider_contact::{self, Seat};
use game_sim::tiles::{generate, Piece, Reach, Source, Tile, Tiled};
use game_sim::track::TerrainLayout;
use game_sim::trick::Set;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;

/// The wrapped world's width: two tiles of 63 cells at 9 feet.
const WIDTH: f32 = 1134.0;
const SPACING: f32 = 9.0;
const DT: F = F(1.0 / 30.0);

/// The ground: gentle rolls that repeat every half [`WIDTH`] along `x`.
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

/// `tests/ride.rs`' made-up wreck body, and the rider's wreck numbers (`tests/off_the_map.rs`'s).
const WRECK_PART: &str = "
[PhysicsPart1]
Type = RigidBody
Name = wreck_physics
Weight = 250.0
CollisionWeight = 0.0001
GravityScale = 1.0
InertiaScale = 1.0, 1.0, 1.0
SpringStiffness = 5.0
SpringDamping = 0.4
MaxPenetration = 0.5
CofG = 0.0, 1.5, 0.0
SpringMomentScale = 1.0, 1.0, 1.0
FrictionMomentScale = 1.0, 1.0, 1.0
FrictionZone1 = 0.5
Radius = 1.0
AccelerationThreshold = 0.01
AngularAccelerationThreshold = 0.01
";
const RIDER_WRECKS: &str = "
[Wrecks]
HitBottomForceTime = 0.1
HitBottomHandHoldTime = 0.3
HitBottomForce = 0.0, 800.0, 0.0
GroundAngularRateDamping = 1.0
RiderFlailHeight = 3.0
RiderRollTime = 0.75
RiderRollSpeed = 0.0
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

/// The stand-in bike with a steering scale, the drag that lets it reach the original's top speed,
/// and the wreck limits of a bike being ridden (`tests/wrap.rs`'s).
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

/// A rider on [`relief`] at `x`, `z`, facing `facing`.
fn rider(phy: &Phy, tables: &Tables, x: f32, z: f32, facing: Vf) -> Rider {
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let y = relief(f64::from(x), f64::from(z)) as f32 + 1.0;
    Rider::new(phy, stock::mounts(&geom), Vf::new(x, y, z, 1.0), facing, Set::default())
}

fn bits(v: Vf) -> [u32; 3] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits()]
}

/// `v` moved back along `x` by the width, exactly.
fn less_width(v: Vf) -> Vf {
    Vf::new((F(v.x()) - F(WIDTH)).0, v.y(), v.z(), v.w())
}

/// The largest gap between `p` and `q` moved along `x` by `dx`.
fn gap(p: Vf, q: Vf, dx: f32) -> f32 {
    (p.x() + dx - q.x()).abs().max((p.y() - q.y()).abs()).max((p.z() - q.z()).abs())
}

// --- The plane ---------------------------------------------------------------------------------

/// The disc's `Biplane\biplane.phy`, out of `machine.res` in `MX.PSR` (`tests/flight.rs`'s).
fn plane_file() -> Option<Vec<u8>> {
    let kit = Kit::load().ok()?;
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() { here } else { kit.ctx("iso").join("MX.PSR") };
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(phy::BIPLANE_PHY))
        .expect("machine.res carries the plane's file");
    Some(res.read(&bytes, e).expect("its bytes"))
}

/// Every place [`Flight::wrap_round`] moves, by name.
fn plane_places(f: &Flight) -> Vec<(&'static str, Vf)> {
    vec![
        ("position", f.body.position),
        ("last position", f.body.last_position),
        ("record origin", f.record.node.origin),
        ("collider cofg", f.collider.cofg_world),
    ]
}

#[test]
fn a_plane_flown_over_the_seam_comes_in_at_the_far_side_flying_as_it_was() {
    let Some(file) = plane_file() else { return };
    let tables = game_sim::ride::Tables::build();
    let (mut wrapped, mut wide) = (world(1, true), world(2, false));
    let mut a = Flight::new(&Plane::read_bytes(&file));
    a.attitude.flown = true;
    a.launch(Vf::new(1000.0, 300.0, 283.0, 1.0), Vf::new(1.0, 0.0, 0.0, 0.0), Vf::new(150.0, 0.0, 0.0, 0.0), &tables.acos);
    let mut b = a.clone();
    let pad = Pad { forward: F::ONE, ..Pad::default() };
    let mut crossed = None;
    for tick in 0..300 {
        let (_, sa) = a.frame_seamed(DT, pad, &mut wrapped, &tables, &[]);
        let (_, sb) = b.frame_seamed(DT, pad, &mut wide, &tables, &[]);
        assert_eq!(sb, [0, 0], "the wide world does not wrap");
        assert_eq!(a.crash.reason, 0, "tick {tick}: the plane crashed");
        if sa == [0, 0] {
            let why = game_sim::hash::first_difference(&a, &b);
            assert!(why.is_none(), "tick {tick}: the wrapped flight parted from the wide one before the seam: {why:?}");
            continue;
        }
        assert_eq!(sa, [1, 0], "tick {tick}");
        for ((name, p), (_, q)) in plane_places(&a).into_iter().zip(plane_places(&b)) {
            assert_eq!(bits(p), bits(less_width(q)), "tick {tick}: {name} {p:?} against {q:?}");
        }
        assert!((0.0..WIDTH).contains(&a.body.position.x()), "tick {tick}: not brought inside: {:?}", a.body.position);
        // The measure's matrix is taken again from the moved body, so its origin is the body's
        // less its turned centre of gravity, a few bits off the wide one's less the width.
        assert!(gap(a.measure.matrix.0[3], b.measure.matrix.0[3], WIDTH) < 1e-3, "tick {tick}: the matrix");
        // The same flight: speed, spin, attitude and every measure that is not a place.
        assert_eq!(a.body.velocity.to_bits(), b.body.velocity.to_bits());
        assert_eq!(a.body.rates.to_bits(), b.body.rates.to_bits());
        assert_eq!(a.body.quat.to_bits(), b.body.quat.to_bits());
        assert_eq!(a.measure.airspeed.0.to_bits(), b.measure.airspeed.0.to_bits());
        assert_eq!(a.measure.velocity.to_bits(), b.measure.velocity.to_bits());
        assert_eq!(a.attitude, b.attitude);
        assert_eq!(a.air, b.air);
        crossed = Some(tick);
        break;
    }
    let crossed = crossed.expect("the plane never reached the seam");
    // Two seconds on, a width apart: the same flight within rounding (the wide one's positions are
    // past 1,134 feet, where a float holds fewer bits; tests/wrap.rs measures the bike's drift).
    let (mut worst, mut worst_v) = (0.0f32, 0.0f32);
    for tick in 0..60 {
        let (_, sa) = a.frame_seamed(DT, pad, &mut wrapped, &tables, &[]);
        b.frame_seamed(DT, pad, &mut wide, &tables, &[]);
        assert_eq!(sa, [0, 0], "tick {}: crossed again", crossed + 1 + tick);
        worst = worst.max(gap(a.body.position, b.body.position, WIDTH));
        worst_v = worst_v.max(gap(a.body.velocity, b.body.velocity, 0.0));
    }
    assert!(worst < 0.05 && worst_v < 0.01, "the wrapped flight parted from the wide one by {worst} ft, {worst_v} ft/s");
    assert!(a.body.velocity.x() > 50.0, "the plane is not flying east: {:?}", a.body.velocity);
}

// --- A wreck -----------------------------------------------------------------------------------

/// A bike 15 feet up at `x`, flying east at 60 ft/s and rolling at 10 rad/s: an air wreck within a
/// few ticks, with a wreck body to throw.
fn rolling(tables: &Tables, x: f32) -> Sim {
    let phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let y = relief(f64::from(x), 283.0) as f32 + 16.0;
    let mut r = Rider::new(&phy, stock::mounts(&geom), Vf::new(x, y, 283.0, 1.0), Vf::new(1.0, 0.0, 0.0, 0.0), Set::default());
    let part = phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    r.carries_wreck(part, wrecks, part.inertia(Vf::new(2.5, 4.0, 7.0, 1.0)));
    let ride = &mut r.ride;
    ride.chassis.vel = Vf::new(60.0, 0.0, 0.0, 0.0);
    ride.chassis.ang_vel = Vf::new(0.0, 0.0, 10.0, 0.0);
    Sim::with_rider(0, Arc::new(Tables::build()), phy, r)
}

/// Every place a wreck in progress keeps, by name.
fn wreck_places(s: &Sim) -> Vec<(&'static str, Vf)> {
    let c = s.rider.crash.as_ref().expect("down");
    vec![
        ("wreck bike", c.bike.body.position),
        ("wreck bike last", c.bike.body.last_position),
        ("record origin", c.bike.record.node.origin),
        ("collider cofg", c.bike.collider.cofg_world),
        ("origin", c.origin),
        ("rider", c.rider),
        ("chassis", s.rider.ride.chassis.cofg_world),
        ("ride origin", s.rider.ride.origin),
    ]
}

/// The wreck is told apart from the shift by stepping a clone's rider alone: [`Sim::step`] is the
/// rider's frame (no traffic, no cars here) and then [`Rider::wrap_round`].
#[test]
fn a_wreck_thrown_over_the_seam_is_moved_whole_and_put_back_inside() {
    let tables = Tables::build();
    let mut wrapped = world(1, true);
    let mut a = rolling(&tables, 1100.0);
    let still = Controls::default();
    let mut crossed = None;
    for tick in 0..300 {
        let mut c = a.clone();
        let f = a.step(DT, &still, &mut wrapped);
        if f.seam == [0, 0] {
            continue;
        }
        assert_eq!(f.seam, [1, 0], "tick {tick}");
        assert!(a.rider.crash.is_some(), "tick {tick}: crossed riding, not down");
        let (phy, t) = (c.phy.clone(), c.tables.clone());
        c.rider.frame(&phy, &t, &still, DT, &mut wrapped);
        for ((name, p), (_, q)) in wreck_places(&a).into_iter().zip(wreck_places(&c)) {
            assert_eq!(bits(p), bits(less_width(q)), "tick {tick}: {name} {p:?} against {q:?}");
        }
        let (ca, cb) = (a.rider.crash.as_ref().unwrap(), c.rider.crash.as_ref().unwrap());
        assert_eq!(ca.bike.body.velocity.to_bits(), cb.bike.body.velocity.to_bits());
        assert_eq!(ca.bike.body.quat.to_bits(), cb.bike.body.quat.to_bits());
        assert_eq!(ca.rider_vel.to_bits(), cb.rider_vel.to_bits());
        assert!((0.0..WIDTH).contains(&ca.bike.body.position.x()), "tick {tick}: not brought inside");
        crossed = Some(tick);
        break;
    }
    crossed.expect("the wreck never reached the seam");
    // On to the put-back: inside the world, on the far side of the seam.
    let mut k = 0;
    while !a.step(DT, &still, &mut wrapped).respawned {
        k += 1;
        assert!(k < 3000, "no put-back in 100 s");
    }
    let at = a.rider.ride.chassis.cofg_world;
    assert!((0.0..300.0).contains(&at.x()), "put back at {at:?}, not past the seam inside the world");
}

// --- Bike against bike -------------------------------------------------------------------------

/// The bike's two collision files, off the split track's archive, and race125p's `[Collisions]`
/// (`tests/rider_contact.rs`'s).
fn bike_col() -> Option<(BikeCol, phy::Collisions)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL))).expect("the bike's collision files");
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let race = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy in a bike archive");
    let tables = game_sim::ride::Tables::build();
    Some((col, Phy::read_bytes(&race, &tables.curve).collisions))
}

/// A bike standing level at `x`, 40 feet up, at `z` 283, facing along `z`.
fn standing(x: f32) -> Mat4 {
    Mat4([Vf::new(1.0, 0.0, 0.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(x, 40.0, 283.0, 1.0)])
}

#[test]
fn two_bikes_a_seam_apart_strike_as_two_side_by_side() {
    let Some((col, _)) = bike_col() else { return };
    let wrapped = world(1, true);
    let near = |from: Vf, at: Vf| game_sim::nearest_copy(&wrapped, from, at);
    let seat = |x: f32| Seat { col: Some(&col), at: standing(x), ghost: false, riding: true };
    // Side by side over the near seam, a foot and a half apart (`tests/rider_contact.rs`'s `SIDE`):
    // one at 0.75, the other at -0.75 — which on the wrapped world is 1133.25.
    let beside = rider_contact::gather(&[seat(0.75), seat(-0.75)], DT);
    assert!(beside.iter().all(|l| !l.is_empty()), "the control pair does not touch: {:?}", beside.iter().map(Vec::len).collect::<Vec<_>>());
    let pair = [seat(0.75), seat(-0.75 + WIDTH)];
    assert!(rider_contact::gather(&pair, DT).iter().all(Vec::is_empty), "a width apart, without the seam, they touch");
    let seamed = rider_contact::gather_near(&pair, DT, &near);
    assert_eq!(seamed.iter().map(Vec::len).collect::<Vec<_>>(), beside.iter().map(Vec::len).collect::<Vec<_>>());
    // Bike 0, at 0.75, searches with bike 1's copy at 1133.25 less the width, -0.75 exactly: its own
    // contacts are the control's to the bit. What bike 1 found searching from 1133.25 is the same
    // within rounding — a float holds fewer bits there — on both lists: bike 1's own, a width over,
    // and the mirrors on bike 0's, brought back by bike 0's copy.
    let mut exact = 0;
    for (who, (s, b)) in seamed.iter().zip(&beside).enumerate() {
        for (k, (c, d)) in s.iter().zip(b).enumerate() {
            assert_eq!((c.kind, c.other), (d.kind, d.other), "bike {who}, contact {k}");
            if who == 0 && c.kind == 0 {
                exact += 1;
                assert_eq!(c.point.to_bits(), d.point.to_bits(), "bike {who}, contact {k}: the point");
                assert_eq!(c.normal.to_bits(), d.normal.to_bits(), "bike {who}, contact {k}: the normal");
                assert_eq!((c.from.to_bits(), c.on_body.to_bits()), (d.from.to_bits(), d.on_body.to_bits()), "bike {who}, contact {k}");
                continue;
            }
            let dx = if who == 1 { WIDTH } else { 0.0 };
            assert!(gap(d.point, c.point, dx) < 1e-3, "bike {who}, contact {k}: the point {:?} against {:?}", c.point, d.point);
            for (name, u, v) in [("normal", c.normal, d.normal), ("from", c.from, d.from), ("on body", c.on_body, d.on_body)] {
                assert!(gap(u, v, 0.0) < 1e-3, "bike {who}, contact {k}: {name} {u:?} against {v:?}");
            }
        }
    }
    assert!(exact > 0, "bike 0 struck nothing of its own");
}

/// Two riders on [`relief`], side by side a foot and a half apart at `x` and `x - 1.5` (folded into
/// the world), facing along `z`, wrecks off, with race125p's `[Collisions]` and, with `col`, the
/// bike's collision files (`tests/rider_contact.rs`'s pair).
fn pair(tables: &Tables, col: Option<&Arc<BikeCol>>, collisions: phy::Collisions, x: f32) -> Sim {
    let mut phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    phy.collisions = collisions;
    let facing = Vf::new(0.0, 0.0, 1.0, 0.0);
    let other_x = if x - 1.5 < 0.0 { x - 1.5 + WIDTH } else { x - 1.5 };
    let riders = [x, other_x].map(|x| {
        let mut r = rider(&phy, tables, x, 283.0, facing);
        r.sense.wrecks_enabled = false;
        if let Some(col) = col {
            r.carries_bike_col(Arc::clone(col));
        }
        r
    });
    let [one, two] = riders;
    let mut s = Sim::with_rider(0, Arc::new(Tables::build()), phy.clone(), one);
    s.add_player(phy, two);
    s
}

#[test]
fn two_riders_a_seam_apart_touch() {
    let Some((col, collisions)) = bike_col() else { return };
    let col = Arc::new(col);
    let tables = Tables::build();
    let mut wrapped = world(1, true);
    // Over the seam (0.75 and 1133.25), and the same pair 567 feet in, where the ground is the same.
    let mut a = pair(&tables, Some(&col), collisions, 0.75);
    let mut b = pair(&tables, Some(&col), collisions, 567.75);
    // And b's pair without the files, to see the pair push.
    let mut alone = pair(&tables, None, collisions, 567.75);
    let still = [Controls::default(); 2];
    let (mut worst, mut pushed) = (0.0f32, 0.0f32);
    for tick in 0..30 {
        let la = a.bike_contacts_in(DT, &wrapped).map(|l| l.iter().map(Vec::len).collect::<Vec<_>>());
        let lb = b.bike_contacts_in(DT, &wrapped).map(|l| l.iter().map(Vec::len).collect::<Vec<_>>());
        assert_eq!(la, lb, "tick {tick}: the seam pair's contacts are not the near pair's");
        if tick == 0 {
            assert!(la.as_ref().is_some_and(|l| l.iter().all(|&n| n > 0)), "the pair does not touch: {la:?}");
            let plain = a.bike_contacts(DT).unwrap();
            assert!(plain.iter().all(Vec::is_empty), "a width apart, without the seam, they touch");
        }
        a.step_all(DT, &still, &mut wrapped);
        b.step_all(DT, &still, &mut wrapped);
        alone.step_all(DT, &still, &mut wrapped);
        for i in 0..2 {
            let (p, q, r) = (a.rider_of(i).ride.chassis.cofg_world, b.rider_of(i).ride.chassis.cofg_world, alone.rider_of(i).ride.chassis.cofg_world);
            let dx = if p.x() > 567.0 { 567.0 - WIDTH } else { 567.0 };
            worst = worst.max(gap(p, q, dx));
            pushed = pushed.max(gap(q, r, 0.0));
        }
    }
    let spread = |s: &Sim| {
        let (p, q) = (s.rider_of(0).ride.chassis.cofg_world, s.rider_of(1).ride.chassis.cofg_world);
        let d = p.x() - q.x();
        if d < -567.0 { d + WIDTH } else { d }
    };
    assert!(worst < 0.01, "the seam pair parted from the near pair by {worst} ft");
    assert!(pushed > 0.25, "the near pair rode as each would alone (by {pushed} ft): nothing touched");
    assert!(spread(&a) > 1.5 + 0.25, "the seam pair did not come apart: {} ft", spread(&a));
}
