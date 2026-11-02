//! S163: the original's nose-in in `biplane.p2m2` replayed through the port's [`Flight`], tick by
//! tick against what P164's capture read of it (`context/capture/p164/crash-sound.csv`): the wreck
//! word, whether the collision record touched, and the body's shock peak — the three things the
//! plane's crash clank reads.
//!
//! The P164 capture has no orientation, so the plane is seeded from C7's acceptance capture of the
//! same playback (`context/capture/acceptance/biplane.csv`, its frame counter ticking two a game
//! tick), whose rows carry the body, the surfaces, the engine and the pad. The two are lined up on
//! the body's position, which both read bit for bit.
//!
//! The second crash runs on past its wreck with free06's cars in the plane's world (S204): the save
//! state's cars, driven from the state's tick, of which the train is what throws the plane.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::{CRASHED, Pad};
use game_sim::eco::{Trees, Wooded};
use game_sim::flight::Flight;
use game_sim::flight_search::{self, Rays};
use game_sim::phy::{self, Plane};
use game_sim::track::{self, Scene, Terrain};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// The nose-in's wreck tick in P164's capture, and the same tick in C7's.
const WRECK: u32 = 11664;
const WRECK_C7: u32 = 28630;
/// The second crash's wreck tick in P164's capture: a belly skid on a hilltop at 135 ft/s.
const SECOND: u32 = 12498;
/// The tick its wreck recovers on (the plane back on its spawn), and both captures' last row.
const RESET: u32 = 12649;
const END: u32 = 12688;
/// How many ticks before a wreck the port is seeded. C7's rows lack the body's last position and
/// orientation and the engine's noise clock, so the first tick flown is the least like the game's
/// (seeded one tick back, the nose-in's wreck tick is 0.7 ft/s off; six back, 0.07).
const BACK: u32 = 6;
/// `[biplaneWreck]`'s `MinImpulse`: the clank's gate on the shock peak.
const MIN_IMPULSE: f32 = 5.0;
/// How far a shock peak's lane may be off the game's, in ft/s: what the seed's missing state leaves.
const PEAK_OFF: f32 = 0.15;

/// One csv, its rows by frame, each a map of column to text.
struct Csv {
    rows: HashMap<u32, HashMap<String, String>>,
}

impl Csv {
    fn load(path: &std::path::Path, from: u32, to: u32) -> Option<Csv> {
        let Ok(text) = std::fs::read_to_string(path) else {
            skip(&format!("no {}", path.display()));
            return None;
        };
        let mut lines = text.lines();
        let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
        let mut rows = HashMap::new();
        for l in lines {
            let c: Vec<&str> = l.split(',').collect();
            let Ok(frame) = c[0].parse::<u32>() else { continue };
            if frame < from || frame > to {
                continue;
            }
            rows.insert(frame, head.iter().cloned().zip(c.iter().map(|s| s.to_string())).collect());
        }
        Some(Csv { rows })
    }

    fn f(&self, frame: u32, name: &str) -> F {
        let row = self.rows.get(&frame).unwrap_or_else(|| panic!("no row {frame}"));
        F(row.get(name).unwrap_or_else(|| panic!("no column {name}")).parse::<f32>().expect("a float"))
    }

    fn u(&self, frame: u32, name: &str) -> u32 {
        self.rows[&frame][name].parse().expect("an integer")
    }

    fn v(&self, frame: u32, name: &str, w: f32) -> Vf {
        let [x, y, z] = ["x", "y", "z"].map(|c| self.f(frame, &format!("{name}.{c}")).to_f32());
        Vf::new(x, y, z, w)
    }
}

/// A member of `MX.PSR`, read whole.
fn psr_member(name: &str) -> Option<Vec<u8>> {
    let kit = Kit::load().ok()?;
    let path = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find(name)?;
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    Some(bytes)
}

fn entry(res: &Res, bytes: &[u8], suffix: &str) -> Option<Vec<u8>> {
    let want = suffix.to_ascii_lowercase();
    let e = res.entries.iter().find(|e| {
        let n = e.name.to_ascii_lowercase();
        n == want || n.ends_with(&format!("\\{want}"))
    })?;
    res.read(bytes, e).ok()
}

/// The disc's plane with its ray probe.
fn the_plane() -> Option<Flight> {
    let bytes = psr_member("machine.res")?;
    let res = Res::parse(&bytes).expect("machine.res");
    let plane = Plane::read_bytes(&entry(&res, &bytes, phy::BIPLANE_PHY).expect("biplane.phy"));
    let rays = track::Rays::parse(&entry(&res, &bytes, flight_search::BIPLANE_RAYS).expect("biplanem.col")).expect("rays");
    Some(Flight::new(&plane).with_rays(Rays::of(&rays)))
}

/// free06's terrain bytes, its scene, its props and its ecosystem, as the shell reads them.
fn free06() -> Option<(Vec<u8>, Scene, Props, Vec<u8>)> {
    let bytes = psr_member("Tracks\\free06.res")?;
    let res = Res::parse(&bytes).ok()?;
    let scn = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))?;
    let scene = Scene::parse(&res.read(&bytes, scn).ok()?).ok()?;
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = entry(&res, &bytes, &leaf)?;
    let esb = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".esb"))?;
    let esb = res.read(&bytes, esb).ok()?;
    let second = psr_member("Tracks\\free06_2.res");
    let mut parts = vec![(bytes.clone(), res)];
    if let Some(b) = second
        && let Ok(r) = Res::parse(&b)
    {
        parts.push((b, r));
    }
    let (props, missing) = Props::load(&scene, |want| {
        parts.iter().find_map(|(b, r)| entry(r, b, want)).ok_or_else(|| "no archive of this track has it".to_string())
    });
    assert!(missing.is_empty(), "free06's props the archives lack: {missing:?}");
    Some((terrain, scene, props, esb))
}

/// C7's frame for P164's.
fn c7(frame: u32) -> u32 {
    (WRECK_C7 as i64 + 2 * (frame as i64 - WRECK as i64)) as u32
}

/// The save state `biplane.p2m2` plays back from: the RAM of the tick before C7's first row.
const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
/// MXBiplane, the player's plane, in that state, and the fields of it read here: its collision
/// object's ray probe (whose world's moving list holds the cars, `tests/physics_cars.rs`) and its
/// spawn, the body's spot and facing a wreck's recovery puts it back on (`tests/replay.rs`, S193).
const PLANE: u32 = 0x00c4_4a40;
mod plane_at {
    pub const COLLISION: u32 = 0x170;
    pub const RAY_PROBE: u32 = 0x1f0;
    pub const SPAWN: u32 = 0xe10;
}

/// A path car's fields (`research/oracle.txt` S81, S127, S136; `tests/physics_cars.rs`).
mod car_at {
    pub const VTABLE: u32 = 0x8;
    pub const NODE: u32 = 0x254;
    pub const NAME: u32 = 0x320;
    pub const PATH: u32 = 0x430;
    pub const LAST: u32 = 0x450;
    pub const HEAD: u32 = 0x480;
    pub const UP: u32 = 0x4a0;
    pub const FIRST: u32 = 0x4e4;
    pub const CURSOR: u32 = 0x4f4;
    pub const TIME_SCALE: u32 = 0x534;
    pub const LAG: u32 = 0x558;
    pub const HEADING_FILTER: u32 = 0x55c;
    pub const NORMAL_FILTER: u32 = 0x560;
    pub const KEYS: u32 = 0x8;
    pub const CLOCK: u32 = 0xc;
    pub const KEY: u32 = 0x30;
    pub const WORLD: u32 = 0x80;
}

/// A collidable's fields and the broad phase's (`tests/physics_cars.rs`).
mod col_at {
    pub const SPACE: u32 = 0x04;
    pub const MOVER: u32 = 0x7c;
    pub const TABLE: u32 = 0x04;
    pub const MOVING: u32 = 0x38;
    pub const LIST_LAST: u32 = 0x38;
    pub const LIST_BLOCKS: u32 = 0x3c;
}

fn vf_at(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// Which of the state's cars go into the plane's world.
#[derive(Clone, Copy, PartialEq)]
enum Cars {
    None,
    All,
    /// All but the one at this place in the plane's moving list (its mesh left out: it still drives).
    Without(usize),
}

/// What the save state gives the replay: its cars as the port's traffic, their models in the same
/// order, its frame counter and the plane's spawn.
struct State {
    traffic: game_sim::traffic::Traffic,
    names: Vec<String>,
    counter: u32,
    spawn: (Vf, Vf),
}

/// [`STATE`]'s cars, in the order the plane's world lists them (free06.toy's `Car1` to `Car23`, the
/// train first; `tests/physics_cars.rs` proves the list), each on its path where the state has it,
/// with its collision mesh out of free06's archive unless `cars` leaves it out. `None` (skipped)
/// without the RAM image.
fn the_state(cars: Cars) -> Option<State> {
    use game_sim::traffic::{Car, Spec, Traffic};
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("no {} — `./kit state ram context/recordings/biplane.p2m2_SaveState.p2s`", path.display()));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    let bytes = psr_member("Tracks\\free06.res")?;
    let res = Res::parse(&bytes).expect("free06.res");
    let (path_car, physics_car) = (addr("path_car_vtable").unwrap(), addr("physics_car_vtable").unwrap());
    let ray = ee.u32_at(ee.u32_at(PLANE + plane_at::COLLISION) + plane_at::RAY_PROBE);
    let list = ee.u32_at(ee.u32_at(ee.u32_at(ray + col_at::SPACE) + col_at::TABLE) + col_at::MOVING);
    let block = ee.u32_at(ee.u32_at(list + col_at::LIST_BLOCKS));
    let last = ee.i32_at(list + col_at::LIST_LAST) as u32;
    let list: Vec<u32> = (0..last)
        .map(|k| ee.u32_at(block + 4 * k))
        .map_while(|o| {
            let car = if o & 2 != 0 { 0 } else { ee.u32_at(o + col_at::MOVER) };
            (car != 0 && [path_car, physics_car].contains(&ee.u32_at(car + car_at::VTABLE))).then_some(car)
        })
        .collect();
    assert_eq!(list.len(), 23, "free06's NumberOfCars at the head of the plane's moving list");
    let names: Vec<String> = list
        .iter()
        .map(|&car| {
            let name: Vec<u8> = (0..64).map(|k| ee.u8_at(car + car_at::NAME + k)).take_while(|&b| b != 0).collect();
            String::from_utf8_lossy(&name).to_ascii_lowercase()
        })
        .collect();
    let mat = |a: u32| game_sim::math::Mat4([vf_at(&ee, a), vf_at(&ee, a + 16), vf_at(&ee, a + 32), vf_at(&ee, a + 48)]);
    let specs = list
        .iter()
        .zip(&names)
        .enumerate()
        .map(|(k, (&car, name))| {
            let col = name.replace(".psg", ".col");
            let mesh = entry(&res, &bytes, &col).map(|b| track::Mesh::parse(&b).unwrap_or_else(|e| panic!("{col}: {e}")));
            assert!(mesh.is_some(), "no {col} in free06.res");
            let mesh = if cars == Cars::Without(k) { None } else { mesh };
            let path = ee.u32_at(car + car_at::PATH);
            let (n, keys) = (ee.u32_at(path), ee.u32_at(path + car_at::KEYS));
            let points: Vec<(F, Vf)> = (0..n).map(|k| keys + k * car_at::KEY).map(|k| (F(ee.f32_at(k)), vf_at(&ee, k + 0x10))).collect();
            let f = |o| F(ee.f32_at(car + o));
            Spec::new(&points, f(car_at::TIME_SCALE), f(car_at::HEADING_FILTER), f(car_at::NORMAL_FILTER), f(car_at::LAG), mesh).expect("a path")
        })
        .collect();
    let mut traffic = Traffic::new(specs);
    traffic.cars = list
        .iter()
        .map(|&car| Car {
            clock: F(ee.f32_at(ee.u32_at(car + car_at::PATH) + car_at::CLOCK)),
            seg: ee.u32_at(car + car_at::CURSOR),
            point: vf_at(&ee, car + car_at::LAST),
            head: vf_at(&ee, car + car_at::HEAD),
            normal: vf_at(&ee, car + car_at::UP),
            first: ee.u32_at(car + car_at::FIRST) != 0,
            pose: mat(ee.u32_at(car + car_at::NODE) + car_at::WORLD),
            stale: false,
            level: Default::default(),
        })
        .collect();
    let spawn = (vf_at(&ee, PLANE + plane_at::SPAWN), vf_at(&ee, PLANE + plane_at::SPAWN + 0x10));
    Some(State { traffic, names, counter: ee.u32_at(addr("frame_counter").unwrap()), spawn })
}

/// The plane put where C7's row has it: the body, the surfaces, the engine and the pad (as
/// `tests/replay.rs`'s `biplane_start` seeds it), then measured.
fn seed(f: &mut Flight, cap: &Csv, frame: u32, acos: &game_sim::air::AcosTable) {
    f.attitude.flown = true;
    let b = &mut f.body;
    b.position = cap.v(frame, "body_pos", 1.0);
    b.velocity = cap.v(frame, "body_vel", 1.0);
    b.quat = {
        let q = cap.v(frame, "body_quat", 0.0);
        Vf::new(q.x(), q.y(), q.z(), cap.f(frame, "body_quat.w").to_f32())
    };
    b.rates = cap.v(frame, "body_rate", 1.0);
    (b.last_position, b.last_quat) = (b.position, b.quat);
    let s = &mut f.surfaces;
    s.throttle = cap.f(frame, "throttle");
    s.brake = cap.f(frame, "thr_braking");
    s.elevator = cap.f(frame, "pitch_cmd");
    s.elevator_command = cap.f(frame, "elevator");
    s.aileron = cap.f(frame, "aileron");
    s.rudder = cap.f(frame, "rudder");
    s.steer = cap.f(frame, "nose_steer");
    s.pitch_integral = cap.f(frame, "pitch_int");
    f.pad = pad(cap, frame);
    let e = &mut f.engine;
    e.power = cap.f(frame, "eng_power");
    e.rpm = cap.f(frame, "prop_rpm");
    e.thrust = cap.f(frame, "eng_thrust");
    e.prop_angle = cap.f(frame, "prop_angle");
    e.note = cap.f(frame, "eng_sound");
    f.remeasure(acos);
}

fn pad(cap: &Csv, frame: u32) -> Pad {
    Pad {
        forward: cap.f(frame, "thr_up"),
        back: cap.f(frame, "thr_down"),
        elevator: cap.f(frame, "elev_cmd"),
        aileron: cap.f(frame, "ail_cmd"),
        rudder: cap.f(frame, "rud_cmd"),
    }
}

fn norm(p: Vf) -> f32 {
    (p.z() * p.z() + p.y() * p.y() + p.x() * p.x()).sqrt()
}

fn dist(a: Vf, b: Vf) -> f32 {
    let d = [a.x() - b.x(), a.y() - b.y(), a.z() - b.z()];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// One replayed tick: the port's and the game's wreck word, touched flag and shock peak.
#[derive(Debug)]
struct Tick {
    frame: u32,
    /// P164's row is not this tick's: it was read before the game wrote it.
    torn: bool,
    port: (u32, bool, Vf),
    game: (u32, bool, Vf),
    /// How far the port's body is off the game's, in ft and ft/s.
    off: (f32, f32),
    /// Whether the port's body is C7's row bit for bit: position, velocity, orientation and rates.
    body: bool,
}

/// The crash on P164's `wreck` tick, the port seeded from C7's row `back` ticks before it and flown on
/// C7's pads to `after` ticks past it. `None` (skipped) without the captures or the disc.
fn replay(wreck: u32, back: u32, after: u32) -> Option<Vec<Tick>> {
    replay_among(wreck, back, after, Cars::None)
}

/// [`replay`] with [`STATE`]'s `cars` in the plane's world: driven from the state's tick on C7's own
/// steps (C7's first row is the state's next tick) to the seed, then a tick of theirs ahead of each
/// of the plane's, as `Sim::step` runs a plane ride. The plane gets the state's spawn.
fn replay_among(wreck: u32, back: u32, after: u32, cars: Cars) -> Option<Vec<Tick>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let (from, to) = (wreck - back, wreck + after);
    let state = match cars {
        Cars::None => None,
        _ => Some(the_state(cars)?),
    };
    let first = state.as_ref().map_or(from, |s| s.counter + 1);
    let p164 = Csv::load(&kit.ctx("capture").join("p164").join("crash-sound.csv"), from, to)?;
    let cap = Csv::load(&kit.ctx("capture").join("acceptance").join("biplane.csv"), c7(first), c7(to))?;
    let mut f = the_plane()?;
    let (terrain, scene, props, esb) = free06()?;
    let tables = game_sim::ride::Tables::build();
    let terrain = Terrain::parse(&terrain).expect("free06's terrain");
    let ground = Ground::new(&terrain, &scene.terrain);
    let (trees, missed) = Trees::on_ground(&esb, &ground, &game_sim::camera::CurveTable::build()).expect("free06's ecosystem");
    assert!(missed.is_empty(), "free06's trees missing: {missed:?}");
    let mut place = Wooded { world: Place { ground, props }, trees };
    let mut traffic = None;
    if let Some(mut s) = state {
        s.traffic.attach(&mut place.world.props);
        for frame in first..=from {
            assert!(cap.rows.contains_key(&c7(frame)), "C7 has no row for tick {frame}: the state is not its playback's start");
            s.traffic.frame(cap.f(c7(frame), "dt"), &mut place);
        }
        f.spawn = s.spawn;
        traffic = Some(s.traffic);
    }
    let (a, b) = (cap.v(c7(from), "body_pos", 1.0), p164.v(from, "body_pos", 1.0));
    assert_eq!(a.to_bits(), b.to_bits(), "{from}: the two captures are not the same playback here");
    seed(&mut f, &cap, c7(from), &tables.acos);
    let mut ticks = Vec::new();
    for frame in from + 1..=to {
        let (row, dt) = (c7(frame), cap.f(c7(frame), "dt"));
        if let Some(t) = traffic.as_mut() {
            t.frame(dt, &mut place);
        }
        f.frame(dt, pad(&cap, row), &mut place, &tables);
        // A P164 row read before the game wrote its tick holds the last tick's body (and its gates).
        let torn = cap.v(row, "body_pos", 1.0).to_bits() != p164.v(frame, "body_pos", 1.0).to_bits();
        let quat = {
            let q = cap.v(row, "body_quat", 0.0);
            Vf::new(q.x(), q.y(), q.z(), cap.f(row, "body_quat.w").to_f32())
        };
        let b = &f.body;
        let body = [(b.position, cap.v(row, "body_pos", 1.0)), (b.velocity, cap.v(row, "body_vel", 1.0)), (b.rates, cap.v(row, "body_rate", 1.0))]
            .iter()
            .all(|(p, g)| p.to_bits()[..3] == g.to_bits()[..3])
            && b.quat.to_bits() == quat.to_bits();
        let tick = Tick {
            frame,
            torn,
            port: (f.crash.reason, f.record.touching, f.body.shock_peak),
            game: (p164.u(frame, "reason"), p164.u(frame, "touched") != 0, p164.v(frame, "shock_peak", 1.0)),
            off: (dist(f.body.position, p164.v(frame, "body_pos", 1.0)), dist(f.body.velocity, p164.v(frame, "body_vel", 1.0))),
            body,
        };
        println!(
            "{frame}{}: port {} {} {:.3} | game {} {} {:.3} | {:.4} ft {:.4} ft/s off{}",
            if torn { " (torn)" } else { "" },
            tick.port.0,
            tick.port.1,
            norm(tick.port.2),
            tick.game.0,
            tick.game.1,
            norm(tick.game.2),
            tick.off.0,
            tick.off.1,
            if body { "" } else { ", not C7's body" }
        );
        ticks.push(tick);
    }
    Some(ticks)
}

/// Whether the shell's clank (P164) would hear the tick: wrecked, the record touching, and the peak at
/// least [`MIN_IMPULSE`].
fn heard((reason, touching, peak): (u32, bool, Vf)) -> bool {
    reason != 0 && touching && norm(peak) >= MIN_IMPULSE
}

/// **The original's nose-in, replayed** (S163): biplane.p2m2's first crash, flown through the port's
/// [`Flight`] over free06 from six ticks before it, wrecks on the game's tick with the collision
/// record touching and the game's shock peak (15.95 to the game's 15.97), and on every tick to four
/// past it the wreck word, the touching flag and each lane of the peak are the game's — so the
/// crash clank's gate first passes on the wreck tick, as in the game. The wheels touch on the same
/// tick as the nose (the main gear is under compression, not bottomed): the port's dive into flat
/// ground (`flight_collision.rs`, `a_plane_crash_is_heard`), whose wheels bottom out a tick before
/// its rays reach the ground, is not this crash.
#[test]
fn the_nose_in_wrecks_on_the_originals_tick_with_its_peak() {
    let Some(ticks) = replay(WRECK, BACK, 4) else { return };
    for t in &ticks {
        assert!(!t.torn, "{}: P164's row is torn", t.frame);
        assert_eq!(t.port.0 != 0, t.game.0 != 0, "{}: the port's wreck word is {} where the game's is {}", t.frame, t.port.0, t.game.0);
        assert_eq!(t.port.0, if t.frame >= WRECK { CRASHED } else { 0 }, "{}", t.frame);
        assert_eq!(t.port.1, t.game.1, "{}: the port's record touching {} where the game's touched {}", t.frame, t.port.1, t.game.1);
        for k in 0..3 {
            let off = (t.port.2[k] - t.game.2[k]).abs();
            assert!(off < PEAK_OFF, "{}: the shock peak's lane {k} is {} where the game's is {}", t.frame, t.port.2[k], t.game.2[k]);
        }
        assert!(t.off.0 < 0.05 && t.off.1 < 0.5, "{}: the body is {} ft and {} ft/s off the game's", t.frame, t.off.0, t.off.1);
    }
    let first = |side: fn(&Tick) -> (u32, bool, Vf)| ticks.iter().find(|t| heard(side(t))).map(|t| t.frame);
    assert_eq!(first(|t| t.game), Some(WRECK), "the game's clank is not on its wreck tick");
    assert_eq!(first(|t| t.port), Some(WRECK), "the port's clank is not on the game's wreck tick");
}

/// **The second crash, through its wreck and recovery, off the train** (S163, S204): biplane.p2m2's
/// belly skid at 135 ft/s on free06's hilltop, seeded six ticks back with the save state's cars driven
/// to the seed, is the game's bit for bit on every tick to C7's last row: the body (position,
/// velocity, orientation and rates, against C7), and on every row P164 read after the game wrote it
/// the wreck word, the record touching and the shock peak. The skid wrecks on the game's tick (peak
/// 9.23, heard); on the next the plane runs into the train crossing the hilltop and is thrown off it,
/// (135, -1, 15) ft/s to (100, 26, 60); on the one after it strikes the side of the car ahead
/// ((-8.65, 0, 8.75), P164's 12500 row); 150 ticks after the wreck its recovery puts it back on the
/// spawn, where it stands to the capture's end.
#[test]
fn the_second_crash_is_the_originals_bit_for_bit_off_the_train_and_back() {
    let Some(ticks) = replay_among(SECOND, BACK, END - SECOND, Cars::All) else { return };
    assert_eq!(ticks.last().map(|t| t.frame), Some(END));
    for t in &ticks {
        assert!(t.body, "{}: the body is not C7's", t.frame);
        let wrecked = (SECOND..RESET).contains(&t.frame);
        assert_eq!(t.port.0, if wrecked { CRASHED } else { 0 }, "{}: the port's wreck word", t.frame);
        if t.torn {
            continue;
        }
        assert_eq!((t.off.0, t.off.1), (0.0, 0.0), "{}: the body is off P164's", t.frame);
        assert_eq!((t.port.0, t.port.1), (t.game.0, t.game.1), "{}: wreck word and touching", t.frame);
        assert_eq!(t.port.2.to_bits()[..3], t.game.2.to_bits()[..3], "{}: the shock peak {:?} is not the game's {:?}", t.frame, t.port.2, t.game.2);
    }
    let torn: Vec<u32> = ticks.iter().filter(|t| t.torn).map(|t| t.frame).collect();
    assert_eq!(torn, [SECOND + 1], "P164's torn rows here");
    let wreck = ticks.iter().find(|t| t.frame == SECOND).expect("the wreck tick");
    assert!(heard(wreck.port), "the port's wreck is not heard on the game's tick: {wreck:?}");
    let side = ticks.iter().find(|t| t.frame == SECOND + 2).expect("the tick after");
    assert!(heard(side.port) && side.port.2.y() == 0.0, "the car's side, a vertical face: {side:?}");
}

/// **What throws the plane is the train** (S204): free06.toy's seventh car (the second
/// `train_car_03`, crossing the hilltop at 55 ft/s) on the tick after the wreck, and the sixth's
/// (`train_car_06`) side on the tick after that. With no cars in the plane's world the port's plane
/// skims the ground and parts from C7 on the tick after the wreck (S163's finding); with the seventh
/// car's mesh left out, on the same tick; with the sixth's, a tick later.
#[test]
fn the_train_throws_the_second_crash() {
    let Some(state) = the_state(Cars::All) else { return };
    assert_eq!((state.names[5].as_str(), state.names[6].as_str()), ("train_car_06.psg", "train_car_03.psg"));
    let first_off = |cars| replay_among(SECOND, BACK, 3, cars).map(|ticks| ticks.iter().find(|t| !t.body).map(|t| t.frame));
    let Some(none) = first_off(Cars::None) else { return };
    assert_eq!(none, Some(SECOND + 1), "with no cars");
    assert_eq!(first_off(Cars::Without(6)).flatten(), Some(SECOND + 1), "without the seventh car");
    assert_eq!(first_off(Cars::Without(5)).flatten(), Some(SECOND + 2), "without the sixth car");
    assert_eq!(first_off(Cars::All).flatten(), None, "with every car");
}
