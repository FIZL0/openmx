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
//!   (research/s159/deck_frame.py);
//! - [`the_roll_back_is_the_originals`]: P1's rider seeded off the 132230 image, on the capture's
//!   pads and frame times, rolls back down the deck as the original does for 59 ticks, bit for bit
//!   (S182).
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

/// S181: the same drive with nothing of the path out of the image: the shell's specs of free09_ss's
/// cars (measured on the clips' keys, their stale tangents the measure's), presampled by the port on
/// free09_ss's ground. Every presampled key's position and normal is the image's, and every car
/// steps from each image into the next as the original's does.
#[test]
fn the_shells_presampled_paths_drive_as_the_originals() {
    let Some(first) = image(IMAGES[0]) else { return };
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(why) => return skip(&why),
    };
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    let mut flat = |p: game_render::mat::V3| (p, game_render::mat::v3(0.0, 1.0, 0.0));
    let shell = game_render::toy::Traffic::load(&parts, "free09_ss", &mut flat).sim(&parts).specs.as_ref().clone();
    let specs: Vec<Spec> = CARS
        .iter()
        .enumerate()
        .map(|(c, &car)| {
            let theirs = keys_of(&first, car);
            let f = |o| first.u32_at(car + o);
            shell
                .iter()
                .find(|s| {
                    (s.lag.to_bits(), s.time_scale.to_bits()) == (f(at::LAG), f(at::TIME_SCALE))
                        && s.keys.len() == theirs.len()
                        && s.keys.iter().zip(&theirs).all(|(a, b)| a.t.to_bits() == b.t.to_bits() && a.pos.x().to_bits() == b.pos.x().to_bits() && a.pos.z().to_bits() == b.pos.z().to_bits())
                })
                .unwrap_or_else(|| panic!("car {c}: no car of free09_ss's .toy drives this path"))
                .clone()
        })
        .collect();
    let (mut pairs, mut keys, mut wrong) = (0, 0, Vec::new());
    let Some(()) = on_free09_ss(|ground| {
        for (c, (spec, &car)) in specs.iter().zip(&CARS).enumerate() {
            assert!(spec.presample && !spec.level, "car {c}: the shell's spec has the wrong switches");
            spec.presample(ground);
            for (k, (ours, theirs)) in spec.keys().iter().zip(&keys_of(&first, car)).enumerate() {
                keys += 1;
                if (bits(ours.pos), bits(ours.normal), ours.len.to_bits()) != (bits(theirs.pos), bits(theirs.normal), theirs.len.to_bits()) {
                    wrong.push(format!("car {c} key {k}: {ours:?} against {theirs:?}"));
                }
            }
        }
        for w in IMAGES.windows(2) {
            let (a, b) = (w[0], w[1]);
            if b != a + 1 {
                continue;
            }
            let (Some(ea), Some(eb)) = (image(a), image(b)) else { return };
            let dt = F(eb.f32_at(app(&eb) + at::DT));
            for (k, (spec, &car)) in specs.iter().zip(&CARS).enumerate() {
                // The image's clean tangents are the ones its curve made on the presampled keys of
                // the segment it is on, which the port makes again: only a cursor still on the
                // segment the measuring left it on rides the measure's.
                let kept = |mut c: Car| {
                    c.stale &= c.seg == spec.cursor;
                    c
                };
                let mut ours = kept(car_of(&ea, car));
                spec.step(&mut ours, dt, ground);
                let theirs = kept(car_of(&eb, car));
                let same = |c: &Car| (c.clock.to_bits(), c.seg, bits(c.point), bits(c.head), bits(c.normal), mat_bits(&c.pose), c.stale);
                if same(&ours) != same(&theirs) {
                    wrong.push(format!("{a} -> {b}: car {k}: {ours:?} against {theirs:?}"));
                }
            }
            pairs += 1;
        }
    }) else {
        return;
    };
    assert!(wrong.is_empty(), "{} keys and steps differ:\n{}", wrong.len(), wrong.join("\n"));
    assert!(keys > 100, "only {keys} keys");
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
            || c == "pad0_a"
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
    /// The bike's velocity along the deck over the car's, per tick of the [`ROLL`].
    along: Vec<f32>,
    /// How far back along the deck the bike went over the [`ROLL`], and up or down off it.
    back: f32,
    up: f32,
    /// Whether it stayed on the deck all the way.
    on: bool,
    /// The car's forward row's y at the end: the sine of the deck's climb.
    pitch: f32,
    /// The chassis' velocity and centre after each tick from [`FROM`], in the world.
    vel: Vec<Vf>,
    centre: Vec<Vf>,
    /// The chassis' velocity, centre and orientation after the last tick, before [`TO`].
    end: [Vf; 3],
}

/// The ticks the roll back runs with nothing held in the original: 132231..132266, before P1's
/// control 7 at 132267.
const ROLL: usize = 36;

/// The image the roll starts from, and the one it is driven to: the tick before [`TO`] is the last
/// stepped.
const FROM: u32 = 132230;
const TO: u32 = 132289;

/// One rider's words off an image, at the offsets `tests/replay.rs`'s save-state seeders read them
/// (S174's, tests/two_p_rider.rs).
struct Image<'a> {
    ee: &'a Ee,
    rider: u32,
}

impl Image<'_> {
    fn word(&self, a: u32) -> u32 {
        self.ee.u32_at(a)
    }
    fn float(&self, a: u32) -> F {
        F(self.ee.f32_at(a))
    }
    fn flag(&self, a: u32) -> bool {
        self.word(a) != 0
    }
    fn vf(&self, a: u32) -> Vf {
        Vf([0, 4, 8, 12].map(|k| self.ee.f32_at(a + k)))
    }
    fn at(&self, off: u32) -> u32 {
        self.word(self.rider + off)
    }

    /// The wheel mounts the bike was built with (each wheel's `+0xa0`), front and rear.
    fn mounts(&self) -> (Vf, Vf) {
        (self.vf(self.at(0x7b0) + 0xa0), self.vf(self.at(0x7b4) + 0xa0))
    }

    /// The chassis' integrator state and node (replay.rs's `seed_chassis`).
    fn chassis(&self, ride: &mut game_sim::ride::Ride) {
        let c = self.at(0x7a4);
        let node = self.word(c + 0x254);
        let ch = &mut ride.chassis;
        ch.speed = self.float(c + 0x34);
        ch.cofg_world = self.vf(c + 0x60);
        ch.accel = self.vf(c + 0x70);
        ch.ang_accel = self.vf(c + 0x80);
        ch.euler = self.vf(c + 0x90);
        ch.force = self.vf(c + 0x130);
        ch.torque = self.vf(c + 0x140);
        ch.impulse = self.vf(c + 0x150);
        ch.correction = self.vf(c + 0x160);
        ch.cofg = self.vf(c + 0x190);
        ch.quat = self.vf(c + 0x1a0);
        ch.awake = self.word(c + 0x1b4) as i32;
        ch.contacts = self.word(c + 0x1e0) as i32;
        ch.vel = self.vf(c + 0x260);
        ch.local_vel = self.vf(c + 0x270);
        ch.world_ang_vel = self.vf(c + 0x280);
        ch.ang_vel = self.vf(c + 0x290);
        for k in 0..3 {
            ch.free[k] = self.word(c + 0x2a0 + 4 * k as u32) as i32;
            ch.ang_free[k] = self.word(c + 0x2ac + 4 * k as u32) as i32;
        }
        let three = |at: u32| [self.float(at), self.float(at + 4), self.float(at + 8)];
        ch.pos_hist.dx = three(c + 0x2b8);
        ch.pos_hist.prev = three(c + 0x2c4);
        ch.vel_hist.dx = three(c + 0x2d0);
        ch.vel_hist.prev = three(c + 0x2dc);
        for k in 0..4 {
            ch.quat_hist.dx[k] = self.float(c + 0x2f0 + 4 * k as u32);
            ch.quat_hist.prev[k] = self.float(c + 0x300 + 4 * k as u32);
        }
        ch.ang_hist.dx = three(c + 0x310);
        ch.ang_hist.prev = three(c + 0x31c);
        ride.node = [self.vf(node + 0x80), self.vf(node + 0x90), self.vf(node + 0xa0)];
        ride.origin = self.vf(node + 0xb0);
    }

    /// One wheel's whole state (replay.rs's `seed_wheel`).
    fn wheel(&self, which: usize, wh: &mut game_sim::ride::Wheel) {
        let a = self.at([0x7b0, 0x7b4][which]);
        let s = &mut wh.w;
        s.mount_world = self.vf(a + 0x150);
        s.probe_end = self.vf(a + 0x100);
        s.probe_end_world = self.vf(a + 0x160);
        s.ground_normal = self.vf(a + 0x200);
        s.ray_hit = self.word(a + 0xd0) as i32;
        s.swept_contact = self.word(a + 0xd4) as i32;
        s.terrain_wins = self.word(a + 0xd8) as i32;
        s.contact = self.flag(a + 0xdc);
        s.ray_steps = self.word(a + 0xec) as i32;
        s.compression = self.float(a + 0xf0);
        s.last_compression = self.float(a + 0xf4);
        s.spring_force = self.float(a + 0xf8);
        s.contact_point = self.vf(a + 0x120);
        s.contact_normal = self.vf(a + 0x130);
        s.load_point = self.vf(a + 0x140);
        s.contact_velocity = self.vf(a + 0x170);
        s.chassis_velocity = self.vf(a + 0x180);
        s.balancing = self.flag(a + 0x3c);
        s.steer_torque = self.float(a + 0x6c);
        s.steer_angle = self.float(a + 0x8ac);
        s.steer_rate = self.float(a + 0x8b0);
        let m = &mut wh.mv;
        m.spin = self.float(a + 0x1d4);
        m.contact = s.contact;
        m.contact_normal = s.contact_normal;
        m.drive_torque = self.float(a + 0x210);
        m.balancing = s.balancing;
        for (k, o) in [0xd0, 0xd4, 0x1f0, 0x1f4, 0xf0, 0xf8, 0x1a0].into_iter().enumerate() {
            m.fresh.scalars[k] = self.word(a + o);
        }
        for (k, o) in [0x170, 0x1b0, 0x1c0].into_iter().enumerate() {
            m.fresh.vectors[k] = self.vf(a + o);
        }
        wh.surface = self.word(a + 0x110) as u8;
        wh.angle = self.float(a + 0x1d0);
        wh.chassis_local_velocity = self.vf(a + 0x190);
        wh.local_velocity = self.vf(a + 0x840);
        wh.local_force = self.vf(a + 0x850);
        for r in 0..4 {
            wh.frame[r] = self.vf(a + 0x8d0 + 16 * r as u32);
        }
        wh.side_per_load = self.float(a + 0x234);
        wh.steer_axis = self.vf(a + 0x8c0);
        wh.slip = game_sim::tire::Slip { spinning: self.flag(a + 0xe0), locked: self.flag(a + 0xe4), sliding: self.flag(a + 0xe8) };
        wh.mv.spin_from_torque = wh.slip.spinning;
    }

    /// The engine, the clutch, the balance, the air state and the rate-limited levers (replay.rs's
    /// `seed_engine_outputs`, `drivetrain`, `seed_balance`, `seed_air`, `seed_held`): the clutch's
    /// launch ramp too (its timer, start scale and centre-of-gravity weight).
    fn rest(&self, ride: &mut game_sim::ride::Ride) {
        let (r, e) = (self.rider, self.at(0x7b8));
        ride.engine.torque = self.float(e + 0xa0);
        ride.engine.wheel_torque = self.float(e + 0xa4);
        ride.engine.road_rpm = self.float(e + 0xac);
        ride.mv.clutch_timer = self.float(r + 0x6c0);
        ride.mv.clutch_scale_start = self.float(r + 0x6c4);
        ride.mv.clutch_cofg_weight = self.float(r + 0x6cc);
        ride.held.gas = self.float(r + 0x658);
        ride.held.clutch = self.float(r + 0x65c);
        ride.mv.clutch_in = ride.held.clutch;
        game_sim::r#move::set_gear(&mut ride.engine, self.word(e + 0x68) as i32);
        ride.engine.clutch_locked = self.flag(e + 0x64);
        ride.engine.clutch_torque_scale = self.float(e + 0x88);
        ride.engine.curve = game_sim::r#move::Curve::at(self.word(e + 0x8c), self.word(e + 0x90), self.float(e + 0x94));
        ride.engine.rpm = self.float(e + 0xa8);
        let mv = &mut ride.mv;
        mv.g_peak = self.vf(r + 0x550);
        mv.balance_inertia = self.float(r + 0xa4c);
        mv.balance_command = self.float(r + 0xa60);
        mv.balance_point = self.float(r + 0xa64);
        mv.balance_noise_time = self.float(r + 0xa6c);
        mv.pitch_integral = self.float(r + 0x724);
        mv.stoppie = self.flag(r + 0x728);
        let air = &mut ride.air;
        air.on_ground = self.flag(r + 0x574);
        air.both_wheels = self.flag(r + 0x578);
        air.airborne = self.flag(r + 0x57c);
        air.was_airborne = self.flag(r + 0x580);
        air.hang_time = self.float(r + 0x4f0);
        air.contact_vel = self.vf(r + 0x430);
        air.crossup = self.float(r + 0x600);
        let h = &mut ride.held;
        h.stick = (self.float(r + 0x628), self.float(r + 0x62c));
        h.flick = self.float(r + 0x630);
        h.front_brake = self.float(r + 0x650);
        h.brake = h.front_brake;
        h.rear_brake = self.float(r + 0x654);
        h.lean = (self.float(r + 0x670), self.float(r + 0x674));
    }
}

/// P1's rider off `ee` (S182): its own mounts, chassis, wheels, drivetrain, balance, air and levers,
/// as S174 seeds a 2p rider. A rider set down fresh instead has its clutch's launch ramp unspent
/// (the image's is long over: 1.5 s on its timer), and with the lever out the ramp shifts the bike's
/// centre of gravity back for the whole roll and holds it to the deck.
fn seeded_rider(ee: &Ee, phy: &game_sim::phy::Phy) -> game_sim::rider::Rider {
    let im = Image { ee, rider: p1(ee) };
    let c = im.at(CHASSIS);
    let mut rider = game_sim::rider::Rider::new(phy, im.mounts(), im.vf(c + 0x60), Vf::new(0.0, 0.0, 1.0, 0.0), Default::default());
    rider.no_start_line();
    im.chassis(&mut rider.ride);
    im.wheel(0, &mut rider.ride.front);
    im.wheel(1, &mut rider.ride.rear);
    im.rest(&mut rider.ride);
    rider
}

/// P1's pad on capture row `row` (`pad0_a`, the report's bytes in hex).
fn pad(cap: &Rows, row: usize) -> game_sim::input::Report {
    let k = cap.cols.iter().position(|c| c == "pad0_a").expect("no column pad0_a");
    let hex = cap.rows[row][k].as_bytes();
    let mut b = [0u8; game_sim::input::REPORT_BYTES];
    for (i, pair) in hex.chunks(2).enumerate() {
        b[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap_or_else(|e| panic!("pad0_a at row {row}: {e}"));
    }
    game_sim::input::Report::from_bytes(b)
}

/// **P1's bike on the port's truck from the [`FROM`] image to [`TO`]**: P1's own `race500p.phy`
/// seeded off the image ([`seeded_rider`]), on free09_ss with its props and car 3 driven by the port
/// from the image (the cars first, then the rider, as `Sim::step` orders them). Each tick `t` polls
/// P1's pad off capture row `t` and steps at row `t + 1`'s `dt`, the 2p state's alternating
/// 1/60-ish frame times; P2's pad is missing. `drag` keeps the bike's air drag, which reads its
/// speed over the world.
fn roll(drag: bool) -> Option<Roll> {
    use game_sim::input::Report;
    use game_sim::rider::Tables;
    use game_sim::traffic::Traffic;
    use game_sim::world::{Place, Props};
    use std::sync::Arc;

    let ee = image(FROM)?;
    let cap = capture()?;
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
    let rider = seeded_rider(&ee, &phy);
    let mut sim = game_sim::Sim::with_rider(0x5159, tables, phy, rider);

    let off = |sim: &game_sim::Sim, car: &Mat4| {
        let p = sim.rider.ride.chassis.cofg_world;
        let d = Vf::new(p.x() - car.0[3].x(), p.y() - car.0[3].y(), p.z() - car.0[3].z(), 0.0);
        chassis::unrotate(&Mat3::new(car.0[0], car.0[1], car.0[2]), d)
    };
    // The pad the image's tick read, so the first tick's presses are not edges.
    let dt_of = |t: u32| F(cap.at(cap.row(u64::from(t) + 1), "dt"));
    sim.poll(dt_of(FROM - 1), &[pad(&cap, cap.row(u64::from(FROM) - 1)), Report::missing()]);
    let (mut along, mut up, mut on) = (Vec::new(), 0.0f32, true);
    let (mut vel, mut centre) = (Vec::new(), Vec::new());
    let (mut start, mut last) = (Vf::ZERO, Vf::ZERO);
    for t in FROM..TO {
        let dt = dt_of(t);
        traffic.frame(dt, &mut world);
        let controls = sim.poll(dt, &[pad(&cap, cap.row(u64::from(t))), Report::missing()]);
        sim.step(dt, &controls, &mut world);
        vel.push(sim.rider.ride.chassis.vel);
        centre.push(sim.rider.ride.chassis.cofg_world);
        // The car's pose is only the stepped one after a frame: measure from the first tick on.
        let now = off(&sim, &traffic.cars[0].pose);
        let k = (t - FROM) as usize;
        if k == 0 {
            start = now;
        } else if k < ROLL {
            along.push((now.z() - last.z()) / dt.0);
            up = up.max((now.y() - start.y()).abs());
            on &= now.x().abs() < 6.0 && now.z().abs() < 33.0 && now.y() > -1.0;
        }
        if k < ROLL {
            last = now;
        }
    }
    let ch = &sim.rider.ride.chassis;
    Some(Roll { along, back: start.z() - last.z(), up, on, pitch: traffic.cars[0].pose.0[2].y(), vel, centre, end: [ch.vel, ch.cofg_world, ch.quat] })
}

/// **The port rolls P1 back down the deck as the original does, bit for bit** (S182): from the
/// [`FROM`] image on the capture's pads and frame times, every tick's chassis velocity and centre is
/// the capture's row two on (a row reads the state the tick before its frame ended with), and the
/// last tick's velocity, centre and orientation are the [`TO`] image's. 59 ticks: the brake let go
/// at 132231, the roll back with nothing held, P1's control 7 at 132267..132273, and on.
#[test]
fn the_roll_back_is_the_originals() {
    let Some(r) = roll(true) else { return };
    let (Some(cap), Some(to)) = (capture(), image(TO)) else { return };
    for (k, (v, c)) in r.vel.iter().zip(&r.centre).enumerate() {
        let (t, row) = (FROM + k as u32, cap.row(u64::from(FROM) + k as u64 + 2));
        let want = |col: &str| ["x", "y", "z"].map(|a| cap.at(row, &format!("{col}.{a}")).to_bits());
        assert_eq!([0, 1, 2].map(|i| v.0[i].to_bits()), want("p1_ch_vel"), "{t}: velocity {v:?}");
        assert_eq!([0, 1, 2].map(|i| c.0[i].to_bits()), want("p1_ch_centre"), "{t}: centre {c:?}");
    }
    let ch = to.u32_at(p1(&to) + CHASSIS);
    for (got, (what, o)) in r.end.iter().zip([("velocity", 0x260), ("centre", 0x60), ("orientation", 0x1a0)]) {
        assert_eq!(bits(*got)[..3], bits(vf(&to, ch + o))[..3], "{TO}: {what} {got:?}");
    }
}

/// The port keeps P1 on the deck, drag or no drag, as the original does
/// ([`the_capture_rides_the_deck`]): S146's guess was a stock bike sliding off a wagon braked. The
/// roll back is the original's ([`the_roll_back_is_the_originals`]), and about half of it is the
/// bike's air drag: the truck drives on at ~40 ft/s, and the drag pulls the bike towards still air.
#[test]
fn the_port_keeps_p1_on_the_deck() {
    let Some(dragged) = roll(true) else { return };
    let Some(still) = roll(false) else { return };
    for (drag, r) in [(true, &dragged), (false, &still)] {
        assert!(r.on, "drag {drag}: the bike left the deck");
        assert!(r.up < 0.5, "drag {drag}: {:.3} ft up or down", r.up);
    }
    assert!((0.5..1.0).contains(&dragged.back), "rolls back {:.3} ft with drag", dragged.back);
    assert!((0.1..0.7 * dragged.back).contains(&still.back), "rolls back {:.3} ft without drag, {:.3} with", still.back, dragged.back);
}

#[test]
#[ignore = "prints the port's roll back on the truck beside the original's"]
fn print_the_roll() {
    let Some(cap) = capture() else { return };
    let (a, b) = (cap.row(u64::from(FROM) + 2), cap.row(u64::from(FROM) + 1 + ROLL as u64));
    println!("original: back {:.3} ft", cap.on_deck(a).0[2] - cap.on_deck(b).0[2]);
    println!("{}", (a + 1..=b).map(|r| format!("{:.2}", cap.on_deck(r).1[2])).collect::<Vec<_>>().join(" "));
    for drag in [true, false] {
        let Some(r) = roll(drag) else { return };
        println!("drag {drag}: back {:.3} ft, up {:.3} ft, on {}, pitch {:.4}", r.back, r.up, r.on, r.pitch);
        println!("{}", r.along.iter().map(|v| format!("{v:.2}")).collect::<Vec<_>>().join(" "));
    }
}
