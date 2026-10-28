//! S159: P1 riding the truck in the user's truck tape (C12), against the port from the RAM image
//! before each tick.
//!
//! `context/recordings/split/truck.p2m2` is the user's two-player ride on free09 in which P1 rides
//! the flatbed truck (path car 3) from 132089 to 132894: on from behind at about 45 ft/s over the
//! road, a stop on the deck, a roll back with nothing held, a reverse and a ride forward, and off
//! the front. P2 stands at the start. C12 captured it (`context/capture/2p/truck.csv`, one row a
//! tick at the state's 60 Hz); S159 saved the original's RAM in the ticks around each event
//! (`context/states/s159/s159-<frame>.ram`, research/c10/play_at.sh and research/s159/rams.py).
//!
//! - [`the_images_are_the_truck_ride`]: each image holds its tick, P1's wheels name car 3 or nothing
//!   as the ride has it, and car 3 is a path car;
//! - [`the_truck_drives_from_each_image_into_the_next`]: the port's car step from an image lands on
//!   the next tick's image, bit for bit: the truck under the bike moves as the original's;
//! - [`the_carried_velocity_is_the_originals`]: on P1's real wheels, the port's carry (the car's
//!   motion over the tick and the contact-velocity step, S136) writes what the original's step
//!   writes, bit for bit, on the deck and off it;
//! - [`the_capture_rides_the_deck`]: the capture's ride on the deck — the wheels' car words, the bike
//!   on the deck from its landing until it leaves the front, and the roll back with nothing held
//!   (research/s159/deck_frame.py).
//!
//! The images, the capture and the disc are game data, git-ignored: absent, the tests skip.

use game_render::disc::Disc;
use game_sim::chassis::{self, Mat3};
use game_sim::math::Mat4;
use game_sim::ride;
use game_sim::traffic::{Car, Key, Moving, Spec};
use game_sim::track::{Scene, Terrain};
use game_sim::world::Ground;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;

/// The images S159 saved, each the tick it is named by: pairs a tick apart where a step is driven
/// from one into the next.
const IMAGES: [u32; 12] = [132087, 132088, 132168, 132169, 132229, 132230, 132289, 132499, 132500, 132891, 132892, 132893];

/// The game manager's slot and the frame counter (kit.toml's `@game_manager`, `@frame_counter`).
const GAME_MANAGER: u32 = 0x004e_1e98;
const FRAME_COUNTER: u32 = 0x0047_9d04;
const BUDGET: u64 = 50_000_000;

/// The ridden vehicle's chassis and its front and rear wheels (watch/2p.toml).
const CHASSIS: u32 = 0x7a4;
const WHEELS: [(&str, u32); 2] = [("front", 0x7b0), ("rear", 0x7b4)];

/// The path car's fields (`research/oracle.txt` S81, S136), as tests/traffic.rs reads them.
mod at {
    pub const NODE: u32 = 0x254;
    pub const PATH: u32 = 0x430;
    pub const LAST: u32 = 0x450;
    pub const HEAD: u32 = 0x480;
    pub const UP: u32 = 0x4a0;
    pub const FIRST: u32 = 0x4e4;
    pub const CURSOR: u32 = 0x4f4;
    pub const DIRTY: u32 = 0x4fc;
    pub const TANGENTS: u32 = 0x510;
    pub const TIME_SCALE: u32 = 0x534;
    pub const LAG: u32 = 0x558;
    pub const HEADING_FILTER: u32 = 0x55c;
    pub const NORMAL_FILTER: u32 = 0x560;
    pub const KEYS: u32 = 0x8;
    pub const CLOCK: u32 = 0xc;
    pub const KEY: u32 = 0x30;
    pub const WORLD: u32 = 0x80;
    pub const VTABLE: u32 = 0x8;
    pub const BEFORE: u32 = 0x2e0;
    pub const DT: u32 = 0x344;
    pub const INV_DT: u32 = 0x348;
}

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there", path.display()));
        return None;
    }
    Some(path)
}

/// S159's RAM image at `frame`, ready to call into.
fn image(frame: u32) -> Option<Ee> {
    let path = ctx(&format!("states/s159/s159-{frame}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

fn vf(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn mat(ee: &Ee, a: u32) -> Mat4 {
    Mat4([vf(ee, a), vf(ee, a + 16), vf(ee, a + 32), vf(ee, a + 48)])
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

fn mat_bits(m: &Mat4) -> [[u32; 4]; 4] {
    m.0.map(bits)
}

/// The application object, which keeps the frame's `dt` and `1/dt`.
fn app(ee: &Ee) -> u32 {
    ee.u32_at(addr("app_pointer").unwrap())
}

/// P1's ridden vehicle: the actor list's fourth entry (watch/2p.toml), a ridden vehicle (type 2) of
/// player 0.
fn p1(ee: &Ee) -> u32 {
    let v = [0x34, 0x34, 0x8, 0x8, 0x8, 0x0].iter().fold(ee.u32_at(GAME_MANAGER), |a, &h| ee.u32_at(a + h));
    assert_eq!(ee.u32_at(v), 2, "p1's list entry is not a ridden vehicle");
    assert_eq!(ee.u32_at(ee.u32_at(v + 0x20) + 0x30), 0, "the list's p1 is another player");
    v
}

/// The level's six path cars, in the capture's order: the cars whose world matrices its
/// `car<k>_xform` columns read (watch/2p.toml: a car's `+0x2a0`).
const CARS: [u32; 6] = [0x00fe_bf40, 0x01b7_fda0, 0x01b8_f300, 0x01b9_3ef0, 0x01ba_0a00, 0x01ba_5610];
/// Car 3, the flatbed P1 rides.
const TRUCK: usize = 3;

fn truck() -> u32 {
    CARS[TRUCK]
}

fn keys_of(ee: &Ee, car: u32) -> Vec<Key> {
    let path = ee.u32_at(car + at::PATH);
    let (n, table) = (ee.u32_at(path), ee.u32_at(path + at::KEYS));
    (0..n)
        .map(|k| table + k * at::KEY)
        .map(|k| Key { t: F(ee.f32_at(k)), len: F(ee.f32_at(k + 4)), pos: vf(ee, k + 0x10), normal: vf(ee, k + 0x20) })
        .collect()
}

/// A car's path and numbers, out of the image (tests/traffic.rs's `spec_of`, no mesh). Every car of
/// free09 is `PresampleTerrain` (S128): the image's keys are already on the ground, and their
/// lengths were measured on the clip's before, so they are the image's, not these keys'; the curve's
/// tangents are the ones it kept.
fn spec_of(ee: &Ee, car: u32) -> Spec {
    let keys = keys_of(ee, car);
    let points: Vec<(F, Vf)> = keys.iter().map(|k| (k.t, k.pos)).collect();
    let f = |o| F(ee.f32_at(car + o));
    let mut spec = Spec::new(&points, f(at::TIME_SCALE), f(at::HEADING_FILTER), f(at::NORMAL_FILTER), f(at::LAG), None).expect("a path");
    for (ours, theirs) in spec.keys.iter_mut().zip(&keys) {
        ours.len = theirs.len;
    }
    spec.measured = [vf(ee, car + at::TANGENTS), vf(ee, car + at::TANGENTS + 16)];
    spec.presampled(keys)
}

/// A car's state, out of the image (tests/traffic.rs's `car_of`).
fn car_of(ee: &Ee, car: u32) -> Car {
    Car {
        clock: F(ee.f32_at(ee.u32_at(car + at::PATH) + at::CLOCK)),
        seg: ee.u32_at(car + at::CURSOR),
        point: vf(ee, car + at::LAST),
        head: vf(ee, car + at::HEAD),
        normal: vf(ee, car + at::UP),
        first: ee.u32_at(car + at::FIRST) != 0,
        pose: mat(ee, ee.u32_at(car + at::NODE) + at::WORLD),
        // The tangents the curve kept, until its cursor moves.
        stale: ee.u32_at(car + at::DIRTY) == 0,
        level: Default::default(),
    }
}

/// free09_ss's ground (the 2p ride's world, as tests/two_p_world.rs loads it), for `then`.
fn on_free09_ss<R>(then: impl FnOnce(&mut Ground<'_>) -> R) -> Option<R> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    let read = |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"));
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&bytes).expect("parse the terrain");
    let mut ground = Ground::new(&terrain, &scene.terrain);
    Some(then(&mut ground))
}

/// Whether each of P1's wheels names car 3 in the image, front and rear.
fn on_car(ee: &Ee) -> [bool; 2] {
    let v = p1(ee);
    WHEELS.map(|(_, w)| ee.u32_at(ee.u32_at(v + w) + 0x1f4) == truck())
}

#[test]
fn the_images_are_the_truck_ride() {
    let mut seen = 0;
    for frame in IMAGES {
        let Some(ee) = image(frame) else { return };
        assert_eq!(ee.u32_at(FRAME_COUNTER), frame, "s159-{frame}.ram holds another tick");
        assert_eq!(ee.u32_at(truck() + at::VTABLE), addr("path_car_vtable").unwrap(), "{frame}: car 3 is not a path car");
        let on = on_car(&ee);
        for (k, (name, w)) in WHEELS.iter().enumerate() {
            let car = ee.u32_at(ee.u32_at(p1(&ee) + w) + 0x1f4);
            assert!(car == 0 || on[k], "{frame}: the {name} wheel names {car:#x}, not car 3 or nothing");
        }
        seen += on.iter().filter(|&&o| o).count();
    }
    assert!(seen > 0, "no image has a wheel on the truck");
}

/// The port's arc lengths of the cars' paths (`Spec::new`, S81), built by the shell from
/// free09_ss's `.toy` and clips, against the ones the image holds: measured on the clip's keys,
/// before the presample puts them on the ground (S128).
#[test]
fn the_paths_measure_as_the_originals() {
    let Some(ee) = image(IMAGES[0]) else { return };
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(why) => return skip(&why),
    };
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    let mut flat = |p: game_render::mat::V3| (p, game_render::mat::v3(0.0, 1.0, 0.0));
    let specs = game_render::toy::Traffic::load(&parts, "free09_ss", &mut flat).sim(&parts).specs.as_ref().clone();
    let (mut wrong, mut checked) = (Vec::new(), 0);
    for (c, &car) in CARS.iter().enumerate() {
        let theirs = keys_of(&ee, car);
        let f = |o| ee.u32_at(car + o);
        let spec = specs
            .iter()
            .find(|s| {
                (s.lag.to_bits(), s.time_scale.to_bits()) == (f(at::LAG), f(at::TIME_SCALE))
                    && s.keys.len() == theirs.len()
                    && s.keys.iter().zip(&theirs).all(|(a, b)| a.t.to_bits() == b.t.to_bits() && a.pos.x().to_bits() == b.pos.x().to_bits() && a.pos.z().to_bits() == b.pos.z().to_bits())
            })
            .unwrap_or_else(|| panic!("car {c}: no car of free09_ss's .toy drives this path"));
        assert!(spec.presample, "car {c}: the shell's spec does not presample");
        for (k, (ours, theirs)) in spec.keys.iter().zip(&theirs).enumerate() {
            checked += 1;
            if ours.len.to_bits() != theirs.len.to_bits() {
                wrong.push(format!("car {c} key {k}: {} against {}", ours.len.0, theirs.len.0));
            }
        }
    }
    assert!(wrong.is_empty(), "{} keys differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(checked > 100, "only {checked} keys");
}

/// The port's car steps (`Spec::step`, S81) from each image, every car in the level's order as
/// [`Traffic::frame`](game_sim::traffic::Traffic::frame) steps them, at the next tick's `dt`, against
/// the next tick's image: the clock, the cursor, the point and frames, and the world matrix the
/// wheel's carry reads: the presampled key normals and the lag's own walk (S128).
#[test]
fn the_truck_drives_from_each_image_into_the_next() {
    let (mut pairs, mut wrong) = (0, Vec::new());
    for w in IMAGES.windows(2) {
        let (a, b) = (w[0], w[1]);
        if b != a + 1 {
            continue;
        }
        let (Some(ea), Some(eb)) = (image(a), image(b)) else { return };
        let specs: Vec<Spec> = CARS.iter().map(|&car| spec_of(&ea, car)).collect();
        let mut ours: Vec<Car> = CARS.iter().map(|&car| car_of(&ea, car)).collect();
        let dt = F(eb.f32_at(app(&eb) + at::DT));
        let Some(()) = on_free09_ss(|ground| specs.iter().zip(&mut ours).for_each(|(spec, car)| spec.step(car, dt, ground))) else { return };
        for (k, &car) in CARS.iter().enumerate() {
            let (ours, theirs) = (&ours[k], car_of(&eb, car));
            let fields = [
                ("clock", ours.clock.to_bits() == theirs.clock.to_bits()),
                ("cursor", ours.seg == theirs.seg),
                ("point", bits(ours.point) == bits(theirs.point)),
                ("heading", bits(ours.head) == bits(theirs.head)),
                ("up", bits(ours.normal) == bits(theirs.normal)),
                ("world matrix", mat_bits(&ours.pose) == mat_bits(&theirs.pose)),
                // The pose the carry reads as the car's last: the one this image left.
                ("previous matrix", mat_bits(&mat(&eb, car + at::BEFORE)) == mat_bits(&car_of(&ea, car).pose)),
            ];
            for (field, same) in fields {
                if !same {
                    wrong.push(format!("{a} -> {b}: car {k} {field}: point {:?} head {:?} up {:?} against point {:?} head {:?} up {:?}", ours.point, ours.head, ours.normal, theirs.point, theirs.head, theirs.normal));
                }
            }
        }
        pairs += 1;
    }
    assert!(wrong.is_empty(), "{} of the cars' fields differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(pairs >= 5, "only {pairs} image pairs a tick apart");
}

/// What the original's contact-velocity step writes on one of P1's wheels in the image, run again
/// on the image's own wheel, chassis and car, and what the port makes of the same: the car's motion
/// from its matrix before its step and its node's now, at the application's `1/dt`
/// ([`Moving::of`]), measured at the wheel's contact ([`Moving::at`]), and the step itself
/// ([`ride::contact_velocity`], [`ride::wheel_contact_velocity`]).
#[test]
fn the_carried_velocity_is_the_originals() {
    let step = addr("wheel_contact_velocity").unwrap();
    let (mut wheels, mut carried) = (0, 0);
    for frame in IMAGES {
        let Some(mut ee) = image(frame) else { return };
        let v = p1(&ee);
        let chassis_at = ee.u32_at(v + CHASSIS);
        let inv_dt = F(ee.f32_at(app(&ee) + at::INV_DT));
        for (name, w) in WHEELS {
            let wheel = ee.u32_at(v + w);
            let car = ee.u32_at(wheel + 0x1f4);
            // The step's inputs, as the image holds them: the run below overwrites only its outputs.
            let (contact, load) = (vf(&ee, wheel + 0x120), vf(&ee, wheel + 0x140));
            let (cofg, vel, spin) = (vf(&ee, chassis_at + 0x60), vf(&ee, chassis_at + 0x260), vf(&ee, chassis_at + 0x280));
            let hung = mat(&ee, ee.u32_at(chassis_at + at::NODE) + at::WORLD);
            let under = (car != 0 && ee.u32_at(wheel + 0x1f0) == 0).then(|| {
                let node = ee.u32_at(car + at::NODE);
                Moving::of(&mat(&ee, car + at::BEFORE), &mat(&ee, node + at::WORLD), inv_dt).at(contact)
            });
            ee.set_u32(wheel + 0x1a0, 0);
            ee.call(step, &[u64::from(wheel)], &[], BUDGET);

            let own = ride::contact_velocity(vel, spin, load, cofg);
            let (contact_vel, chassis_vel) = ride::wheel_contact_velocity(own, vel, under);
            let local = chassis::unrotate(&Mat3::new(hung.0[0], hung.0[1], hung.0[2]), chassis_vel);
            let what = |field: &str| format!("{frame}: P1's {name} wheel {}: {field}", if under.is_some() { "on car 3" } else { "off it" });
            if let Some((theirs_v, theirs_p)) = under {
                assert_eq!(bits(vf(&ee, wheel + 0x1b0)), bits(theirs_v), "{}", what("+0x1b0 the car's velocity"));
                assert_eq!(bits(vf(&ee, wheel + 0x1c0)), bits(theirs_p), "{}", what("+0x1c0 the car's velocity at the contact"));
                carried += 1;
            }
            assert_eq!(bits(vf(&ee, wheel + 0x170)), bits(contact_vel), "{}", what("+0x170 the contact's velocity"));
            assert_eq!(bits(vf(&ee, wheel + 0x180)), bits(chassis_vel), "{}", what("+0x180 the chassis' velocity"));
            assert_eq!(bits(vf(&ee, wheel + 0x190)), bits(local), "{}", what("+0x190 the chassis' velocity, its own way"));
            wheels += 1;
        }
    }
    assert!(carried > 0 && wheels > carried, "{carried} of {wheels} wheels on the truck");
}

/// The truck capture (C12), the columns the deck needs.
const CAPTURE: &str = "capture/2p/truck.csv";

/// A capture row: its frame, then each wanted column's value.
struct Rows {
    frames: Vec<u64>,
    cols: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Rows {
    fn at(&self, row: usize, col: &str) -> f32 {
        let k = self.cols.iter().position(|c| c == col).unwrap_or_else(|| panic!("no column {col}"));
        self.rows[row][k].parse().unwrap_or_else(|_| panic!("{col} at row {row}: {}", self.rows[row][k]))
    }

    fn word(&self, row: usize, col: &str) -> u32 {
        let k = self.cols.iter().position(|c| c == col).unwrap_or_else(|| panic!("no column {col}"));
        self.rows[row][k].parse().unwrap_or_else(|_| panic!("{col} at row {row}: {}", self.rows[row][k]))
    }

    fn row(&self, frame: u64) -> usize {
        self.frames.iter().position(|&f| f == frame).unwrap_or_else(|| panic!("no row {frame}"))
    }

    /// P1's chassis centre and velocity over the deck in car 3's own frame (`car3_xform`'s rows; the
    /// car's velocity off its position and the one before, at the row's `dt`).
    fn on_deck(&self, row: usize) -> ([f32; 3], [f32; 3]) {
        let m = |i: usize, j: usize| self.at(row, &format!("car3_xform.m{i}{j}"));
        let inv = 1.0 / self.at(row, "dt");
        let mut p = [0.0; 3];
        let mut v = [0.0; 3];
        for (k, c) in ["x", "y", "z"].iter().enumerate() {
            p[k] = self.at(row, &format!("p1_ch_centre.{c}")) - m(3, k);
            v[k] = self.at(row, &format!("p1_ch_vel.{c}")) - (m(3, k) - self.at(row, &format!("car3_prev.{c}"))) * inv;
        }
        let turn = |d: [f32; 3]| [0, 1, 2].map(|i| (0..3).map(|k| m(i, k) * d[k]).sum::<f32>());
        (turn(p), turn(v))
    }
}

fn capture() -> Option<Rows> {
    let path = ctx(CAPTURE)?;
    let file = std::io::BufReader::new(std::fs::File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let keep = |c: &str| {
        c == "dt"
            || c.starts_with("car3_")
            || c.starts_with("ctl0")
            || ["p1_wf_car", "p1_wr_car", "p2_wf_car", "p2_wr_car", "p1_on_ground"].contains(&c)
            || c.starts_with("p1_ch_centre.")
            || c.starts_with("p1_ch_vel.")
    };
    let cap = game_sim::replay::two_p::read(file, keep).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    Some(Rows { frames: cap.frames.clone(), cols: cap.columns.clone(), rows: cap.rows })
}

/// The ride on the deck as the capture has it. The wheels' car words name car 3 from 132089 to
/// 132894 and nothing else, P2's never; read at the capture's NominalScalar 0.05 they read 0 on
/// about two rows in three (the row's read phase against the wheel step, C12's deck.py: the film
/// has the bike on the deck throughout). The bike lands on the deck at 132164 and is on the ground
/// on every row until it leaves the front; with nothing held from 132240 to 132329 but P1's control 7
/// for 132267..132273 it rolls back over the deck, nearly 3 ft/s, and stays on it.
#[test]
fn the_capture_rides_the_deck() {
    let Some(cap) = capture() else { return };
    assert_eq!(cap.frames.len(), 1801, "the truck capture's rows");
    let named: Vec<u64> = (0..cap.frames.len())
        .filter(|&r| {
            let (f, b) = (cap.word(r, "p1_wf_car"), cap.word(r, "p1_wr_car"));
            for w in [f, b] {
                assert!(w == 0 || w == truck(), "{}: P1's wheel names {w:#x}", cap.frames[r]);
            }
            assert_eq!((cap.word(r, "p2_wf_car"), cap.word(r, "p2_wr_car")), (0, 0), "{}: P2 on a car", cap.frames[r]);
            f != 0 || b != 0
        })
        .map(|r| cap.frames[r])
        .collect();
    assert_eq!((named.first(), named.last(), named.len()), (Some(&132089), Some(&132894), 237), "the rows a wheel names car 3");

    let (landed, left) = (cap.row(132164), cap.row(132893));
    assert_eq!(cap.word(landed - 1, "p1_on_ground"), 0, "in the air before the deck");
    for r in landed..=left {
        assert_eq!(cap.word(r, "p1_on_ground"), 1, "{}: off the ground on the deck", cap.frames[r]);
        let (at, _) = cap.on_deck(r);
        assert!(at[0].abs() < 6.0 && at[2].abs() < 33.0, "{}: off the deck at {at:?}", cap.frames[r]);
    }

    let (from, to) = (cap.row(132240), cap.row(132329));
    let held = |r: usize| (0..16).filter(|k| cap.word(r, &format!("ctl0{k}.held")) != 0).collect::<Vec<_>>();
    for r in from..to {
        let want: &[usize] = if (132267..=132273).contains(&cap.frames[r]) { &[7] } else { &[] };
        assert_eq!(held(r), want, "{}: held on the roll", cap.frames[r]);
    }
    let slowest = (from..to).map(|r| cap.on_deck(r).1[2]).fold(f32::MAX, f32::min);
    let back = cap.on_deck(from).0[2] - cap.on_deck(to).0[2];
    assert!((-3.2..-2.6).contains(&slowest), "rolls back at {slowest} ft/s");
    assert!((2.5..4.0).contains(&back), "rolls back {back} ft");
}

/// What [`roll`] measured, along the deck in car 3's own frame (`+z` the way it drives).
#[derive(Debug)]
struct Roll {
    /// The bike's velocity along the deck over the car's, per tick.
    along: Vec<f32>,
    /// How far back along the deck the bike went in all, and up or down off it.
    back: f32,
    up: f32,
    /// Whether it stayed on the deck all the way.
    on: bool,
    /// The car's forward row's y at the end: the sine of the deck's climb.
    pitch: f32,
}

/// The ticks the roll back runs with nothing held in the original: 132231..132266, before P1's
/// control 7 at 132267.
const ROLL: usize = 36;

/// **P1's bike parked on the port's truck from the 132230 image**, nothing held, for [`ROLL`]
/// ticks at the image's `dt`: P1's own `race500p.phy`, set down where the image's chassis node
/// stands, at its velocity, on free09_ss with its props and car 3 driven by the port from the
/// image (the cars first, then the rider, as `Sim::step` orders them). `drag` keeps the bike's air
/// drag, which reads its speed over the world.
fn roll(drag: bool) -> Option<Roll> {
    use game_sim::rider::Tables;
    use game_sim::traffic::Traffic;
    use game_sim::world::{Place, Props};
    use std::sync::Arc;

    let ee = image(132230)?;
    let car = truck();
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    let read = |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"));
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&bytes).expect("parse the terrain");
    let (props, _) = Props::load(&scene, read);
    let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props };
    let name: Vec<u8> = (0..64).map(|k| ee.u8_at(car + 0x320 + k)).take_while(|&b| b != 0).collect();
    let col = String::from_utf8_lossy(&name).to_ascii_lowercase().replace(".psg", ".col");
    let mesh = game_sim::track::Mesh::parse(&read(&col).unwrap_or_else(|e| panic!("{e}"))).unwrap_or_else(|e| panic!("{col}: {e}"));
    let mut spec = spec_of(&ee, car);
    spec.mesh = Some(mesh);
    let mut traffic = Traffic::new(vec![spec]);
    traffic.cars = vec![car_of(&ee, car)];
    traffic.attach(&mut world.props);

    let tables = Arc::new(Tables::build());
    let phy_bytes = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| n.to_ascii_lowercase().starts_with("bike\\") && n.to_ascii_lowercase().ends_with(".res"))
        .collect::<Vec<_>>()
        .iter()
        .find_map(|b| disc.res(b).ok()?.read_suffix("race500p.phy").ok())
        .expect("race500p.phy in a bike archive");
    let mut phy = game_sim::phy::Phy::read_bytes(&phy_bytes, &tables.ride.curve);
    if !drag {
        phy.mv.drag = F::ZERO;
    }
    let chassis_at = ee.u32_at(p1(&ee) + CHASSIS);
    let node = mat(&ee, ee.u32_at(chassis_at + at::NODE) + at::WORLD);
    let mut sim = game_sim::Sim::ride(0x5159, tables, phy, node.0[3], node.0[2], Default::default());
    sim.rider.ride.place_on(node.0[3], node.0[2], node.0[1]);
    let v = vf(&ee, chassis_at + 0x260);
    sim.rider.ride.chassis.vel = Vf::new(v.x(), v.y(), v.z(), 0.0);

    let dt = F(ee.f32_at(app(&ee) + at::DT));
    let off = |sim: &game_sim::Sim, car: &Mat4| {
        let p = sim.rider.ride.position();
        let d = Vf::new(p.x() - car.0[3].x(), p.y() - car.0[3].y(), p.z() - car.0[3].z(), 0.0);
        chassis::unrotate(&Mat3::new(car.0[0], car.0[1], car.0[2]), d)
    };
    // The car's pose is only the stepped one after a frame: measure from the first tick on.
    traffic.frame(dt, &mut world);
    sim.step(dt, &ride::Controls::default(), &mut world);
    let start = off(&sim, &traffic.cars[0].pose);
    let (mut along, mut up) = (Vec::new(), 0.0f32);
    let mut last = start;
    let mut on = true;
    for _ in 1..ROLL {
        traffic.frame(dt, &mut world);
        sim.step(dt, &ride::Controls::default(), &mut world);
        let now = off(&sim, &traffic.cars[0].pose);
        along.push((now.z() - last.z()) / dt.0);
        up = up.max((now.y() - start.y()).abs());
        on &= now.x().abs() < 6.0 && now.z().abs() < 33.0 && now.y() > -1.0;
        last = now;
    }
    Some(Roll { along, back: start.z() - last.z(), up, on, pitch: traffic.cars[0].pose.0[2].y() })
}

/// The port keeps P1 on the deck with nothing held, drag or no drag, as the original does
/// ([`the_capture_rides_the_deck`]): S146's guess was a stock bike sliding off a wagon braked, and
/// on P1's own bike from the image the port's drag slip stays well under a foot here. How fast the
/// bike rolls back is not the original's (S182): it gets 0.65 ft back by 132266, the port ~0.2.
#[test]
fn the_port_keeps_p1_on_the_deck() {
    for drag in [true, false] {
        let Some(r) = roll(drag) else { return };
        assert!(r.on, "drag {drag}: the bike left the deck");
        assert!(r.back.abs() < 1.0 && r.up < 0.5, "drag {drag}: {:.3} ft back, {:.3} ft up or down", r.back, r.up);
    }
}

#[test]
#[ignore = "prints the port's roll back on the truck beside the original's"]
fn print_the_roll() {
    for drag in [true, false] {
        let Some(r) = roll(drag) else { return };
        println!("drag {drag}: back {:.3} ft, up {:.3} ft, on {}, pitch {:.4}", r.back, r.up, r.on, r.pitch);
        println!("{}", r.along.iter().map(|v| format!("{v:.2}")).collect::<Vec<_>>().join(" "));
    }
}
