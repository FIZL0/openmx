//! S272: C16's flying tape, the port's plane against the original's, by S228's method.
//!
//! `context/recordings/c16/fly.p2m2` is a state of the split free09 (C10's session, the S228 heap) in
//! which P1 switches to plane 0 from the pause menu, taxis into P2's parked rider (the joust that wrecks
//! P2 with reason 7, 137857), takes off (138061), crashes inverted into the ground (139204), is put back
//! 300 ticks on (139504) and taxis into the scenery south of the spawn at 46 ft/s (139913..139918, no
//! crash). C16 captured its playback with watch/c16-fly.toml (`context/capture/c16/fly.csv`, one row a
//! tick at 60 Hz, 137403..140224); S272 saved the original's RAM in the ticks before each event
//! (`context/states/s272/s272-<frame>.ram`, research/c10/play_at.sh and research/s272/rams.py). The
//! index is research/c16/index.md, the findings research/s272/index.md.
//!
//! - [`the_plane_flies_each_event_as_the_original`]: plane 0 seeded off the image before each event
//!   (body, tires, engine, controls, crash, spawn, noise, and what no word of the record holds: the
//!   game's rand(), a wreck's gravity, the aero model's true speed), on the tape's pads and the capture's frame
//!   times over the split free09's ground, props and trunks with P2's bike standing where the image has
//!   it, against the capture's body position and velocity tick by tick ([`held`]); where each run
//!   parts is pinned ([`EVENTS`]).
//! - [`the_plane_flies_the_tape_as_the_original`]: the whole tape, 100 frames at a time from an image
//!   every 100 frames, pinned the same way ([`STRETCHES`]).
//! - [`the_plane_jousts_p2_as_the_original`]: the joust in the split screen's pass, both seats
//!   stepped ([`fly_two`]): P2's chassis, its wreck 7 on the original's tick, and the plane's body
//!   ([`JOUSTS`]).
//!
//! The images, the capture, the tape and the disc are game data, git-ignored: absent, the tests skip.

use game_render::disc::Disc;
use game_sim::Sim;
use game_sim::archive::{Psr, Res};
use game_sim::biplane::Area;
use game_sim::noise::SmoothNoise;
use game_sim::bike_col::{self, BikeCol};
use game_sim::camera::CurveTable;
use game_sim::eco::{Trees, Wooded};
use game_sim::flight::{self, Flight};
use game_sim::flight_search::{self, Rays};
use game_sim::input::Report;
use game_sim::phy::{self, Phy, Plane};
use game_sim::replay::P2m2;
use game_sim::ride::{Ride, Wheel};
use game_sim::rider::{Rider, Tables};
use game_sim::track::{self, Scene, Terrain};
use game_sim::trick::Set;
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;

/// The capture of the tape's playback (watch/c16-fly.toml).
const CAPTURE: &str = "capture/c16/fly.csv";

/// The game manager's slot (kit.toml's `@game_manager`).
const GAME_MANAGER: u32 = 0x004e_1e98;

/// The plane record's fields (tests/replay.rs's `plane_at`, watch/c16-fly.toml).
mod plane_at {
    pub const BODY: u32 = 0x160;
    pub const ENGINE: u32 = 0x400;
    pub const TIRES: [u32; 3] = [0x408, 0x40c, 0x410];
    pub const FLOWN: u32 = 0x334;
    pub const PITCH_INTEGRAL: u32 = 0x584;
    pub const CRASH: u32 = 0x3b0;
    pub const CRASH_DOWN: u32 = 0x3b4;
    pub const CRASH_TIME: u32 = 0x3bc;
    /// The spawn transform the factory copied (oracle.txt vehicle_factory, S205): the body's spot,
    /// the nose, the up.
    pub const SPAWN: u32 = 0xe10;
}

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (S272: research/s272/index.md)", path.display()));
        return None;
    }
    Some(path)
}

/// S272's RAM image at `frame`.
fn image(frame: u32) -> Option<Ee> {
    let path = ctx(&format!("states/s272/s272-{frame}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "s272-{frame}.ram is not frame {frame}");
    Some(ee)
}

fn chain(ee: &Ee, hops: &[u32]) -> u32 {
    hops.iter().fold(ee.u32_at(GAME_MANAGER), |a, &h| ee.u32_at(a + h))
}

/// Plane 0: the actor list's first entry (watch/c16-fly.toml), flown by P1 after the switch.
fn plane0(ee: &Ee) -> u32 {
    chain(ee, &[0x34, 0x34, 0x0])
}

/// P1's ridden vehicle, parked where P1 left it for the plane: the list's fourth entry.
fn p1(ee: &Ee) -> u32 {
    let v = chain(ee, &[0x34, 0x34, 0x8, 0x8, 0x8, 0x0]);
    assert_eq!(ee.u32_at(v), 2, "p1's list entry is not a ridden vehicle");
    assert_eq!(ee.u32_at(ee.u32_at(v + 0x20) + 0x30), 0, "the list's p1 is another player");
    v
}

/// P2's ridden vehicle: the list's third entry, a ridden vehicle (type 2) of player 1.
fn p2(ee: &Ee) -> u32 {
    let v = chain(ee, &[0x34, 0x34, 0x8, 0x8, 0x0]);
    assert_eq!(ee.u32_at(v), 2, "p2's list entry is not a ridden vehicle");
    assert_eq!(ee.u32_at(ee.u32_at(v + 0x20) + 0x30), 1, "the list's p2 is another player");
    v
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// The capture's few columns these tests read.
struct Rows {
    first: u64,
    cols: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Rows {
    fn row(&self, frame: u32) -> Option<usize> {
        let r = u64::from(frame).checked_sub(self.first)? as usize;
        (r < self.rows.len()).then_some(r)
    }
    fn text(&self, frame: u32, col: &str) -> &str {
        let k = self.cols.iter().position(|c| c == col).unwrap_or_else(|| panic!("no column {col}"));
        &self.rows[self.row(frame).unwrap_or_else(|| panic!("no row {frame}"))][k]
    }
    fn f(&self, frame: u32, col: &str) -> f32 {
        self.text(frame, col).parse::<f64>().unwrap_or_else(|_| panic!("{col} at {frame}")) as f32
    }
    fn u(&self, frame: u32, col: &str) -> u32 {
        self.text(frame, col).parse().unwrap_or_else(|_| panic!("{col} at {frame}"))
    }
    fn v3(&self, frame: u32, what: &str) -> [u32; 3] {
        ["x", "y", "z"].map(|a| self.f(frame, &format!("{what}.{a}")).to_bits())
    }
    /// The plane's velocity and position on the row of `frame`, as bits.
    fn state(&self, frame: u32) -> [[u32; 3]; 2] {
        [self.v3(frame, "body_vel"), self.v3(frame, "body_pos")]
    }
}

fn capture() -> Option<Rows> {
    let path = ctx(CAPTURE)?;
    let file = std::io::BufReader::new(File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let keep = |c: &str| {
        c == "dt" || c == "crash" || c == "speed" || c == "on_ground" || c == "p2_wreck_reason" || c.starts_with("p2_ch_vel.") || c.starts_with("p2_ch_centre.") || c.starts_with("body_pos.") || c.starts_with("body_vel.")
    };
    let cap = game_sim::replay::two_p::read(file, keep).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    Some(Rows { first: cap.frames[0], cols: cap.columns.clone(), rows: cap.rows })
}

fn tape() -> Option<P2m2> {
    let path = ctx("recordings/c16/fly.p2m2")?;
    Some(P2m2::parse(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e:?}", path.display())))
}

/// The frame whose tick polls the tape's first field: the one of the state's frame + 1..=5 under
/// which the taxi from the state holds longest (S228's way): the state's + 2, every one of 298 ticks;
/// each other offset parts on the 32nd, the throttle's first press. S228's was the state's + 3.
const FIELD_ZERO: u32 = 137404;

fn tape_pad(tape: &P2m2, t: u32, zero: u32) -> Report {
    match t.checked_sub(zero) {
        Some(n) => tape.field(n as usize, 0),
        None => tape.field(0, 0),
    }
}

/// L1 + R1 pressed together on tick `t`'s field: the reset chord.
fn chord(tape: &P2m2, t: u32, zero: u32) -> bool {
    let (now, was) = (tape_pad(tape, t, zero).buttons(), tape_pad(tape, t - 1, zero).buttons());
    let (l, r) = (0x4u16, 0x8u16);
    let pressed = |b: u16| now & b != 0 && was & b == 0;
    (pressed(l) && now & r != 0) || (pressed(r) && now & l != 0)
}

/// The machines' archive out of MX.PSR: the plane's tuning, its ray probe and its hull.
fn machines() -> Option<(Plane, Rays, track::Mesh)> {
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
    let member = |name: &str| {
        let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("machine.res has {name}"));
        res.read(&bytes, e).expect("its bytes")
    };
    let plane = Plane::read_bytes(&member(phy::BIPLANE_PHY));
    let rays = track::Rays::parse(&member(flight_search::BIPLANE_RAYS)).expect("biplanem.col");
    let hull = track::Mesh::parse(&member(flight::HULL)).expect("biplanef.col");
    Some((plane, Rays::of(&rays), hull))
}

/// The split free09's archives, read by suffix.
fn track_read(disc: &mut Disc) -> impl Fn(&str) -> Result<Vec<u8>, String> + use<> {
    let parts = ["Tracks\\free09_ss.res", "Tracks\\free09_ss_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    move |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"))
}

/// The bike, `Race500P.phy`, and the bike's collision files out of the split track's archive (S157's,
/// tests/fly_and_collide.rs's).
fn bike(disc: &mut Disc, tables: &Tables) -> (Phy, BikeCol) {
    let read = track_read(disc);
    let col = BikeCol::load(&read(&bike_col::rays_file(bike_col::MODEL)).unwrap(), &read(&bike_col::shape_file(bike_col::MODEL)).unwrap())
        .expect("the bike's collision files");
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let phy = bikes
        .iter()
        .find_map(|b| disc.res(b).ok()?.read_suffix("race500p.phy").ok())
        .expect("race500p.phy in a bike archive");
    (Phy::read_bytes(&phy, &tables.ride.curve), col)
}

/// The bikes' collision box the plane's search strikes (S95), out of whichever archive has it.
fn bike_box(disc: &mut Disc) -> Option<track::Mesh> {
    let read = track_read(disc);
    let bytes = read(flight_search::BIKE_COLLISION).ok().or_else(|| {
        let names: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().ends_with(".res")).collect();
        names.iter().find_map(|n| disc.res(n).ok()?.read_suffix(flight_search::BIKE_COLLISION).ok())
    })?;
    track::Mesh::parse(&bytes).ok()
}

/// The wreck bike's hull mesh (`wreckbikef.col`), as the shell's ragdoll kit parses it: the plane's
/// rays strike a downed rider's (F35, [`Flight::with_wreck_hull`]), whose rider here has no kit.
fn wreck_hull(disc: &mut Disc) -> Option<track::Mesh> {
    let names: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().ends_with(".res")).collect();
    let bytes = names.iter().find_map(|n| disc.res(n).ok()?.read_suffix("wreckbikef.col").ok())?;
    game_sim::ragdoll_scene::Bike::parse(&bytes).map(|b| b.mesh)
}

/// The split free09 as the shell reads it (S157's: terrain, props out of both archives, the
/// ecosystem's trunks) and the terrain's extent for the plane's edge, handed to `then`.
fn in_free09_ss<R>(disc: &mut Disc, then: impl FnOnce(&mut Wooded<Place<'_>>, Area) -> R) -> R {
    let read = track_read(disc);
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&bytes).expect("parse the terrain");
    let t = &scene.terrain;
    let area = Area::new(F(t.width as f32), F(t.breadth as f32), t.width_scale, t.breadth_scale);
    let ground = Ground::new(&terrain, &scene.terrain);
    let (props, _) = Props::load(&scene, |w: &str| read(w));
    let (trees, missed) = Trees::on_ground(&read(".esb").unwrap(), &ground, &CurveTable::build()).expect("the ecosystem");
    assert!(missed.is_empty(), "free09_ss's trunks the port lacks: {missed:?}");
    then(&mut Wooded { world: Place { ground, props }, trees }, area)
}

/// **Plane 0 off an image**: what `tests/replay.rs`'s `off_the_save_state` and
/// `controls_off_the_save_state` read off the one-player plane, here off the 2p list's plane 0 — the
/// whole body, the three tires, the engine, the pad and surfaces, the flown gate, the crash state,
/// the spawn and the noise tables — then measured.
fn seeded_plane(ee: &Ee, plane: &Plane, rays: &Rays, hull: &track::Mesh, bike: Option<&track::Mesh>, area: Area, tables: &Tables) -> Flight {
    seeded_plane_at(ee, plane0(ee), plane, rays, hull, bike, area, tables)
}

/// [`seeded_plane`] off the plane record `p`: plane 0's, or plane 1's (N24).
#[allow(clippy::too_many_arguments)]
fn seeded_plane_at(ee: &Ee, p: u32, plane: &Plane, rays: &Rays, hull: &track::Mesh, bike: Option<&track::Mesh>, area: Area, tables: &Tables) -> Flight {
    let word = |at: u32| F(ee.f32_at(at));
    let mut f = Flight::new(plane).with_rays(rays.clone()).with_hull(hull.clone());
    if let Some(mesh) = bike {
        f = f.with_bike(mesh.clone());
    }
    f.area = Some(area);
    let b = ee.u32_at(p + plane_at::BODY);
    let body = &mut f.body;
    body.enabled = ee.u32_at(b) != 0;
    body.inv_mass = word(b + 0x10);
    for (v, o) in [
        (&mut body.damping, 0x20),
        (&mut body.last_position, 0x50),
        (&mut body.position, 0x60),
        (&mut body.velocity, 0x70),
        (&mut body.last_force, 0x80),
        (&mut body.force, 0x90),
        (&mut body.accel, 0xa0),
        (&mut body.shock_peak, 0xb0),
        (&mut body.shock, 0xc0),
        (&mut body.step, 0xf0),
        (&mut body.inv_inertia, 0x120),
        (&mut body.ang_damping, 0x130),
        (&mut body.last_quat, 0x150),
        (&mut body.quat, 0x170),
        (&mut body.cofg, 0x180),
        (&mut body.moment, 0x190),
        (&mut body.last_moment, 0x1a0),
        (&mut body.rates, 0x1b0),
        (&mut body.ang_accel, 0x1c0),
        (&mut body.twist_peak, 0x1d0),
        (&mut body.twist, 0x1e0),
    ] {
        *v = quad(ee, b + o);
    }
    for (tire, at) in f.tires.iter_mut().zip(plane_at::TIRES) {
        let t = ee.u32_at(p + at);
        let flag = |o: u32| ee.u32_at(t + o) != 0;
        (tire.below, tire.held, tire.touching) = (flag(0x80), flag(0x84), flag(0x88));
        (tire.compression, tire.last_compression, tire.spring_force) = (word(t + 0x8c), word(t + 0x90), word(t + 0x94));
        (tire.angle, tire.spin) = (word(t + 0x120), word(t + 0x124));
        for (v, o) in [
            (&mut tire.offset, 0x50),
            (&mut tire.bottom, 0xa0),
            (&mut tire.ground, 0xc0),
            (&mut tire.normal, 0xd0),
            (&mut tire.contact, 0xe0),
            (&mut tire.hub, 0xf0),
            (&mut tire.low, 0x100),
            (&mut tire.contact_velocity, 0x110),
            (&mut tire.last_normal, 0x140),
            (&mut tire.slip, 0x170),
            (&mut tire.friction, 0x180),
        ] {
            *v = quad(ee, t + o);
        }
        tire.frame = [0x190, 0x1a0, 0x1b0].map(|o| quad(ee, t + o));
    }
    let e = ee.u32_at(p + plane_at::ENGINE);
    let en = &mut f.engine;
    (en.power, en.rpm, en.thrust) = (word(e + 0x5c), word(e + 0x60), word(e + 0x6c));
    (en.prop_angle, en.note, en.clock) = (word(e + 0x48), word(e + 0x70), word(e + 0x74));
    f.attitude.flown = ee.u32_at(p + plane_at::FLOWN) != 0;
    let pw = |o: u32| word(p + o);
    let pad = &mut f.pad;
    (pad.forward, pad.back, pad.elevator, pad.aileron, pad.rudder) = (pw(0x338), pw(0x33c), pw(0x340), pw(0x344), pw(0x348));
    let s = &mut f.surfaces;
    (s.throttle, s.brake, s.elevator, s.elevator_command) = (pw(0x34c), pw(0x350), pw(0x354), pw(0x358));
    (s.aileron, s.rudder, s.steer, s.pitch_integral) = (pw(0x35c), pw(0x360), pw(0x364), pw(plane_at::PITCH_INTEGRAL));
    f.crash.reason = ee.u32_at(p + plane_at::CRASH);
    f.crash.landed = ee.u32_at(p + plane_at::CRASH_DOWN) != 0;
    f.crash.clock = pw(plane_at::CRASH_TIME);
    // A wrecked plane falls on `WreckGravity`, which no record word holds: Flight::wreck's force, as
    // biplane::gravity_force builds it (the damping and inertia it set are the body's, read above).
    if f.crash.reason != 0 {
        let y = F(phy::G) * (-f.wrecks.wreck_gravity * f.mass);
        f.gravity = Vf::new(0.0, y.0, 0.0, 1.0);
    }
    f.spawn = (quad(ee, p + plane_at::SPAWN), quad(ee, p + plane_at::SPAWN + 0x10));
    f.spawn_up = quad(ee, p + plane_at::SPAWN + 0x20);
    let table = |name: &str| addr(name).unwrap_or_else(|why| panic!("{name}: {why}"));
    let (perm, g1) = (table("smooth_noise_perm"), table("smooth_noise_g1"));
    assert_eq!(ee.u32_at(table("smooth_noise_unbuilt")), 0, "the noise tables are not built in the image");
    f.noise = Some(SmoothNoise::from_tables(
        &std::array::from_fn(|k| ee.u32_at(perm + 4 * k as u32)),
        &std::array::from_fn(|k| F(ee.f32_at(g1 + 4 * k as u32))),
    ));
    // The game's rand() as the image left it, which the plane's touchdown and throw picks draw.
    f.rng = game_sim::rng::Rng::from_state(ee.read(table("rand_state"), 8) as u64);
    f.remeasure(&tables.ride.acos);
    // The aero model's true speed is the last post's: a wheel down or the record touching
    // (Flight::post). In the air it is off, and the yaw damping reads the floored speed.
    f.aero.true_speed = f.tires.iter().any(|t| t.touching) || f.record.touching;
    f
}

/// P2's rider as tests/fly_and_collide.rs builds P1's, from P2's record: only the chassis, which is
/// what the plane's search reads of a standing bike (its transform, [`flight_search::Bike`]).
fn standing_p2(ee: &Ee, phy: &Phy, col: &Arc<BikeCol>) -> Rider {
    standing_at(ee, p2(ee), phy, col)
}

/// [`standing_p2`] off the rider record `r` (R53's boot lists its riders elsewhere).
fn standing_at(ee: &Ee, r: u32, phy: &Phy, col: &Arc<BikeCol>) -> Rider {
    let c = ee.u32_at(r + 0x7a4);
    let mounts = (quad(ee, ee.u32_at(r + 0x7b0) + 0xa0), quad(ee, ee.u32_at(r + 0x7b4) + 0xa0));
    let mut rider = Rider::new(phy, mounts, quad(ee, c + 0x60), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    rider.no_start_line();
    rider.carries_bike_col(Arc::clone(col));
    let node = ee.u32_at(c + 0x254);
    let ride = &mut rider.ride;
    ride.chassis.cofg_world = quad(ee, c + 0x60);
    ride.chassis.cofg = quad(ee, c + 0x190);
    ride.chassis.quat = quad(ee, c + 0x1a0);
    ride.node = [quad(ee, node + 0x80), quad(ee, node + 0x90), quad(ee, node + 0xa0)];
    ride.origin = quad(ee, node + 0xb0);
    rider
}

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
    fn chassis(&self, ride: &mut Ride) {
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
    fn wheel(&self, which: usize, wh: &mut Wheel) {
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
    /// `seed_engine_outputs`, `drivetrain`, `seed_balance`, `seed_air`, `seed_held`).
    fn rest(&self, ride: &mut Ride) {
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
        // What tests/air.rs pokes for the rest of the air step: the authority, the preload and the
        // flip, so a ride seeded on a lip or in a flip goes on with them.
        air.authority = self.float(r + 0x4e8);
        air.preload_ready = self.flag(r + 0x4a0);
        air.preload_firing = self.flag(r + 0x4a4);
        air.preload_armed = self.flag(r + 0x4a8);
        air.preload_timer = self.float(r + 0x4b0);
        air.preload_force = self.vf(r + 0x490);
        air.flip_torque_applied = self.flag(r + 0x4f4);
        air.flipping = self.flag(r + 0x4f8);
        air.flip_active = self.flag(r + 0x4fc);
        air.flip_armed = self.flag(r + 0x500);
        air.flip_grace = self.float(r + 0x50c);
        air.flip_torque = self.vf(r + 0x520);
        air.wreck_impact = self.vf(r + 0x560);
        let h = &mut ride.held;
        h.stick = (self.float(r + 0x628), self.float(r + 0x62c));
        h.flick = self.float(r + 0x630);
        h.front_brake = self.float(r + 0x650);
        h.brake = h.front_brake;
        h.rear_brake = self.float(r + 0x654);
        h.lean = (self.float(r + 0x670), self.float(r + 0x674));
        ride.back = game_sim::ride::Back { at: self.vf(r + 0x3b0), facing: self.vf(r + 0x3c0), up: self.vf(r + 0x3d0) };
    }
}

/// A rider as the shell builds it for the 2p state's bike (S174's: Race500P, the bike's collision
/// files, the 50cc reach), seeded off the image's record `rider` (tests/fly_and_collide.rs's).
fn seeded_rider(ee: &Ee, rider: u32, phy: &Phy, col: &Arc<BikeCol>) -> Rider {
    let im = Image { ee, rider };
    let c = im.at(0x7a4);
    let mut rider = Rider::new(phy, im.mounts(), im.vf(c + 0x60), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    rider.no_start_line();
    rider.carries_bike_col(Arc::clone(col));
    rider.fifty = track::fifty_reach(&track::physics_file(track::Engine::Cc500, false, track::Tuning::P));
    im.chassis(&mut rider.ride);
    im.wheel(0, &mut rider.ride.front);
    im.wheel(1, &mut rider.ride.rear);
    im.rest(&mut rider.ride);
    rider
}
/// One tick of the port's flight: the frame it stepped and the plane after it.
#[derive(Debug, Clone)]
struct Tick {
    t: u32,
    state: [[u32; 3]; 2],
    crash: u32,
    /// P2's chassis velocity and centre, and the wreck its frame said ([`fly_two`]; in [`fly`], which
    /// does not step P2's rider, a downed P2's wreck bike's body velocity and position, S276).
    p2: [[u32; 3]; 2],
    /// The centre P2's next integrate rebuilds from the transform before it writes the new one
    /// ([`rebuilt_centre`]; [`fly_two`] only).
    p2_rebuilt: [u32; 3],
    p2_wreck: Option<u8>,
    /// The plane's velocity and position after the tick's first substep, and how many substeps it
    /// took ([`game_sim::flight::substeps`]): where a row PINE read inside the frame stands (T8).
    first: [[u32; 3]; 2],
    substeps: usize,
    /// P2's wreck after the tick, while P2 is down ([`fly_two`]; F30).
    wreck: Option<Box<game_sim::ride::Crash>>,
    /// The plane's contact list after the tick (F30).
    plane_list: Vec<game_sim::contact::Contact>,
    /// The plane's whole body after the tick (R31).
    plane_body: game_sim::biplane::Body,
}

/// The plane's state after the first of the substeps [`game_sim::flight::substeps`] kept, and their
/// count; the tick's end if it kept none.
fn first_substep(kept: &[game_sim::biplane::Body], end: [[u32; 3]; 2]) -> ([[u32; 3]; 2], usize) {
    (kept.first().map_or(end, |b| [three(b.velocity), three(b.position)]), kept.len())
}

fn three(v: Vf) -> [u32; 3] {
    [0, 1, 2].map(|k| v.0[k].to_bits())
}

/// **Plane 0's flight by the port from image `from` through frame `to`**, polling the tape's field
/// `t - zero` on tick `t` and stepping at row `t + 1`'s `dt` (S228's rule). P1's own bike is out of the
/// plane's world, as the original takes it out at the switch; P2's stands where the image has it, or
/// is down where the capture has P2 wrecked ([`wrecked_p2`], S276), its wreck bike meeting the plane.
fn fly(from: u32, to: u32, zero: u32) -> Option<([[u32; 3]; 2], Vec<Tick>)> {
    let cap = capture()?;
    let tape = tape()?;
    let dt_of = |t: u32| F(cap.f((t + 1).max(cap.first as u32), "dt"));
    let reason = cap.u(from.max(cap.first as u32), "p2_wreck_reason");
    fly_by(from, to, &dt_of, &|t| tape_pad(&tape, t, zero), &|t| chord(&tape, t + 1, zero), reason)
}

/// [`fly`] on any pads and steps: tick `t` steps at `dt_of(t)` on pad `pad_of(t)`, its reset chord
/// `respawn_of(t)`; P2 down with wreck `reason`, or standing for 0.
fn fly_by(
    from: u32,
    to: u32,
    dt_of: &dyn Fn(u32) -> F,
    pad_of: &dyn Fn(u32) -> Report,
    respawn_of: &dyn Fn(u32) -> bool,
    reason: u32,
) -> Option<([[u32; 3]; 2], Vec<Tick>)> {
    let ee = image(from)?;
    let (plane, rays, hull) = machines()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let tables = Arc::new(Tables::build());
    let (phy, col) = bike(&mut disc, &tables);
    let col = Arc::new(col);
    let mesh = bike_box(&mut disc);
    assert!(mesh.is_some(), "no {} in the disc", flight_search::BIKE_COLLISION);
    let mut other = standing_p2(&ee, &phy, &col);
    if reason != 0 {
        let (part, wrecks) = wreck_files(&mut disc).expect("the rider's wreck files on the disc");
        other.carries_wreck(part, wrecks, game_sim::ride::WRECK_INERTIA);
        wrecked_p2(&ee, p2(&ee), reason, &mut other);
    }
    let me = standing_p2(&ee, &phy, &col);
    let mut sim = Sim::with_rider(0x5272, Arc::clone(&tables), phy.clone(), me);
    sim.add_player(phy, other);
    let wreck = wreck_hull(&mut disc);
    in_free09_ss(&mut disc, |world, area| {
        let flight = seeded_plane(&ee, &plane, &rays, &hull, mesh.as_ref(), area, &tables);
        sim.plane = Some(match wreck.clone() {
            Some(w) => flight.with_wreck_hull(w),
            None => flight,
        });
        // The pad the image's tick read, so the first tick's presses are not edges.
        sim.poll(dt_of(from - 1), &[pad_of(from - 1), Report::missing()]);
        let p = sim.plane.as_ref().unwrap();
        let seed = [three(p.body.velocity), three(p.body.position)];
        let mut out = Vec::new();
        for t in from..to {
            let dt = dt_of(t);
            let mut controls = sim.poll(dt, &[pad_of(t), Report::missing()]);
            controls.respawn = respawn_of(t);
            let (_, kept) = game_sim::flight::substeps(|| sim.step(dt, &controls, world));
            let p = sim.plane.as_ref().unwrap();
            let p2 = sim.others[0].rider.crash.as_ref().map_or([[0; 3]; 2], |c| [three(c.bike.body.velocity), three(c.bike.body.position)]);
            let state = [three(p.body.velocity), three(p.body.position)];
            let (first, substeps) = first_substep(&kept, state);
            out.push(Tick { t, state, crash: p.crash.reason, p2, p2_rebuilt: [0; 3], p2_wreck: None, first, substeps, wreck: None, plane_list: p.record.contacts.clone(), plane_body: p.body });
        }
        Some((seed, out))
    })
}

/// **The split screen's pass, plane 0 and P2's rider, from image `from` through frame `to`** — the
/// online seats' tick (`openmx::net::rides`, the offline shell's `simulate` the same in Sims of its
/// own): every bike's contacts and the flown plane's hull's mirrors from where the last tick left
/// them ([`rider_contact::among_met`]); the flying seat first, its plane meeting P2's bike both ways
/// ([`rider_contact::met_plane_among`], [`Flight::frame_met`], then [`rider_contact::pushed_among`]);
/// then P2's rider on its own pad, its list with the plane's strikes first
/// ([`rider_contact::struck_first`], [`rider_contact::bump_among_in`] with the plane among its
/// others), then [`rider_contact::bumped_among`] and the plane pushed back ([`Flight::pushed`]).
/// P1's own bike is parked out of the world, as the original's switch leaves it.
fn fly_two(from: u32, to: u32, zero: u32) -> Option<([[[u32; 3]; 2]; 2], Vec<Tick>)> {
    let cap = capture()?;
    let tape = tape()?;
    let dt_of = |t: u32| F(cap.f((t + 1).max(cap.first as u32), "dt"));
    let p2_pad = |t: u32| tape.field(t.saturating_sub(zero) as usize, 1);
    let reason = cap.u(from, "p2_wreck_reason");
    fly_two_by(from, to, &dt_of, &|t| tape_pad(&tape, t, zero), &p2_pad, &|t| chord(&tape, t + 1, zero), reason)
}

/// [`fly_two`] on any pads and steps, as [`fly_by`] is [`fly`]'s: P2's pad on tick `t` is `p2_pad_of(t)`.
fn fly_two_by(
    from: u32,
    to: u32,
    dt_of: &dyn Fn(u32) -> F,
    pad_of: &dyn Fn(u32) -> Report,
    p2_pad_of: &dyn Fn(u32) -> Report,
    respawn_of: &dyn Fn(u32) -> bool,
    reason: u32,
) -> Option<([[[u32; 3]; 2]; 2], Vec<Tick>)> {
    use game_sim::rider_contact;
    let ee = image(from)?;
    let (plane, rays, hull) = machines()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let tables = Arc::new(Tables::build());
    let (phy, col) = bike(&mut disc, &tables);
    let col = Arc::new(col);
    let mesh = bike_box(&mut disc);
    assert!(mesh.is_some(), "no {} in the disc", flight_search::BIKE_COLLISION);
    let mut seats = vec![
        Sim::with_rider(0x5272, Arc::clone(&tables), phy.clone(), seeded_rider(&ee, p1(&ee), &phy, &col)),
        Sim::with_rider(0x5273, Arc::clone(&tables), phy.clone(), seeded_rider(&ee, p2(&ee), &phy, &col)),
    ];
    if let Some((part, wrecks)) = wreck_files(&mut disc) {
        for s in &mut seats {
            s.rider.carries_wreck(part.clone(), wrecks.clone(), game_sim::ride::WRECK_INERTIA);
        }
    }
    if reason != 0 {
        wrecked_p2(&ee, p2(&ee), reason, &mut seats[1].rider);
    }
    let wreck = wreck_hull(&mut disc);
    in_free09_ss(&mut disc, |world, area| {
        let flight = seeded_plane(&ee, &plane, &rays, &hull, mesh.as_ref(), area, &tables);
        seats[0].plane = Some(match wreck.clone() {
            Some(w) => flight.with_wreck_hull(w),
            None => flight,
        });
        let pads = |t: u32| [pad_of(t), Report::missing()];
        let p2_pads = |t: u32| [p2_pad_of(t), Report::missing()];
        seats[0].poll(dt_of(from - 1), &pads(from - 1));
        seats[1].poll(dt_of(from - 1), &p2_pads(from - 1));
        let p = seats[0].plane.as_ref().unwrap();
        let ch = &seats[1].rider.ride.chassis;
        let seed = [[three(p.body.velocity), three(p.body.position)], [three(ch.vel), three(ch.cofg_world)]];
        let mut out = Vec::new();
        for t in from..to {
            let dt = dt_of(t);
            let touching = [true, true];
            let (mut lists, mut mirrors) = match rider_contact::among_met(&seats, &touching, dt, &*world) {
                Some((l, m)) => (Some(l), m),
                None => (None, Vec::new()),
            };
            // P2's wreck bike's search, plane 0's hull in its world (S274): its mirrors go on the plane's list.
            // The flying seat: its plane among the bikes, and P2's wreck bike's search with its hull
            // in the wreck bike's world before the plane's frame, its step after (S274, S276).
            let first;
            let mut struck = {
                let m = mirrors.get_mut(0).map(std::mem::take).unwrap_or_default();
                let at = seats[0].plane.as_ref().unwrap().body.position;
                let mut met = rider_contact::met_plane_among(&seats, 0, 2, m, at, &*world);
                let downed = {
                    let (me, others) = seats.split_at_mut(1);
                    let mut riders: Vec<&mut Rider> = others.iter_mut().map(|s| &mut s.rider).collect();
                    game_sim::wreck_plane::search_downed(&mut riders, me[0].plane.as_ref().unwrap(), dt, &mut *world, &mut met)
                };
                let on: Vec<(&Rider, bool)> = seats.iter().map(|s| (&s.rider, s.plane.is_none())).collect();
                let bikes = rider_contact::plane_bikes(&on, 0, at, &*world);
                drop(on);
                let mut controls = seats[0].poll(dt, &pads(t));
                controls.respawn = respawn_of(t);
                let s = &mut seats[0];
                let pad = flight::pad(s.input.pad(0));
                let f = s.plane.as_mut().unwrap();
                f.reset_chord(controls.respawn, &tables.ride.acos);
                let (_, kept) = game_sim::flight::substeps(|| f.frame_met(dt, pad, world, &tables.ride, &bikes, Some(&mut met)));
                first = kept;
                rider_contact::pushed_among(&mut seats, &met);
                // P2's own step comes after: its crash skips the wreck bike's tick (WreckBike::flown).
                let (me, others) = seats.split_at_mut(1);
                let mut riders: Vec<&mut Rider> = others.iter_mut().map(|s| &mut s.rider).collect();
                game_sim::wreck_plane::step_downed(&mut riders, me[0].plane.as_mut().unwrap(), downed, &mut met, dt, &*world, true);
                drop(riders);
                met.struck
            };
            // P2's seat: its rider among the plane.
            let at = seats[1].rider.ride.transform().0[3];
            let firsts = vec![struck.get_mut(1).map(std::mem::take).unwrap_or_default()];
            let own = lists.as_mut().map(|l| std::mem::take(&mut l[1]));
            let mut bump = own.map(|l| rider_contact::struck_first(&firsts, l, at, &*world)).filter(|l| !l.is_empty()).map(|l| {
                let mut b = rider_contact::bump_among_in(&seats, 1, l, &*world);
                let from = seats[1].rider.ride.transform().0[3];
                b.planes = seats[0].plane.as_ref().map(|f| f.flown(0, from, &*world)).into_iter().collect();
                b
            });
            let on: Vec<(&Rider, bool)> = seats.iter().map(|s| (&s.rider, s.plane.is_none())).collect();
            let rivals = rider_contact::rivals_among(&on, 1, &*world);
            drop(on);
            let controls = seats[1].poll(dt, &p2_pads(t));
            let said = seats[1].step_among(dt, &controls, world, bump.as_mut(), &rivals);
            if let Some(b) = &bump {
                rider_contact::bumped_among(&mut seats, b);
                for f in &b.planes {
                    if let Some(p) = seats[f.seat].plane.as_mut() {
                        p.pushed(f);
                    }
                }
            }
            let p = seats[0].plane.as_ref().unwrap();
            let ch = &seats[1].rider.ride.chassis;
            let state = [three(p.body.velocity), three(p.body.position)];
            let (first, substeps) = first_substep(&first, state);
            out.push(Tick {
                t,
                state,
                crash: p.crash.reason,
                p2: [three(ch.vel), three(ch.cofg_world)],
                p2_rebuilt: three(rebuilt_centre(&seats[1].rider.ride)),
                p2_wreck: said.wreck.map(|w| w.0),
                first,
                substeps,
                wreck: seats[1].rider.crash.clone().map(Box::new),
                plane_list: p.record.contacts.clone(),
                plane_body: p.body,
            });
        }
        Some((seed, out))
    })
}

/// **The centre a ride's next integrate rebuilds first**: the chassis' local centre through the
/// transform the last step left, as `chassis_integrate`'s first call writes it (ride.rs's
/// `orientation_point`, the same madds). A row read inside integrate carries it.
fn rebuilt_centre(ride: &Ride) -> Vf {
    use ps2_float::{Dest, Field};
    let (m, v) = (ride.transform(), ride.chassis.cofg);
    let acc = m.0[3].add(Vf::new(0.0, 0.0, 0.0, 1.0).bc(Field::X));
    let acc = Vf::madd(acc, m.0[0], v.bc(Field::X));
    let acc = Vf::madd(acc, m.0[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m.0[2], v.bc(Field::Z)))
}

/// The wreck bike's part and the rider's wreck set off the disc (`tests/replay.rs`'s `the_ragdoll_kit`).
fn wreck_files(disc: &mut Disc) -> Option<(phy::Part, phy::Wrecks)> {
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let rider = names.iter().filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len())?.clone();
    let rider_file = disc.res(&rider).ok()?.read_suffix("mrider.phy").ok()?;
    let wreck_bike = names.iter().find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbike.phy").ok()))?;
    Some((phy::Part::read_bytes(&wreck_bike, "PhysicsPart1"), phy::Wrecks::read_bytes(&rider_file)))
}

/// **A rider down as the image has them** (S274): the wreck entered with `reason` ([`Crash::begin`]),
/// then the wreck bike's body and collision object as the vehicle's wreck part holds them
/// (`*(rider + 0x7a8)`: `+0x110` the body, `+0x120` the collision object, research/s274), the hands
/// off the bars and the rider down. Needs the rider's wreck files ([`Rider::carries_wreck`]).
fn wrecked_p2(ee: &Ee, rider: u32, reason: u32, r: &mut Rider) {
    use game_sim::ride::Crash;
    let part = ee.u32_at(rider + 0x7a8);
    let (b, c) = (ee.u32_at(part + 0x110), ee.u32_at(part + 0x120));
    let w = r.wreckage.as_ref().expect("the rider's wreck files");
    let wreck = game_sim::trick::Wreck(reason as u8);
    let mut crash = Crash::begin(&r.ride, wreck, &w.part, w.inertia, &w.wrecks, None);
    let bike = &mut crash.bike;
    read_body(ee, b, &mut bike.body);
    bike.collider.cofg_world = quad(ee, c + 0xa0);
    bike.collider.normal_speed = F(ee.f32_at(c + 0x1d8));
    bike.collider.relative = quad(ee, c + 0x1e0);
    let rec = &mut bike.record;
    let flag = |o: u32| ee.u32_at(c + o) != 0;
    (rec.touching, rec.touching_other, rec.on_ground, rec.on_object) = (flag(0x40), flag(0x44), flag(0x48), flag(0x4c));
    (rec.swept_from, rec.swept_to) = (quad(ee, c + 0xd0), quad(ee, c + 0xe0));
    let n = ee.u32_at(c + 0xf8);
    rec.node = game_sim::contact::Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) };
    crash.origin = crash.bike.origin();
    (crash.holding, crash.landed) = (false, true);
    crash.clock = crash.throw.hand_hold_time;
    r.state.wreck = Some(wreck);
    r.crash = Some(crash);
}

/// The original's rigid body at `b` read into `body` (the plane's and the wreck bike's layout,
/// [`seeded_plane`]); what the layout does not hold stays as it was.
fn read_body(ee: &Ee, b: u32, body: &mut game_sim::biplane::Body) {
    body.enabled = ee.u32_at(b) != 0;
    body.inv_mass = F(ee.f32_at(b + 0x10));
    for (v, o) in [
        (&mut body.damping, 0x20),
        (&mut body.last_position, 0x50),
        (&mut body.position, 0x60),
        (&mut body.velocity, 0x70),
        (&mut body.last_force, 0x80),
        (&mut body.force, 0x90),
        (&mut body.accel, 0xa0),
        (&mut body.shock_peak, 0xb0),
        (&mut body.shock, 0xc0),
        (&mut body.step, 0xf0),
        (&mut body.inv_inertia, 0x120),
        (&mut body.ang_damping, 0x130),
        (&mut body.last_quat, 0x150),
        (&mut body.quat, 0x170),
        (&mut body.cofg, 0x180),
        (&mut body.moment, 0x190),
        (&mut body.last_moment, 0x1a0),
        (&mut body.rates, 0x1b0),
        (&mut body.ang_accel, 0x1c0),
        (&mut body.twist_peak, 0x1d0),
        (&mut body.twist, 0x1e0),
    ] {
        *v = quad(ee, b + o);
    }
}

/// How many of the port's ticks the capture confirms, bit for bit (tests/fly_and_collide.rs's rule):
/// each row from the image's frame on reads one of the port's states in order, the image's then each
/// tick's, or repeats the one before, or skips one, or is torn between two axis by axis. A row that is
/// none of those parts the run. A row may also be read inside a tick's frame, after its first substep
/// (T8: row 137911 is the plane after 137910's first, S274), or torn between that and either end.
fn held(cap: &Rows, seed: [[u32; 3]; 2], ticks: &[Tick]) -> usize {
    let states: Vec<[[u32; 3]; 2]> = std::iter::once(seed).chain(ticks.iter().map(|k| k.state)).collect();
    let firsts: Vec<[[u32; 3]; 2]> = std::iter::once(seed).chain(ticks.iter().map(|k| k.first)).collect();
    held_in(cap, &states, &firsts, ticks[0].t, ticks.last().unwrap().t, |r| cap.state(r))
}

/// [`held`] on P2's chassis, velocity and centre. A row may also be read inside the next tick's
/// integrate, after it rebuilt the centre from the transform and before it wrote the new one: the
/// state's velocity with [`Tick::p2_rebuilt`] (S275: row 137829 is tick 137827's velocity and
/// rebuilt centre to the bit, and from 137810 P2 parted there before).
fn p2_held(cap: &Rows, seed: [[u32; 3]; 2], ticks: &[Tick]) -> usize {
    let states: Vec<[[u32; 3]; 2]> = std::iter::once(seed).chain(ticks.iter().map(|k| k.p2)).collect();
    let inside: Vec<[[u32; 3]; 2]> = std::iter::once(seed).chain(ticks.iter().map(|k| [k.p2[0], k.p2_rebuilt])).collect();
    held_in(cap, &states, &inside, ticks[0].t, ticks.last().unwrap().t, |r| [cap.v3(r, "p2_ch_vel"), cap.v3(r, "p2_ch_centre")])
}

/// [`held`]'s rule on any states read off the rows by `read`, where state `j`'s tick also stands at
/// `firsts[j]` inside its frame.
fn held_in(
    cap: &Rows,
    states: &[[[u32; 3]; 2]],
    firsts: &[[[u32; 3]; 2]],
    from: u32,
    last: u32,
    read: impl Fn(u32) -> [[u32; 3]; 2],
) -> usize {
    let mut at = 0;
    for r in from..=last + 2 {
        if cap.row(r).is_none() {
            continue;
        }
        let row = read(r);
        let reads = |j: usize| {
            let pair = [states[j.saturating_sub(1)], firsts[j], states[j]];
            (0..2).all(|v| (0..3).all(|a| pair.iter().any(|s| s[v][a] == row[v][a])))
        };
        // A row read between a put-back's placing and its integrate: the velocity zeroed, the
        // position the one before.
        let placed = |j: usize| j > 0 && row[0] == [0; 3] && row[1] == states[j - 1][1];
        let window = at..(at + 3).min(states.len());
        match window.clone().find(|&j| reads(j)).or_else(|| window.clone().find(|&j| placed(j))) {
            Some(j) => at = j,
            None if r <= from + 1 && at == 0 => {}
            None => break,
        }
    }
    while at + 1 < states.len() && states[at + 1] == states[at] {
        at += 1;
    }
    at
}

/// The first frame from `t` on whose row reads the plane's crash word non-zero, and the word.
fn crashed_from(cap: &Rows, t: u32, to: u32) -> Option<(u32, u32)> {
    (t..to).filter(|&f| cap.row(f).is_some()).map(|f| (f, cap.u(f, "crash"))).find(|&(_, c)| c != 0)
}

/// **Which tape offset the taxi holds longest under** (S228's search): from the state's image, each of
/// the five frames after it as the first field's poll. Run with `--ignored` to repeat it.
#[test]
#[ignore]
fn the_tape_offset_the_taxi_holds_longest_under() {
    let Some(cap) = capture() else { return };
    for zero in 137403..=137407 {
        let Some((seed, ticks)) = fly(137402, 137700, zero) else { return };
        println!("zero {zero}: held {} of {}", held(&cap, seed, &ticks), ticks.len());
    }
}

/// Each event of research/c16/index.md: the image it flies from, the frame it flies to, and how many
/// of its ticks the capture confirms (S272's pins: where the port parts from the original). All but the
/// joust hold to the end; the joust parts on the hit (137853's step, row 137855), because the Sim's
/// plane lane stands P2 still — [`the_plane_jousts_p2_as_the_original`] flies both seats.
const EVENTS: [(&str, u32, u32, usize); 7] = [
    ("the state: switch, taxi", 137402, 137845, 443),
    ("the joust on P2 (P2 wreck 7 at 137857)", 137845, 137900, 8),
    ("take-off at 138061 and the climb", 138050, 138200, 150),
    ("inverted ground crash at 139204", 139190, 139260, 70),
    ("the 300-tick put-back at 139504", 139490, 139560, 70),
    ("down inverted, through the put-back", 139355, 139560, 205),
    ("46 ft/s on static scenery at 139913..139918, no crash", 139900, 139960, 60),
];

/// **Plane 0 flies each event as the original**, as far as [`EVENTS`] pins: from the image before
/// each, on the tape's pads and the capture's frame times, the body's velocity and position are the
/// capture's rows bit for bit ([`held`]) for the pinned number of ticks; the crash word turns on the
/// original's tick where the run holds through it.
#[test]
fn the_plane_flies_each_event_as_the_original() {
    let Some(cap) = capture() else { return };
    let mut got = Vec::new();
    for (what, from, to, _) in EVENTS {
        let Some((seed, ticks)) = fly(from, to, FIELD_ZERO) else { return };
        let n = held(&cap, seed, &ticks);
        let port = ticks.iter().find(|k| k.crash != 0).map(|k| (k.t + 1, k.crash));
        let theirs = crashed_from(&cap, from, to);
        let parted = ticks.get(n).map(|k| k.t);
        println!("{what}: {from}..{to} holds {n} of {} ticks (parts on tick {parted:?}); crash port {port:?}, original {theirs:?}", ticks.len());
        if let Some(k) = ticks.get(n) {
            let r = k.t + 1;
            if cap.row(r).is_some() {
                let f = |b: [u32; 3]| b.map(f32::from_bits);
                println!("    port vel {:?} pos {:?}", f(k.state[0]), f(k.state[1]));
                println!("    row  vel {:?} pos {:?}", f(cap.state(r)[0]), f(cap.state(r)[1]));
            }
        }
        // A crash the run holds through turns on on the original's frame (one already on in the
        // image is the image's).
        if let Some((at, _)) = theirs.filter(|&(at, _)| at > from && at < from + n as u32) {
            assert_eq!(port, theirs, "{what}: the crash at {at}");
        }
        got.push(n);
    }
    let want: Vec<usize> = EVENTS.iter().map(|e| e.3).collect();
    assert_eq!(got, want, "the ticks each event holds");
}

/// The first frame from `t` on whose row reads P2 wrecked, and the reason.
fn p2_wrecked_from(cap: &Rows, t: u32, to: u32) -> Option<(u32, u32)> {
    (t..to).filter(|&f| cap.row(f).is_some()).map(|f| (f, cap.u(f, "p2_wreck_reason"))).find(|&(_, r)| r != 0)
}

/// **The joust: plane 0 taxis into P2's parked rider and wrecks it, as the original** (C16's
/// 137857, reason 7). In the split screen's pass ([`fly_two`]) from the image before it, the plane's
/// body and P2's chassis against the capture's rows ([`held`], [`p2_held`]), and P2's wreck reason
/// and frame against the first row that reads it. Here the rows lag a frame more than the taxi's: the
/// port's state after 137855's step is row 137857's, the wreck's row. Pinned ([`JOUSTS`]): from
/// 137845 P2 holds to the end, through the hit (P2's ray reaching 4.58 on the plane's hull, past the
/// 50cc limit: rider_contact::wreck_contacts) and the wreck, and the plane to the end (F35: its rays
/// strike P2's wreck bike's hull); from 137810 both to the end, P2 standing until the hit (S275).
#[test]
fn the_plane_jousts_p2_as_the_original() {
    let Some(cap) = capture() else { return };
    let mut got = Vec::new();
    for (from, to, _, _) in JOUSTS {
        let Some(([plane, p2], ticks)) = fly_two(from, to, FIELD_ZERO) else { return };
        let n = held(&cap, plane, &ticks);
        let m = p2_held(&cap, p2, &ticks);
        let port = ticks.iter().find_map(|k| k.p2_wreck.map(|w| (k.t + 1, w)));
        let theirs = p2_wrecked_from(&cap, from, to);
        let parted = ticks.get(n).map(|k| k.t);
        println!("joust from {from}: the plane holds {n} (parts on tick {parted:?}), P2 {m} of {} ticks; P2's wreck port {port:?} (its frame), original {theirs:?} (first row)", ticks.len());
        for k in ticks.iter().filter(|k| (137850..137860).contains(&k.t)) {
            let f = |b: [u32; 3]| b.map(f32::from_bits);
            println!("    {} plane vel {:?} p2 vel {:?} wreck {:?}", k.t + 1, f(k.state[0]), f(k.p2[0]), k.p2_wreck);
        }
        if from < 137857 {
            assert_eq!(theirs, Some((137857, 7)), "the original's wreck");
            assert_eq!(port.map(|(f, w)| (f + 1, u32::from(w))), theirs, "from {from}: P2's wreck, a row late");
        }
        got.push((n, m));
    }
    let want: Vec<(usize, usize)> = JOUSTS.iter().map(|j| (j.2, j.3)).collect();
    assert_eq!(got, want, "the ticks the plane and P2 hold");
}

/// The joust's runs: from, to, and the ticks the plane and P2 hold. 137910's seeds P2 down
/// ([`wrecked_p2`]); its wreck bike holds, and since F32 the plane too, to the end: the wreck bike is
/// a body of the plane's world, its resolve after the plane's each substep (research/f30), and its
/// first tick holds on row 137911, read after that tick's first substep ([`held`], T8). From 137845
/// the plane holds through the wreck (it parted on tick 137856 before F32), and since F35 to the end:
/// it parted on tick 137863 (18), where its ray 33 strikes P2's wreck bike's hull (research/r31),
/// which is in its world now. From 137810 it holds to the end too (53 before F35), and P2 with it
/// (17 before S275: row 137829 was read inside 137828's integrate, [`p2_held`]). Pinned on C25's
/// capture.
const JOUSTS: [(u32, u32, usize, usize); 3] = [(137810, 137900, 90, 90), (137845, 137900, 55, 55), (137910, 138010, 100, 100)];

/// The whole tape as images every 100 frames cut it: each stretch from its image to the next, and
/// how many of its ticks hold (S272's pins). One parts: 137810's on the joust's hit (137853's step),
/// the Sim's plane lane standing P2 still. 137910's parted on its second tick, the plane bouncing on
/// P2's wreck. Since S276 [`fly`] seeds P2 down there and its wreck bike meets the plane in the Sim's lane
/// ([`the_sim_lane_meets_p2s_wreck_bike`]). Its first tick holds on a row read after the first
/// substep (T8, [`row_137911_is_the_wreck_bikes_first_substep`]), and since F32, the wreck bike
/// stepped in the plane's world, the rest of it holds too. Pinned on C25's capture.
const STRETCHES: [usize; 29] = [
    8, 100, 100, 100, 100, 43, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
    100, 100, 100, 100,
];

/// **Plane 0 flies the whole tape as the original**, as far as [`STRETCHES`] pins: from 137402 to
/// 137410, then from each image every 100 frames to the next, to 140210, as
/// [`the_plane_flies_each_event_as_the_original`] flies each event (the Sim's plane lane, P2 standing).
#[test]
fn the_plane_flies_the_tape_as_the_original() {
    let Some(cap) = capture() else { return };
    let cuts: Vec<u32> = std::iter::once(137402).chain((137410..=140210).step_by(100)).collect();
    let mut got = Vec::new();
    for w in cuts.windows(2) {
        let Some((seed, ticks)) = fly(w[0], w[1], FIELD_ZERO) else { return };
        let n = held(&cap, seed, &ticks);
        println!("{}..{}: holds {n} of {} (parts on tick {:?})", w[0], w[1], ticks.len(), ticks.get(n).map(|k| k.t));
        if let Some(k) = ticks.get(n) {
            let f = |b: [u32; 3]| b.map(f32::from_bits);
            println!("    port vel {:?} pos {:?}", f(k.state[0]), f(k.state[1]));
            println!("    row  vel {:?} pos {:?}", f(cap.state(k.t + 1)[0]), f(cap.state(k.t + 1)[1]));
        }
        got.push(n);
    }
    assert_eq!(got, STRETCHES, "the ticks each stretch holds");
}

/// What the tape holds on 137910's ticks, without the tape (research/c16/index.md's segment list,
/// `150:cross` from field 472, frame [`FIELD_ZERO`]` + 472`): Cross held at full pressure, the sticks
/// centred. And the 2p state's step, 1/60 s.
fn cross_held(t: u32) -> Report {
    assert!((FIELD_ZERO + 472..FIELD_ZERO + 622).contains(&t), "tick {t} is outside the tape's 150 fields of Cross");
    Report::from_buttons(0x0040)
}

/// **The tape's pads without the tape** (S274): research/c16's segment list for fly.p2m2,
/// `30 60:cross+l1 262:l1 120 150:cross 980 110:-:127,0 420 90:cross+r1 400:r1 200`, each segment
/// (fields, buttons, the left stick) written into a field as research/c16/make_tape.py writes it, and
/// read as [`P2m2`] reads one ([`game_sim::replay::report_of_p2m2`]). Tick `t` polls field
/// `t - `[`FIELD_ZERO`], the first field before it; port 2 is idle on every field.
fn segment_pad(t: u32) -> Report {
    const CROSS: u8 = 0x40;
    const L1: u8 = 0x04;
    const R1: u8 = 0x08;
    const SEGMENTS: [(u32, u8, u8); 11] =
        [(30, 0, 127), (60, CROSS | L1, 127), (262, L1, 127), (120, 0, 127), (150, CROSS, 127), (980, 0, 127), (110, 0, 0), (420, 0, 127), (90, CROSS | R1, 127), (400, R1, 127), (200, 0, 127)];
    let mut n = t.saturating_sub(FIELD_ZERO);
    let &(_, buttons, ly) = SEGMENTS
        .iter()
        .find(|&&(fields, _, _)| n < fields || {
            n -= fields;
            false
        })
        .unwrap_or_else(|| panic!("tick {t} is past the tape's 2822 fields"));
    // make_tape.py's field: the two button bytes active low, the sticks (right x y, left x y), then
    // the twelve pressures, Cross's at 6, L1's at 8, R1's at 9.
    let mut field = [0u8; game_sim::replay::P2M2_PAD_BYTES];
    field[..6].copy_from_slice(&[0xff, !buttons, 127, 127, 127, ly]);
    for (bit, at) in [(CROSS, 6), (L1, 8), (R1, 9)] {
        if buttons & bit != 0 {
            field[6 + at] = 0xff;
        }
    }
    game_sim::replay::report_of_p2m2(&field)
}

/// [`cross_held`] to its segment's end, then the 980 idle fields after it.
fn cross_then_idle(t: u32) -> Report {
    assert!((FIELD_ZERO + 472..FIELD_ZERO + 1602).contains(&t), "tick {t} is outside the tape's Cross and idle segments");
    if t < FIELD_ZERO + 622 { Report::from_buttons(0x0040) } else { Report::idle() }
}

/// **Away from the wreck, the wreck changes nothing** (S276): [`fly`] seeds P2 down on 138010's
/// stretch too (the capture has P2 wrecked to 138012), and the plane, taxiing away from the wreck bike
/// and taking off, flies it exactly as with P2 standing, on which [`STRETCHES`] pinned it.
#[test]
fn the_wreck_bike_leaves_a_far_plane_alone() {
    let dt = |_| F(1.0 / 60.0);
    let Some((_, down)) = fly_by(138010, 138110, &dt, &cross_then_idle, &|_| false, 7) else { return };
    let Some((_, standing)) = fly_by(138010, 138110, &dt, &cross_then_idle, &|_| false, 0) else { return };
    let planes = |ks: &[Tick]| ks.iter().map(|k| k.state).collect::<Vec<_>>();
    assert_eq!(planes(&down), planes(&standing), "the plane with P2 down, as with P2 standing");
}

/// **The Sim's plane lane meets a downed P2's wreck bike** (S276): from 137910's image, P2 down with
/// the capture's wreck 7 ([`wrecked_p2`]), [`fly`]'s lane (the Sim's [`game_sim::Sim::step`], which
/// steps no rider but the plane) flies the plane as [`fly_two`]'s split-screen pass does, bit for bit
/// over ten ticks: P2's wreck bike's search, its mirror on the plane's list and its step run in the
/// Sim as in the shell ([`game_sim::wreck_plane::search_downed`], [`game_sim::wreck_plane::step_downed`]).
/// Needs only the image and the disc: the tape's pad there is Cross held ([`cross_held`]). Its first
/// tick is S274's to the last bits of z (-24.854322, 8.510814, 2.1630852 against S274's 2.1630855,
/// which stepped at the capture's dt and not 1/60), short of the original's row 137911, (-24.94349, 8.60006, 2.21980). The wreck bike, which nothing
/// else steps in this lane, goes on moving.
#[test]
fn the_sim_lane_meets_p2s_wreck_bike() {
    let dt = |_| F(1.0 / 60.0);
    let Some((_, down)) = fly_by(137910, 137920, &dt, &cross_held, &|_| false, 7) else { return };
    let f = |b: [u32; 3]| b.map(f32::from_bits);
    for k in &down {
        println!("{}: plane vel {:?} pos {:?}; wreck bike vel {:?}", k.t + 1, f(k.state[0]), f(k.state[1]), f(k.p2[0]));
    }
    let Some((_, two)) = fly_two_by(137910, 137920, &dt, &cross_held, &|_| Report::idle(), &|_| false, 7) else { return };
    for k in &two {
        println!("{}: fly_two plane vel {:?} pos {:?}", k.t + 1, f(k.state[0]), f(k.state[1]));
    }
    let planes = |ks: &[Tick]| ks.iter().map(|k| k.state).collect::<Vec<_>>();
    assert_eq!(planes(&down), planes(&two), "the plane in the Sim's lane, as fly_two's");
    assert!(down.windows(2).all(|w| w[0].p2[1] != w[1].p2[1]), "the wreck bike steps in the plane's lane");
}

/// **Row 137911 is read inside 137910's frame** (S274): from 137910's image, P2 down with wreck 7,
/// the plane meeting P2's wreck bike's mirror on its list, the port's plane at the end of the tick's
/// first substep ([`game_sim::flight::substeps`]) is the original's row 137911 to the capture's printed
/// digits, at the image's `frame_dt` (0x3c888882; 0x3c88ce6a and 1/60 land there too), and the tick's end is not: the row is torn, PINE reading between the frame's two steps
/// (S196's three phases). With P2 standing the substep is not on it, so the mirror's push is the
/// original's. Why [`held`] takes a row read after a tick's first substep (T8).
#[test]
fn row_137911_is_the_wreck_bikes_first_substep() {
    let dt = |_| F(f32::from_bits(0x3c888882));
    let row = ["-24.94349", "8.60006", "2.21980"];
    let digits = |v: Vf| [0, 1, 2].map(|k| format!("{:.5}", v.0[k]));
    let first = |reason| fly_by(137910, 137911, &dt, &segment_pad, &|_| false, reason).map(|(_, ticks)| ticks[0].clone());
    let vel = |bits: [u32; 3]| digits(Vf::new(f32::from_bits(bits[0]), f32::from_bits(bits[1]), f32::from_bits(bits[2]), 0.0));
    let Some(tick) = first(7) else { return };
    assert_eq!(tick.substeps, 2, "two substeps a tick");
    assert_eq!(vel(tick.first[0]), row, "the first substep, P2 down");
    assert_ne!(vel(tick.state[0]), row, "the tick's end is past the row");
    let Some(standing) = first(0) else { return };
    assert_ne!(vel(standing.first[0]), row, "the first substep, P2 standing");
}

/// **P2's wreck 7 by the tape's pads** (S274): from 137845's image, P1's pads the tape's segments
/// ([`segment_pad`]) and P2 idle, [`fly_two_by`]'s P2, struck by the plane on tick 137853, wrecks with
/// the capture's reason 7 on tick 137855, and nowhere else up to 137858.
#[test]
fn p2_wrecks_7_on_the_tapes_pads() {
    let dt = |_| F(1.0 / 60.0);
    let Some((_, two)) = fly_two_by(137845, 137858, &dt, &segment_pad, &|_| Report::idle(), &|_| false, 0) else { return };
    let wrecked: Vec<_> = two.iter().filter_map(|k| k.p2_wreck.map(|w| (k.t, w))).collect();
    assert_eq!(wrecked, [(137855, 7)], "P2's wreck, by tick");
}

/// What [`fly_with_far`] keeps of a tick: the near plane's speed and spot, P2's wreck bike's, and
/// whether the near plane's rays struck the wreck bike's hull (F36: an entry of its own naming another
/// side on its list, and the mirror at the head of the wreck bike's).
type NearTick = ([[u32; 3]; 2], [[u32; 3]; 2], bool);

/// **P2's wreck bike under plane 0, alone or flown together with a far plane** (S276a), from
/// 137910's image as [`fly_by`]: P2 down with wreck 7, Cross held, nothing stepping P2 but the
/// planes. `far` `None` flies plane 0 alone ([`game_sim::wreck_plane::search_downed`],
/// [`Flight::frame_met`], [`game_sim::wreck_plane::step_downed`]); `Some(first)` flies it with a
/// second plane 400 ft above, out of everyone's reach, in one [`flight::frames_together`]
/// ([`game_sim::wreck_plane::search_downed_by`], [`game_sim::wreck_plane::step_downed_by`]), plane 0
/// first among them or second. From image `from` to tick `to`, P1's pad `pad`.
fn fly_with_far(from: u32, to: u32, pad: &dyn Fn(u32) -> Report, far: Option<bool>) -> Option<Vec<NearTick>> {
    use game_sim::flight::{Fly, Met};
    use game_sim::wreck_plane;
    let dt = F(1.0 / 60.0);
    let ee = image(from)?;
    let (plane, rays, hull) = machines()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let tables = Arc::new(Tables::build());
    let (phy, col) = bike(&mut disc, &tables);
    let col = Arc::new(col);
    let mesh = bike_box(&mut disc);
    let mut other = standing_p2(&ee, &phy, &col);
    let (part, wrecks) = wreck_files(&mut disc).expect("the rider's wreck files on the disc");
    other.carries_wreck(part, wrecks, game_sim::ride::WRECK_INERTIA);
    wrecked_p2(&ee, p2(&ee), 7, &mut other);
    let me = standing_p2(&ee, &phy, &col);
    let mut sim = Sim::with_rider(0x5272, Arc::clone(&tables), phy, me);
    let wreck = wreck_hull(&mut disc);
    in_free09_ss(&mut disc, |world, area| {
        let mut near = seeded_plane(&ee, &plane, &rays, &hull, mesh.as_ref(), area, &tables);
        if let Some(w) = wreck.clone() {
            near = near.with_wreck_hull(w);
        }
        let mut high = near.clone();
        let up = near.spawn_origin().add(Vf::new(0.0, 400.0, 0.0, 0.0));
        high.place(up, Vf::new(0.0, 0.0, 1.0, 0.0), &tables.ride.acos);
        sim.poll(dt, &[pad(from - 1), Report::missing()]);
        let mut out = Vec::new();
        for t in from..to {
            sim.poll(dt, &[pad(t), Report::missing()]);
            let pad = flight::pad(sim.input.pad(0));
            let mut riders = vec![&mut other];
            let (mut m0, mut m1) = (Met::default(), Met::default());
            match far {
                None => {
                    let downed = wreck_plane::search_downed(&mut riders, &near, dt, &mut *world, &mut m0);
                    let _ = near.frame_met(dt, pad, world, &tables.ride, &[], Some(&mut m0));
                    wreck_plane::step_downed(&mut riders, &mut near, downed, &mut m0, dt, &*world, false);
                }
                Some(first) => {
                    let (a, b) = if first { (&mut near, &mut high) } else { (&mut high, &mut near) };
                    let downed = wreck_plane::search_downed_by(&mut riders, &[&*a, &*b], dt, &mut *world, &mut [&mut m0, &mut m1]);
                    let mut fleet = [
                        Fly { flight: &mut *a, pad, met: Some(&mut m0), bikes: &[] },
                        Fly { flight: &mut *b, pad, met: Some(&mut m1), bikes: &[] },
                    ];
                    let _ = flight::frames_together(dt, &mut fleet, world, &tables.ride);
                    wreck_plane::step_downed_by(&mut riders, &mut [a, b], downed, &mut [&mut m0, &mut m1], dt, &*world, false);
                }
            }
            let c = other.crash.as_ref().expect("P2 stays down");
            let struck = near.record.contacts.iter().any(|e| e.kind == 0 && e.other.is_some());
            let mirrored = c.bike.record.contacts.first().is_some_and(|e| e.kind == 1);
            assert_eq!(struck, mirrored, "{t}: the plane's strike on the hull and its mirror at the head of the wreck bike's list");
            out.push(([three(near.body.velocity), three(near.body.position)], [three(c.bike.body.velocity), three(c.bike.body.position)], struck));
        }
        Some(out)
    })
}

/// **Planes flown together meet a downed rider's wreck bike** (S276a): with a second plane far
/// above, flown in one joint frame, plane 0 and P2's wreck bike go bit for bit as plane 0 alone
/// takes them ([`fly_with_far`]), plane 0 first among the fleet or second — the wreck bike's search
/// with both hulls, its mirror on plane 0's list alone, one body the joint resolve pushes, and its
/// step with both planes' bodies. That the lone plane meets it at all is
/// [`the_sim_lane_meets_p2s_wreck_bike`]'s. Since F36 the planes' rays meet the wreck bike's hull in
/// the joint frame as a lone plane's do (F35): from 137856's image on the tape's pads, the plane's ray
/// strikes it (R31's tick 137863, ray 33), its entry on the plane's list and the mirror at the head of
/// the wreck bike's, and the two go on as alone.
#[test]
fn planes_flown_together_meet_p2s_wreck_bike() {
    let runs: [(u32, u32, &dyn Fn(u32) -> Report); 2] = [(137910, 137920, &cross_held), (137856, 137870, &segment_pad)];
    for (from, to, pad) in runs {
        let Some(alone) = fly_with_far(from, to, pad, None) else { return };
        for (t, k) in (from..).zip(&alone) {
            println!("{t} alone: plane {:?}; wreck bike {:?}; struck {}", k.0.map(|v| v.map(f32::from_bits)), k.1.map(|v| v.map(f32::from_bits)), k.2);
        }
        if from == 137856 {
            assert!(alone.iter().any(|k| k.2), "from {from} the plane's rays reach the wreck bike's hull");
        }
        for first in [true, false] {
            let Some(together) = fly_with_far(from, to, pad, Some(first)) else { return };
            assert_eq!(together, alone, "from {from}, plane 0 {} the far plane: as alone", if first { "ahead of" } else { "after" });
        }
    }
}

/// **F30's probe: P2's wreck bike, the original's images against the port's**, 137854..137858.
/// Each image holds the state after the tick before its frame ([`fly_by`] seeds tick `from` off
/// image `from`); the port is [`fly_two`] from 137845. Prints the plane's body and the wreck bike's
/// (`*(P2 + 0x7a8) + 0x110`) both ways. Run with `--ignored`.
#[test]
#[ignore]
fn f30_wreck_bike_images_against_the_port() {
    let Some(cap) = capture() else { return };
    let Some((_, ticks)) = fly_two(137845, 137860, FIELD_ZERO) else { return };
    if std::env::var_os("F30_CRASH").is_some() {
        let mut c = ticks.iter().find(|k| k.t == 137855).unwrap().wreck.clone().unwrap();
        c.bike.record.contacts.clear();
        println!("{c:#?}");
    }
    let v = |x: Vf| format!("({:?}, {:?}, {:?}, {:?})", x.0[0], x.0[1], x.0[2], x.0[3]);
    for f in 137854..=137858 {
        let Some(ee) = image(f) else { return };
        let plane = quad(&ee, ee.u32_at(plane0(&ee) + plane_at::BODY) + 0x70);
        let part = ee.u32_at(p2(&ee) + 0x7a8);
        println!("image {f}: plane vel {}; row {f} vel {:?}", v(plane), cap.state(f)[0].map(f32::from_bits));
        let port = ticks.iter().find(|k| k.t + 1 == f).unwrap();
        println!("  port after {}: plane vel {:?}", port.t, port.state[0].map(f32::from_bits));
        if part == 0 {
            println!("  no wreck part");
            continue;
        }
        let mut theirs = game_sim::biplane::Body::default();
        read_body(&ee, ee.u32_at(part + 0x110), &mut theirs);
        let c = ee.u32_at(part + 0x120);
        if let Some(w) = &port.wreck {
            let n = ee.u32_at(c + 0xf8);
            let bits = |x: Vf| x.0.map(f32::to_bits);
            for k in 0..4 {
                let theirs = bits(quad(&ee, n + 0x80 + 16 * k));
                let ours = bits(if k < 3 { w.bike.record.node.rows[k as usize] } else { w.bike.record.node.origin });
                println!("  {} node row {k}: {theirs:08x?} | {ours:08x?}", if theirs == ours { ' ' } else { '*' });
            }
        }
        if let Some(w) = &port.wreck {
            let pb = ee.u32_at(plane0(&ee) + plane_at::BODY);
            let wb = ee.u32_at(part + 0x110);
            let plane_obj = (0x0010_0000..0x01ff_0000u32).step_by(16).map(|a| a + 0x0).find(|&a| ee.u32_at(a + 0xf4) == pb && ee.u32_at(a + 0x108) != 0);
            let name = |o: u32| if o == 0 { "-".to_string() } else if o == c { "wreck".into() } else if Some(o) == plane_obj { "plane".into() } else { format!("{o:#x}") };
            println!("  wreck body {wb:#x} object +0xf4 {:#x}; plane object {plane_obj:x?}", ee.u32_at(c + 0xf4));
            let show = |what: &str, obj: u32, ours: &[game_sim::contact::Contact]| {
                let n = ee.u32_at(obj + 0x100);
                let at = ee.u32_at(obj + 0x108);
                println!("  {what}'s list: original {n}, port {}", ours.len());
                for k in 0..n.max(ours.len() as u32) {
                    if k < n {
                        let e = at + 0x80 * k;
                        println!("    orig {k}: kind {} on {} n {} pt {} other {} mover {:#x} surf {}", ee.u32_at(e), v(quad(&ee, e + 0x20)), v(quad(&ee, e + 0x30)), v(quad(&ee, e + 0x40)), name(ee.u32_at(e + 0x74)), ee.u32_at(e + 0x78), ee.u32_at(e + 0x70) as i32);
                    }
                    if let Some(o) = ours.get(k as usize) {
                        println!("    port {k}: kind {} on {} n {} pt {} other {:?} mover {:?} surf {}", o.kind, v(o.on_body), v(o.normal), v(o.point), o.other, o.mover, o.surface);
                    }
                }
            };
            show("wreck", c, &w.bike.record.contacts);
            if let Some(po) = plane_obj {
                show("plane", po, &port.plane_list);
            }
        }
        if std::env::var_os("F30_RAW").is_some() {
            let n = ee.u32_at(c + 0xf8);
            for (what, at, len) in [("object", c, 0x200), ("node", n, 0xc0)] {
                println!("  {what} at {at:#x}:");
                for o in (0..len).step_by(16) {
                    let w: Vec<String> = (0..4).map(|k| format!("{:>08x} {:<12}", ee.u32_at(at + o + 4 * k), ee.f32_at(at + o + 4 * k))).collect();
                    println!("    +{o:#05x} {}", w.join(" "));
                }
            }
        }
        println!("  wreck object: cofg_world {} touching {} {} relative {}", v(quad(&ee, c + 0xa0)), ee.u32_at(c + 0x40), ee.u32_at(c + 0x44), v(quad(&ee, c + 0x1e0)));
        let fields = |b: &game_sim::biplane::Body| {
            let mut out = vec![("enabled", format!("{}", b.enabled)), ("inv_mass", format!("{:?}", b.inv_mass.0))];
            for (n, x) in [
                ("damping", b.damping),
                ("last_position", b.last_position),
                ("position", b.position),
                ("velocity", b.velocity),
                ("last_force", b.last_force),
                ("force", b.force),
                ("accel", b.accel),
                ("shock_peak", b.shock_peak),
                ("shock", b.shock),
                ("step", b.step),
                ("inv_inertia", b.inv_inertia),
                ("ang_damping", b.ang_damping),
                ("last_quat", b.last_quat),
                ("quat", b.quat),
                ("cofg", b.cofg),
                ("moment", b.moment),
                ("last_moment", b.last_moment),
                ("rates", b.rates),
                ("ang_accel", b.ang_accel),
                ("twist_peak", b.twist_peak),
                ("twist", b.twist),
            ] {
                out.push((n, v(x)));
            }
            out
        };
        let a = fields(&theirs);
        let b = port.wreck.as_ref().map(|o| fields(&o.bike.body)).unwrap_or_default();
        for (k, (n, x)) in a.iter().enumerate() {
            let o = b.get(k).map_or("-", |p| p.1.as_str());
            println!("  {} {n:<13} {x:<58} | {o}", if *x == o { ' ' } else { '*' });
        }
    }
}

/// **F32: the plane and P2's wreck bike after the wreck are the original's images**: [`fly_two`]
/// from 137845, the wreck bike stepped in the plane's world (its resolve after the plane's each
/// substep, research/f30), and the port's plane after each tick 137854..137857 is image `t + 1`'s
/// body bit for bit, velocity and position, as is the wreck bike's body once P2 is down. Before F32
/// the plane parted on tick 137856 (-28.609848, -0.08526538, 3.6421182 against the image's
/// -28.616682, -0.05351289, 3.6458206). Skips without the images (`research/f30/index.md`).
#[test]
fn f32_the_plane_and_wreck_bike_are_the_images() {
    let Some((_, ticks)) = fly_two(137845, 137860, FIELD_ZERO) else { return };
    let bits = |v: Vf| [0, 1, 2].map(|k| v.0[k].to_bits());
    let f = |b: [u32; 3]| b.map(f32::from_bits);
    let mut wrecks = 0;
    for t in 137854..=137857 {
        let Some(ee) = image(t + 1) else { return };
        let mut plane = game_sim::biplane::Body::default();
        read_body(&ee, ee.u32_at(plane0(&ee) + plane_at::BODY), &mut plane);
        let port = ticks.iter().find(|k| k.t == t).expect("the tick");
        assert_eq!(port.state, [bits(plane.velocity), bits(plane.position)], "the plane after {t}: port vel {:?}, image {:?}", f(port.state[0]), f(bits(plane.velocity)));
        let part = ee.u32_at(p2(&ee) + 0x7a8);
        let (Some(w), true) = (&port.wreck, part != 0) else { continue };
        let mut theirs = game_sim::biplane::Body::default();
        read_body(&ee, ee.u32_at(part + 0x110), &mut theirs);
        let ours = &w.bike.body;
        assert_eq!([bits(ours.velocity), bits(ours.position)], [bits(theirs.velocity), bits(theirs.position)], "the wreck bike after {t}");
        wrecks += 1;
    }
    assert!(wrecks >= 2, "P2's wreck bike down on the last ticks");
}

/// **F30: the original's own tick 137856 off image 137856**, the level's collision node's update run
/// by the oracle (tests/plane_meets_plane.rs's way), each contact resolve logged: whose list, which
/// substep. The plane's and the wreck bike's bodies after it are image 137857's, bit for bit, and the
/// wreck bike resolves between the plane's two substeps (F30). Run with `--ignored`; skips without
/// the images (`research/f30/index.md`).
#[test]
#[ignore]
fn f30_the_originals_tick_137856() {
    const NODE_UPDATE: u32 = 0x0026_f800;
    const RESOLVE: u32 = 0x0027_0dc0;
    let (Some(mut ee), Some(after)) = (image(137856), image(137857)) else { return };
    let plane_body = ee.u32_at(plane0(&ee) + plane_at::BODY);
    let part = ee.u32_at(p2(&ee) + 0x7a8);
    let (wreck_body, wreck_obj) = (ee.u32_at(part + 0x110), ee.u32_at(part + 0x120));
    // The node: an object list (count at +0x30, the array at +0x38) holding the wreck bike's object.
    let ram = ee.ram.len() as u32;
    let array = (0x0010_0000..ram - 4).step_by(4).filter(|&a| ee.u32_at(a) == wreck_obj).collect::<Vec<_>>();
    let mut node = None;
    for a in &array {
        for back in 0..32u32 {
            let start = a - 4 * back;
            for n in (0x0010_0000..ram - 0x40).step_by(4) {
                if ee.u32_at(n + 0x38) == start && (back..back + 16).contains(&ee.u32_at(n + 0x30)) && ee.u32_at(n + 0x30) > back {
                    node = Some(n);
                }
            }
            if node.is_some() {
                break;
            }
        }
        if node.is_some() {
            break;
        }
    }
    let node = node.expect("the node listing the wreck bike's object");
    let objects: Vec<u32> = (0..ee.u32_at(node + 0x30)).map(|k| ee.u32_at(ee.u32_at(node + 0x38) + 4 * k)).collect();
    let names = |o: u32| if o == wreck_obj { "wreck".to_string() } else if ee.u32_at(o + 0xf4) == plane_body { "plane".into() } else { format!("{o:#x} (body {:#x})", ee.u32_at(o + 0xf4)) };
    println!("node {node:#x}: objects {:?}", objects.iter().map(|&o| names(o)).collect::<Vec<_>>());
    let named: Vec<(u32, String)> = objects.iter().map(|&o| (o, names(o))).collect();
    let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = std::rc::Rc::clone(&log);
    ee.hooks.insert(RESOLVE, Box::new(move |ee: &mut Ee| {
        let (r, first) = (ee.r(4) as u32, ee.r(5) as u32);
        let who = named.iter().find(|(o, _)| *o == r).map_or(format!("{r:#x}"), |(_, n)| n.clone());
        seen.borrow_mut().push(format!("{who} first {first} list {}", ee.u32_at(r + 0x100)));
        let (ra, sp, stack) = (ee.r(31), ee.r(29), ee.stack);
        ee.stack = (sp as u32 - 0x400) & !0xf;
        ee.call(RESOLVE, &[u64::from(r), u64::from(first)], &[], 50_000_000);
        ee.stack = stack;
        ee.set(29, sp);
        ee.set(31, ra);
        ee.pc = RESOLVE;
    }));
    // The tick's own step is the next image's frame_dt (the frame writes it before its update).
    let dt = after.f32_at(addr("frame_dt").unwrap());
    println!("frame_dt {dt:?} ({:#x}); image 137857's {:#x}", dt.to_bits(), after.f32_at(addr("frame_dt").unwrap()).to_bits());
    ee.call(NODE_UPDATE, &[u64::from(node)], &[dt], 2_000_000_000);
    ee.hooks.clear();
    for l in log.borrow().iter() {
        println!("  resolve {l}");
    }
    let v = |x: Vf| format!("({:?}, {:?}, {:?})", x.0[0], x.0[1], x.0[2]);
    let mut off = Vec::new();
    for (what, b) in [("plane", plane_body), ("wreck", wreck_body)] {
        for (n, o) in [("velocity", 0x70), ("position", 0x60), ("rates", 0x1b0), ("quat", 0x170), ("last_force", 0x80), ("last_moment", 0x1a0)] {
            let (x, y) = (quad(&ee, b + o), quad(&after, b + o));
            let same = x.0.map(f32::to_bits) == y.0.map(f32::to_bits);
            println!("  {} {what} {n:<11} oracle {:<52} image {}", if same { ' ' } else { '*' }, v(x), v(y));
            if !same {
                off.push(format!("{what} {n}"));
            }
        }
    }
    // The node's bodies resolve one after another each substep, the wreck bike after the plane:
    // one dynamics world, not the plane's frame and then the wreck bike's (the port's, S276).
    let order: Vec<String> = log.borrow().iter().map(|l| l.split(" first").next().unwrap_or("").to_string()).collect();
    let (pi, wi) = (order.iter().position(|w| w == "plane"), order.iter().position(|w| w == "wreck"));
    let again = order.iter().skip(wi.map_or(0, |w| w + 1)).position(|w| w == "plane");
    assert!(pi < wi && again.is_some(), "the wreck bike resolves between the plane's substeps: {order:?}");
    assert!(off.is_empty(), "the oracle's tick 137856 is image 137857's but for {off:?}");
}

/// The original's body at `b` and the port's `ours`, field by field ([`read_body`]'s layout): each
/// line `*` where the bits differ. Returns the fields that differ.
fn body_diff(ee: &Ee, b: u32, ours: &game_sim::biplane::Body, show: bool) -> Vec<&'static str> {
    let mut theirs = game_sim::biplane::Body::default();
    read_body(ee, b, &mut theirs);
    let v = |x: Vf| format!("({:?}, {:?}, {:?}, {:?})", x.0[0], x.0[1], x.0[2], x.0[3]);
    let fields = |b: &game_sim::biplane::Body| {
        [
            ("velocity", b.velocity),
            ("position", b.position),
            ("last_position", b.last_position),
            ("last_force", b.last_force),
            ("force", b.force),
            ("accel", b.accel),
            ("shock_peak", b.shock_peak),
            ("shock", b.shock),
            ("quat", b.quat),
            ("last_quat", b.last_quat),
            ("moment", b.moment),
            ("last_moment", b.last_moment),
            ("rates", b.rates),
            ("ang_accel", b.ang_accel),
            ("twist_peak", b.twist_peak),
            ("twist", b.twist),
        ]
    };
    let mut off = Vec::new();
    for ((n, x), (_, o)) in fields(&theirs).into_iter().zip(fields(ours)) {
        // the w lanes are not the port's to keep on every field: x, y, z only
        let same = (0..3).all(|k| x.0[k].to_bits() == o.0[k].to_bits());
        if !same {
            off.push(n);
        }
        if show && !same {
            println!("    * {n:<13} image {:<58} port {}", v(x), v(o));
        }
    }
    off
}

/// **R31's probe: the joust from 137845 against images 137855..137866.** [`fly_two`] from 137845;
/// after each tick `t` the port's plane body and P2's wreck bike's body against image `t + 1`, field by
/// field, and the two contact lists' lengths and kinds. Run with `--ignored`; skips without the images
/// (`research/r31/index.md`).
#[test]
#[ignore]
fn r31_the_joust_against_images_to_137866() {
    let Some(cap) = capture() else { return };
    let Some((_, ticks)) = fly_two(137845, 137866, FIELD_ZERO) else { return };
    let v = |x: Vf| format!("({:?}, {:?}, {:?})", x.0[0], x.0[1], x.0[2]);
    // The plane's ray segments with an end at x = -7.5, where the original's 137863 strike on the
    // wreck bike stands on the plane (-7.5, 2.3962276, -1.6266332).
    let Some((_, rays, _)) = machines() else { return };
    for (k, s) in rays.segments.iter().enumerate().filter(|(_, s)| s.from.0[0] == -7.5 || s.to.0[0] == -7.5) {
        println!("plane ray {k}: {} -> {}", v(s.from), v(s.to));
    }
    // What the ray strikes: the wreck part's mesh collidable (class 2, shape kind 0, its record the
    // wreck part's collision object), its mesh's root box against the disc's two bike meshes.
    if let (Some(ee), Ok(mut disc)) = (image(137864), Disc::open_configured()) {
        let c = ee.u32_at(ee.u32_at(p2(&ee) + 0x7a8) + 0x120);
        let ram = ee.ram.len() as u32;
        for a in (0x0010_0000..ram - 0x300).step_by(4).filter(|&a| ee.u32_at(a + 0x80) == c && ee.ram[a as usize + 2] == 2 && ee.u32_at(a + 0x68) == 0) {
            let mesh = ee.u32_at(a + 0x234);
            println!("wreck mesh collidable {a:#x}: category {:#x}, flags {:#x}, root centre {} half {}", ee.u32_at(a + 0x84), ee.ram[a as usize + 0x116], v(quad(&ee, mesh + 0xc)), v(quad(&ee, mesh + 0x18)));
        }
        let names: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().ends_with(".res")).collect();
        for file in ["wreckbikef.col", "bike\\col\\bikef.col"] {
            let Some(m) = names.iter().find_map(|n| disc.res(n).ok()?.read_suffix(file).ok()).and_then(|b| track::Mesh::parse(&b).ok()) else { continue };
            let n = &m.nodes[0];
            println!("{file}: {} vertices, root centre {:?} half {:?}", m.vertices.len(), n.centre.map(|x| x.0), n.half_extent.map(|x| x.0));
        }
    }
    for t in 137854..=137865u32 {
        let Some(ee) = image(t + 1) else { continue };
        let port = ticks.iter().find(|k| k.t == t).expect("the tick");
        let pb = ee.u32_at(plane0(&ee) + plane_at::BODY);
        let plane_off = body_diff(&ee, pb, &port.plane_body, true);
        println!("after {t} (image {}): plane {} | row {} vel {:?}", t + 1, if plane_off.is_empty() { "bit for bit".into() } else { format!("{plane_off:?}") }, t + 1, cap.state(t + 1)[0].map(f32::from_bits));
        let part = ee.u32_at(p2(&ee) + 0x7a8);
        match (&port.wreck, part) {
            (None, 0) => println!("  no wreck either side"),
            (None, _) => println!("  the original has a wreck part, the port none"),
            (Some(_), 0) => println!("  the port has a wreck, the original no part"),
            (Some(w), _) => {
                let off = body_diff(&ee, ee.u32_at(part + 0x110), &w.bike.body, true);
                println!("  wreck bike {}", if off.is_empty() { "bit for bit".into() } else { format!("{off:?}") });
                let c = ee.u32_at(part + 0x120);
                let plane_obj = (0x0010_0000..0x01ff_0000u32).step_by(16).find(|&a| ee.u32_at(a + 0xf4) == pb && ee.u32_at(a + 0x108) != 0);
                let name = |o: u32| if o == 0 { "-".to_string() } else if o == c { "wreck".into() } else if Some(o) == plane_obj { "plane".into() } else { format!("{o:#x}") };
                let show = |what: &str, obj: u32, ours: &[game_sim::contact::Contact]| {
                    let n = ee.u32_at(obj + 0x100);
                    let at = ee.u32_at(obj + 0x108);
                    println!("  {what}'s list: original {n}, port {}", ours.len());
                    for k in 0..n.max(ours.len() as u32) {
                        if k < n {
                            let e = at + 0x80 * k;
                            println!("    orig {k}: kind {} on {} n {} pt {} other {} surf {}", ee.u32_at(e), v(quad(&ee, e + 0x20)), v(quad(&ee, e + 0x30)), v(quad(&ee, e + 0x40)), name(ee.u32_at(e + 0x74)), ee.u32_at(e + 0x70) as i32);
                        }
                        if let Some(o) = ours.get(k as usize) {
                            println!("    port {k}: kind {} on {} n {} pt {} other {:?} surf {}", o.kind, v(o.on_body), v(o.normal), v(o.point), o.other, o.surface);
                        }
                    }
                };
                show("wreck", c, &w.bike.record.contacts);
                if let Some(po) = plane_obj {
                    show("plane", po, &port.plane_list);
                }
            }
        }
    }
}

// --- N24: both players' planes flown into each other, on a two-pad tape ---------------------------------

/// N24's tape (research/n24/index.md): C16's state (P1 in plane 0), where P2 switches to plane 1 from
/// its own pause menu, then P2 creeps forward on a little throttle and P1 taxis plane 0 into plane 1's
/// tail at 32 ft/s and pushes it along. Its capture (watch/n24-meet.toml, both planes' blocks) and the
/// original's RAM in the ticks around the touch (`context/states/n24/n24-<frame>.ram`).
const MEET_TAPE: &str = "recordings/n24/meet.p2m2";
const MEET_CAPTURE: &str = "capture/n24/meet.csv";

fn meet_image(name: &str) -> Option<Ee> {
    let path = ctx(&format!("states/n24/n24-{name}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Ok(frame) = name.parse::<u32>() {
        assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "n24-{frame}.ram is not frame {frame}");
    }
    Some(ee)
}

/// Plane 1: the actor list's second entry, flown by P2 after its switch.
fn plane1(ee: &Ee) -> u32 {
    chain(ee, &[0x34, 0x34, 0x8, 0x0])
}

/// The player flying plane record `p` (its player record's index: 0 for P1, 1 for P2).
fn flown_by(ee: &Ee, p: u32) -> u32 {
    ee.u32_at(ee.u32_at(p + 0x20) + 0x30)
}

/// The level node's objects whose bodies are `bodies`, as (node, index of each in the node's array).
/// The node is not at S88's address in this tape's heap, so it is found: every word naming a record
/// whose +0xf4 is one of the bodies, then a node (+0x30 count, +0x38 array) whose array holds them.
fn node_slots(ee: &Ee, bodies: [u32; 2]) -> (u32, [u32; 2]) {
    let words: Vec<u32> = ee.ram.chunks_exact(4).map(|w| u32::from_le_bytes(w.try_into().unwrap())).collect();
    let ram = |a: u32| (a & 0x0fff_ffff) < ee.ram.len() as u32 && a & 3 == 0 && a >> 28 == 0;
    let records = bodies.map(|b| {
        let at: Vec<u32> = (0..words.len() as u32).filter(|&i| words[i as usize] == b && i >= 0x3d).map(|i| 4 * i - 0xf4).collect();
        at
    });
    let mut found = Vec::new();
    for (i, &w) in words.iter().enumerate() {
        let (count, array) = (w, words.get(i + 2).copied().unwrap_or(0));
        if !(2..64).contains(&count) || !ram(array) || array as usize + 4 * count as usize > ee.ram.len() {
            continue;
        }
        let objects: Vec<u32> = (0..count).map(|k| ee.u32_at(array + 4 * k)).collect();
        let index = |k: usize| objects.iter().position(|o| records[k].contains(o)).map(|j| j as u32);
        if let (Some(a), Some(b)) = (index(0), index(1)) {
            found.push((4 * i as u32 - 0x30, [a, b]));
        }
    }
    assert_eq!(found.len(), 1, "one node holds both planes' records: {found:x?}");
    found[0]
}
fn meet_capture() -> Option<Rows> {
    let path = ctx(MEET_CAPTURE)?;
    let file = std::io::BufReader::new(File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let keep = |c: &str| {
        let c = c.strip_prefix("plane1_").unwrap_or(c);
        c == "dt" || c == "crash" || c.starts_with("body_pos.") || c.starts_with("body_vel.")
    };
    let cap = game_sim::replay::two_p::read(file, keep).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    Some(Rows { first: cap.frames[0], cols: cap.columns.clone(), rows: cap.rows })
}

impl Rows {
    /// Plane `k`'s velocity and position on the row of `frame`, as bits.
    fn plane_state(&self, frame: u32, k: usize) -> [[u32; 3]; 2] {
        let pre = ["", "plane1_"][k];
        [self.v3(frame, &format!("{pre}body_vel")), self.v3(frame, &format!("{pre}body_pos"))]
    }
}

fn meet_tape() -> Option<P2m2> {
    let path = ctx(MEET_TAPE)?;
    Some(P2m2::parse(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e:?}", path.display())))
}

/// The frame whose tick polls the tape's first field: the state's + 2, as C16's ([`FIELD_ZERO`]; the
/// same state). Not searched after the tape's pause-menu stretch: the runs below hold through the
/// meeting on it, so the fields reach the ticks they hold on.
const MEET_ZERO: u32 = 137404;

/// Both planes' velocity and position, plane 0's then plane 1's.
type Pair = [[[u32; 3]; 2]; 2];

/// **Both planes by the port from image `from` through frame `to`**, one joint frame a tick
/// ([`flight::frames_together`], each with a `Met` of its own and no bikes: both players' bikes are
/// out of the world while they fly), plane `order[0]` searching and resolving first; each plane on its
/// player's tape field `t - zero` and stepping at row `t + 1`'s `dt`. The seed, then each tick's
/// planes.
fn fly_pair(from: u32, to: u32, zero: u32, order: [usize; 2]) -> Option<(Pair, Vec<Pair>)> {
    use game_sim::flight::{Fly, Met};
    let cap = meet_capture()?;
    let tape = meet_tape()?;
    let ee = meet_image(&from.to_string())?;
    let (plane, rays, hull) = machines()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let tables = Arc::new(Tables::build());
    let mesh = bike_box(&mut disc);
    let wreck = wreck_hull(&mut disc);
    let dt_of = |t: u32| F(cap.f((t + 1).max(cap.first as u32), "dt"));
    let pads = |t: u32| [0, 1].map(|port| tape.field(t.saturating_sub(zero) as usize, port));
    in_free09_ss(&mut disc, |world, area| {
        let mut planes = [plane0(&ee), plane1(&ee)].map(|p| {
            let f = seeded_plane_at(&ee, p, &plane, &rays, &hull, mesh.as_ref(), area, &tables);
            match wreck.clone() {
                Some(w) => f.with_wreck_hull(w),
                None => f,
            }
        });
        let state = |p: &[Flight; 2]| p.each_ref().map(|f| [three(f.body.velocity), three(f.body.position)]);
        let mut input = game_sim::input::Manager::new();
        // The pads the image's tick read, so the first tick's presses are not edges.
        input.tick(dt_of(from - 1), &pads(from - 1));
        let seed = state(&planes);
        let mut out = Vec::new();
        for t in from..to {
            let dt = dt_of(t);
            input.tick(dt, &pads(t));
            let pad = [0, 1].map(|k| flight::pad(input.pad(k)));
            let mut mets = [Met::default(), Met::default()];
            let [m0, m1] = &mut mets;
            let [a, b] = &mut planes;
            let (first, second) = if order[0] == 0 { ((a, m0, pad[0]), (b, m1, pad[1])) } else { ((b, m1, pad[1]), (a, m0, pad[0])) };
            let mut fleet = [
                Fly { flight: first.0, pad: first.2, met: Some(first.1), bikes: &[] },
                Fly { flight: second.0, pad: second.2, met: Some(second.1), bikes: &[] },
            ];
            let _ = flight::frames_together(dt, &mut fleet, world, &tables.ride);
            out.push(state(&planes));
        }
        Some((seed, out))
    })
}

/// How many of [`fly_pair`]'s ticks the capture confirms for plane `k`, by [`held_in`]'s rule, and the
/// rows it let through as read inside a frame's step. [`flight::substeps`] keeps no bodies for planes
/// flown together, so such a row (C16's T8) cannot be matched bit for bit here: a row that is none of
/// the port's states passes only when every axis lies between two successive states — the reached one
/// and the next, or for a second such row the next pair on — and a row after it is read exactly.
fn pair_held(cap: &Rows, from: u32, seed: Pair, ticks: &[Pair], k: usize) -> (usize, Vec<u32>) {
    let states: Vec<[[u32; 3]; 2]> = std::iter::once(seed[k]).chain(ticks.iter().map(|p| p[k])).collect();
    let last = from + ticks.len() as u32 - 1;
    let between = |row: [[u32; 3]; 2], a: [[u32; 3]; 2], b: [[u32; 3]; 2]| {
        (0..2).all(|v| {
            (0..3).all(|x| {
                let (r, a, b) = (f32::from_bits(row[v][x]), f32::from_bits(a[v][x]), f32::from_bits(b[v][x]));
                a.min(b) <= r && r <= a.max(b)
            })
        })
    };
    let (mut at, mut mid, mut pending) = (0, Vec::new(), Vec::new());
    for r in from..=last + 2 {
        if cap.row(r).is_none() {
            continue;
        }
        let row = cap.plane_state(r, k);
        let reads = |j: usize| {
            let pair = [states[j.saturating_sub(1)], states[j]];
            (0..2).all(|v| (0..3).all(|a| pair.iter().any(|s| s[v][a] == row[v][a])))
        };
        let window = at..(at + 3).min(states.len());
        match window.clone().find(|&j| reads(j)) {
            Some(j) => {
                at = j;
                mid.append(&mut pending);
            }
            None if r <= from + 1 && at == 0 => {}
            None if pending.len() < 2 && at + pending.len() + 1 < states.len() && {
                let m = at + pending.len();
                between(row, states[m], states[m + 1])
            } =>
            {
                pending.push(r)
            }
            None => {
                if std::env::var("N24_DEBUG").is_ok() {
                    let f = |b: [u32; 3]| b.map(f32::from_bits);
                    eprintln!("plane {k} parts on row {r} at state {at}: row {:?} {:?}", f(row[0]), f(row[1]));
                    for j in at.saturating_sub(1)..(at + 3).min(states.len()) {
                        eprintln!("    state {j}: {:?} {:?}", f(states[j][0]), f(states[j][1]));
                    }
                }
                break;
            }
        }
    }
    while at + 1 < states.len() && states[at + 1] == states[at] {
        at += 1;
    }
    (at, mid)
}

/// **P1's plane is plane 0, whoever switched first** (N24): on the meeting tape's image before the
/// touch (P1 switched first, C16's) and on `n24-order.ram` (P2 switched first, from C16's
/// fly_and_collide state: research/n24/index.md), the actor list's first plane is P1's and its second
/// P2's, and the level node's first two objects are those planes' bodies in that order — so the
/// node's plane 0, which searches first and resolves first in each substep, is always seat 0's.
#[test]
fn n24_p1s_plane_is_plane_0_whoever_switched_first() {
    for name in ["138200", "order"] {
        let Some(ee) = meet_image(name) else { return };
        let (a, b) = (plane0(&ee), plane1(&ee));
        assert_eq!([ee.u32_at(a), ee.u32_at(b)], [1, 1], "{name}: the list's first two entries are planes");
        assert_eq!([flown_by(&ee, a), flown_by(&ee, b)], [0, 1], "{name}: plane 0 is P1's, plane 1 P2's");
        let (node, slots) = node_slots(&ee, [a, b].map(|p| ee.u32_at(p + plane_at::BODY)));
        assert_eq!(slots, [0, 1], "{name}: the node at {node:#x} holds plane 0's object first, then plane 1's");
    }
}

/// Runs from the original's RAM around the meeting: (image frame, ticks held for plane 0 and plane 1
/// with plane 0 searching and resolving first, the same with plane 1 first), each of 100 ticks. P1's
/// plane rides 32 ft/s into the tail of P2's taxiing plane at 138217, hits it hard at 138221 and pushes
/// it along. Seat 0's plane first holds every tick, through the touch and the hit (two rows read inside
/// a frame at 138212-138213); the other order parts within a few ticks of the meeting, and before it at
/// 138500 where they touch again — so the original flies plane 0 first, and each plane's post refills
/// against the other's hull as the original's does.
const MEETS: [(u32, [usize; 2], [usize; 2]); 4] = [
    (138200, [100, 100], [19, 19]),
    (138213, [100, 100], [6, 6]),
    (138219, [100, 100], [0, 0]),
    (138500, [100, 100], [16, 16]),
];

/// **Two planes meeting hold in seat order, and only in it** (N24, [`MEETS`]).
#[test]
fn n24_planes_meet_as_the_original_flies_them() {
    let Some(cap) = meet_capture() else { return };
    for (from, first0, first1) in MEETS {
        for (order, want) in [([0, 1], first0), ([1, 0], first1)] {
            let Some((seed, ticks)) = fly_pair(from, from + 100, MEET_ZERO, order) else { return };
            let got = [0, 1].map(|k| pair_held(&cap, from, seed, &ticks, k).0);
            assert_eq!(got, want, "from {from}, plane {} first: ticks held", order[0]);
        }
    }
}

/// Prints each image's run both ways (`--ignored --nocapture`): what [`MEETS`] pins. 138000 and 138150
/// part in the taxi before the meeting where rows read a tick late, then on time, then late again
/// (torn in both planes alike, whichever order); 138225 parts on its third row, a torn read in the hit.
#[test]
#[ignore]
fn n24_meeting_holds() {
    let Some(cap) = meet_capture() else { return };
    let only: Option<u32> = std::env::var("N24_FROM").ok().and_then(|v| v.parse().ok());
    for from in [138000, 138150, 138200, 138210, 138213, 138215, 138217, 138219, 138221, 138225, 138230, 138260, 138300, 138400, 138500] {
        if only.is_some_and(|o| o != from) {
            continue;
        }
        for order in [[0, 1], [1, 0]] {
            let to = (from + 100).min(138630);
            let Some((seed, ticks)) = fly_pair(from, to, MEET_ZERO, order) else { return };
            let n = [0, 1].map(|k| pair_held(&cap, from, seed, &ticks, k));
            println!("{from}..{to} first {}: plane 0 holds {} (mid-frame rows {:?}), plane 1 {} ({:?}) of {}", order[0], n[0].0, n[0].1, n[1].0, n[1].1, ticks.len());
        }
    }
}

/// Prints the port's planes beside the rows from image `N24_FROM` (`--ignored --nocapture`).
#[test]
#[ignore]
fn n24_rows_beside_the_port() {
    let Some(cap) = meet_capture() else { return };
    let from: u32 = std::env::var("N24_FROM").ok().and_then(|v| v.parse().ok()).unwrap_or(138200);
    let Some((seed, ticks)) = fly_pair(from, from + 30, MEET_ZERO, [0, 1]) else { return };
    let f = |b: [u32; 3]| b.map(f32::from_bits);
    for (j, p) in std::iter::once(&seed).chain(&ticks).enumerate() {
        let r = from + j as u32;
        println!("after {r:>6}: port v0 {:?} v1 {:?}", f(p[0][0]), f(p[1][0]));
        println!("   row {r:>6}:      v0 {:?} v1 {:?}", f(cap.plane_state(r, 0)[0]), f(cap.plane_state(r, 1)[0]));
    }
}

/// What [`fly_by_parked`] keeps of a tick (S486a): the truck awake, its place and velocity, and the
/// plane's velocity.
#[derive(Clone, Debug)]
struct ParkedTick {
    awake: bool,
    truck: Vf,
    truck_vel: Vf,
    plane_vel: Vf,
}

/// **[`fly_by`]'s lane with free09's truck parked at `spot`** (S486a): `spot` handed the plane's
/// position and P2's wreck bike's origin (zero standing) in image `from`, the truck settled over the
/// ground there, its nose along `-z`, and in the ride ([`Sim::park_cars`]). `None` where the image or
/// the disc is not in this worktree.
fn fly_by_parked(from: u32, to: u32, pad_of: &dyn Fn(u32) -> Report, reason: u32, spot: &dyn Fn(Vf, Vf) -> Vf) -> Option<Vec<ParkedTick>> {
    use game_sim::car::{Car, CarModel, Kind, Parked};
    let dt = F(1.0 / 60.0);
    let ee = image(from)?;
    let (plane, rays, hull) = machines()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let truck = {
        let res = disc.res("machine.res").ok()?;
        let kind = Kind::MonsterTruck;
        let car = Car::read_bytes(&res.read(kind.phy()).ok()?, kind);
        let model = CarModel::read(&res.read("monstertruck\\anim\\monstertruck.psg").ok()?).ok()?;
        let rays = track::Rays::parse(&res.read(kind.rays()).ok()?).ok()?;
        let hull = track::Mesh::parse(&res.read(kind.hull()).ok()?).ok()?;
        (car, model, rays, hull)
    };
    let tables = Arc::new(Tables::build());
    let (phy, col) = bike(&mut disc, &tables);
    let col = Arc::new(col);
    let mesh = bike_box(&mut disc);
    let mut other = standing_p2(&ee, &phy, &col);
    if reason != 0 {
        let (part, wrecks) = wreck_files(&mut disc).expect("the rider's wreck files on the disc");
        other.carries_wreck(part, wrecks, game_sim::ride::WRECK_INERTIA);
        wrecked_p2(&ee, p2(&ee), reason, &mut other);
    }
    let me = standing_p2(&ee, &phy, &col);
    let mut sim = Sim::with_rider(0x5272, Arc::clone(&tables), phy.clone(), me);
    sim.add_player(phy, other);
    let wreck = wreck_hull(&mut disc);
    in_free09_ss(&mut disc, |world, area| {
        let flight = seeded_plane(&ee, &plane, &rays, &hull, mesh.as_ref(), area, &tables);
        let down = sim.others[0].rider.crash.as_ref().map_or(Vf::ZERO, |c| c.bike.origin());
        let at = spot(flight.body.position, down);
        sim.plane = Some(match wreck.clone() {
            Some(w) => flight.with_wreck_hull(w),
            None => flight,
        });
        let (car, model, rays, hull) = truck;
        let t = &tables.ride;
        let truck = Parked::settled(Arc::new(car), &model, Rays::of(&rays), Arc::new(hull), world, at, Vf::new(0.0, 0.0, -1.0, 0.0), &t.acos, &t.curve);
        assert!(!truck.awake(), "the truck settled and asleep");
        sim.park_cars(vec![truck]);
        sim.poll(dt, &[pad_of(from - 1), Report::missing()]);
        let mut out = Vec::new();
        for t in from..to {
            let controls = sim.poll(dt, &[pad_of(t), Report::missing()]);
            sim.step(dt, &controls, world);
            let p = sim.plane.as_ref().unwrap();
            let c = &sim.parked_cars[0];
            out.push(ParkedTick {
                awake: c.awake(),
                truck: c.machine.body.position,
                truck_vel: c.machine.body.velocity,
                plane_vel: p.body.velocity,
            });
        }
        Some(out)
    })
}

/// **The plane flown into a parked truck wakes it and shoves it its way** (S486a): from image 137910
/// the truck parked 30 ft ahead of the plane (along `-x`), asleep; the plane's rays reach its hull at
/// tick 42, and that tick the truck is awake with the plane's way in its velocity (about -11 ft/s in
/// x, the plane's -25 cut to -16). Before S486a the plane flew through.
/// R48 caught this meeting on the original (no truck in C16's 2p session; R48 booted its own): the
/// wake on the meeting tick, the shove along the plane's way and the plane's lost speed are held to it
/// by [`r48_the_plane_taxied_into_the_truck_wakes_it_on_the_originals_tick`], the original's shove
/// 1.33 times the plane's lost speed (14.0 of 10.5 ft/s); its truck does not sleep again while the
/// plane's throttle stays on it ([`r48_the_plane_keeps_pushing_the_truck`]).
#[test]
fn the_plane_flown_into_a_parked_truck_wakes_and_pushes_it() {
    let ahead = |plane: Vf, _: Vf| Vf::new(plane.x() - 30.0, plane.y(), plane.z() + 2.0, 1.0);
    let Some(ticks) = fly_by_parked(137910, 138030, &cross_then_idle, 0, &ahead) else { return };
    let hit = ticks.iter().position(|t| t.awake).expect("the plane met the truck");
    let (before, at) = (&ticks[hit - 1], &ticks[hit]);
    assert!((30..60).contains(&hit), "the plane reaches the truck about tick 42, got {hit}");
    assert!(at.truck_vel.x() < -5.0, "the truck shoved along the plane's -x: {:?}", at.truck_vel.0);
    assert!(at.plane_vel.x() > before.plane_vel.x() + 5.0, "the plane slowed by the truck: {:?} -> {:?}", before.plane_vel.0, at.plane_vel.0);
    assert!(ticks[hit + 1].truck.x() < at.truck.x(), "the truck moves on its own frame next tick");
}

// --- R48: the plane taxied into free09's parked truck, on the original -------------------------------
//
// C16's 2p session parks no truck (both players' machines are planes), so R48 booted its own free09
// for one player with the plane and the truck parked (research/r48/index.md): the human switched into
// the plane, put 45 ft south of the truck with its nose at the truck's, Cross for 160 fields (80 ticks)
// and idle after. `context/capture/r48/truck.csv` (watch/r48-truck.toml) has the plane and the truck's
// body a row a tick from frame 1488; `context/states/r48/r48-1487.ram` is the tape's state.

const R48_CAPTURE: &str = "capture/r48/truck.csv";
const R48_TAPE: &str = "recordings/r48/drive.p2m2";
/// The tape's state's frame: tick `R48_FROM` writes the capture's first row.
const R48_FROM: u32 = 1487;
/// The plane (the list's first entry, type 1) and the truck (car vtable 0x0045edac at +0x88) in R48's boot.
const R48_PLANE: u32 = 0x007c_cae0;
const R48_TRUCK: u32 = 0x0149_0240;
/// The human's bike, parked where the switch left it (the list's type-4 entry).
const R48_BIKE: u32 = 0x0156_13d0;

fn r48_image() -> Option<Ee> {
    let path = ctx(&format!("states/r48/r48-{R48_FROM}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), R48_FROM, "r48-{R48_FROM}.ram is not frame {R48_FROM}");
    Some(ee)
}

fn r48_capture() -> Option<Rows> {
    let path = ctx(R48_CAPTURE)?;
    let file = std::io::BufReader::new(File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let keep = |c: &str| ["dt", "crash", "plane_contacts", "cmd_34c", "truck_awake"].contains(&c) || ["body_pos.", "body_vel.", "truck_pos.", "truck_vel."].iter().any(|p| c.starts_with(p));
    let cap = game_sim::replay::two_p::read(file, keep).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    Some(Rows { first: cap.frames[0], cols: cap.columns.clone(), rows: cap.rows })
}

/// The one-player free09 as [`in_free09_ss`] reads the split one: `free09.res` and `free09_2.res`.
fn in_free09<R>(disc: &mut Disc, then: impl FnOnce(&mut Wooded<Place<'_>>, Area) -> R) -> R {
    let parts = ["Tracks\\free09.res", "Tracks\\free09_2.res"].map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e}")));
    let read = move |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| format!("no archive of this track has {want}"));
    let scene = Scene::parse(&read(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&bytes).expect("parse the terrain");
    let t = &scene.terrain;
    let area = Area::new(F(t.width as f32), F(t.breadth as f32), t.width_scale, t.breadth_scale);
    let ground = Ground::new(&terrain, &scene.terrain);
    let (props, _) = Props::load(&scene, |w: &str| read(w));
    let (trees, missed) = Trees::on_ground(&read(".esb").unwrap(), &ground, &CurveTable::build()).expect("the ecosystem");
    assert!(missed.is_empty(), "free09's trunks the port lacks: {missed:?}");
    then(&mut Wooded { world: Place { ground, props }, trees }, area)
}

/// What [`r48_fly`] keeps of a tick: [`ParkedTick`]'s words and the plane's throttle.
#[derive(Clone, Debug)]
struct R48Tick {
    awake: bool,
    truck: Vf,
    truck_vel: Vf,
    plane: Vf,
    plane_vel: Vf,
    throttle: f32,
}

/// **The Sim's plane lane on R48's boot** (one seat): the plane seeded off the tape's state
/// ([`seeded_plane_at`]), the human's rider standing at its parked bike, free09's truck parked at its
/// map spot (`[Monster2]`, (4688, 3838), nose `-z`, settled: [`Parked::settled`]), on the tape's pads
/// (two fields a tick; the first tick after the load reads the state's idle controls) and the
/// capture's frame times, ticks `R48_FROM..to`, the truck then put as the tape's state holds it
/// ([`r48_seat_truck`]). Also the settled truck's offset from the image's body and its turn.
fn r48_fly(to: u32) -> Option<(Vec<R48Tick>, Vf, Vf)> {
    use game_sim::car::{Car, CarModel, Kind, Parked};
    let ee = r48_image()?;
    let cap = r48_capture()?;
    let path = ctx(R48_TAPE)?;
    let tape = P2m2::parse(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    let pad = |t: u32| match t.checked_sub(R48_FROM + 1) {
        Some(n) => tape.field((2 * n as usize).min(tape.fields() - 1), 0),
        None => Report::idle(),
    };
    let (plane, rays, hull) = machines()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let truck = {
        let res = disc.res("machine.res").ok()?;
        let kind = Kind::MonsterTruck;
        let car = Car::read_bytes(&res.read(kind.phy()).ok()?, kind);
        let model = CarModel::read(&res.read("monstertruck\\anim\\monstertruck.psg").ok()?).ok()?;
        let rays = track::Rays::parse(&res.read(kind.rays()).ok()?).ok()?;
        let hull = track::Mesh::parse(&res.read(kind.hull()).ok()?).ok()?;
        (car, model, rays, hull)
    };
    let tables = Arc::new(Tables::build());
    let (phy, col) = bike(&mut disc, &tables);
    let mesh = bike_box(&mut disc);
    let mut me = Rider::new(&phy, game_sim::stock::MOUNTS, quad(&ee, R48_BIKE + 0x1b0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    me.no_start_line();
    me.carries_bike_col(Arc::new(col));
    let mut sim = Sim::with_rider(0x5272, Arc::clone(&tables), phy, me);
    let wreck = wreck_hull(&mut disc);
    let image_truck = ee.u32_at(R48_TRUCK + 0x160);
    let image_at = (quad(&ee, image_truck + 0x60), quad(&ee, image_truck + 0x170));
    in_free09(&mut disc, |world, area| {
        let flight = seeded_plane_at(&ee, R48_PLANE, &plane, &rays, &hull, mesh.as_ref(), area, &tables);
        sim.plane = Some(match wreck.clone() {
            Some(w) => flight.with_wreck_hull(w),
            None => flight,
        });
        let (car, model, rays, hull) = truck;
        let t = &tables.ride;
        let truck = Parked::settled(Arc::new(car), &model, Rays::of(&rays), Arc::new(hull), world, Vf::new(4688.0, 135.0, 3838.0, 1.0), Vf::new(0.0, 0.0, -1.0, 0.0), &t.acos, &t.curve);
        assert!(!truck.awake(), "the truck settled and asleep");
        let settled = (truck.machine.body.position, truck.machine.body.quat);
        let mut truck = truck;
        r48_seat_truck(&ee, &mut truck);
        sim.park_cars(vec![truck]);
        let dt = |t: u32| F(cap.f(t + 1, "dt"));
        sim.poll(dt(R48_FROM), &[pad(R48_FROM - 1), Report::missing()]);
        let mut out = Vec::new();
        for t in R48_FROM..to {
            let controls = sim.poll(dt(t), &[pad(t), Report::missing()]);
            sim.step(dt(t), &controls, world);
            let p = sim.plane.as_ref().unwrap();
            let c = &sim.parked_cars[0];
            out.push(R48Tick {
                awake: c.awake(),
                truck: c.machine.body.position,
                truck_vel: c.machine.body.velocity,
                plane: p.body.position,
                plane_vel: p.body.velocity,
                throttle: p.surfaces.throttle.0,
            });
        }
        let off = Vf::new(settled.0.x() - image_at.0.x(), settled.0.y() - image_at.0.y(), settled.0.z() - image_at.0.z(), 0.0);
        Some((out, off, settled.1))
    })
}

/// **R48's truck as the tape's state holds it** (S489a): its body, its four tires and its engine
/// taken off the image into the settled one, with its launch timer and boost (S489: the boost is
/// 1.886 on the image, and the truck's speed off the wake follows it). R48's truck is not where the load's settle left it: it
/// sleeps 0.09 ft higher, its springs 0.08 ft less pressed and its body still sinking at 0.29 ft/s
/// (something woke it after the load, and it slept again mid-bounce). A woken body takes that up
/// where it stopped, so the Sim's push is held to the original's only from the same truck.
fn r48_seat_truck(ee: &Ee, parked: &mut game_sim::car::Parked) {
    seat_truck(ee, R48_TRUCK, parked);
}

/// [`r48_seat_truck`] off the truck record `t` (R53's truck is at another address).
fn seat_truck(ee: &Ee, t: u32, parked: &mut game_sim::car::Parked) {
    let word = |at: u32| F(ee.f32_at(at));
    let flag = |at: u32| ee.u32_at(at) != 0;
    let m = &mut parked.machine;
    let b = ee.u32_at(t + 0x160);
    let body = &mut m.body;
    body.enabled = flag(b);
    for (v, o) in [
        (&mut body.last_position, 0x50),
        (&mut body.position, 0x60),
        (&mut body.velocity, 0x70),
        (&mut body.last_force, 0x80),
        (&mut body.force, 0x90),
        (&mut body.accel, 0xa0),
        (&mut body.shock_peak, 0xb0),
        (&mut body.shock, 0xc0),
        (&mut body.step, 0xf0),
        (&mut body.last_quat, 0x150),
        (&mut body.quat, 0x170),
        (&mut body.moment, 0x190),
        (&mut body.last_moment, 0x1a0),
        (&mut body.rates, 0x1b0),
        (&mut body.ang_accel, 0x1c0),
        (&mut body.twist_peak, 0x1d0),
        (&mut body.twist, 0x1e0),
    ] {
        *v = quad(ee, b + o);
    }
    for (k, w) in m.wheels.iter_mut().enumerate() {
        let at = ee.u32_at(t + 0x548 + 4 * k as u32);
        let s = &mut w.s;
        (s.grounded, s.low_speed_grip, s.slipping, s.sliding, s.locked) = (flag(at + 0xc0), flag(at + 0x1e0), flag(at + 0x1e4), flag(at + 0x1e8), flag(at + 0x1ec));
        s.handoff = flag(at + 0xc8).then(|| word(at + 0xcc));
        (s.compression, s.last_compression, s.load) = (word(at + 0xdc), word(at + 0xe0), word(at + 0xe4));
        (s.angle, s.spin, s.torque, s.last_torque) = (word(at + 0x1a0), word(at + 0x1a4), word(at + 0x1f0), word(at + 0x1f4));
        (s.slip_ratio, s.slip_angle, s.slip_tan) = (word(at + 0x20c), word(at + 0x210), word(at + 0x214));
        (s.steer, s.side_ratio, s.camber) = (word(at + 0x260), word(at + 0x2b0), word(at + 0x9bc));
        for (v, o) in [(&mut s.contact, 0x140), (&mut s.velocity, 0x220), (&mut s.last_velocity, 0x230), (&mut s.force, 0x240), (&mut s.last_force, 0x250)] {
            *v = quad(ee, at + o);
        }
        (w.solid, w.sorted, w.terrain_held, w.lone_front) = (flag(at + 0xb4), flag(at + 0xb8), flag(at + 0xbc), flag(at + 0xc4));
        w.probes = ee.u32_at(at + 0xd8) as i32;
        for (v, o) in [
            (&mut w.lever, 0xf0),
            (&mut w.ground_point, 0x110),
            (&mut w.ground_normal, 0x120),
            (&mut w.car_normal, 0x130),
            (&mut w.upper, 0x150),
            (&mut w.lower, 0x160),
            (&mut w.point_velocity, 0x170),
            (&mut w.body_velocity, 0x180),
            (&mut w.local_velocity, 0x190),
            (&mut w.probe_normal, 0x1d0),
        ] {
            *v = quad(ee, at + o);
        }
        w.frame = game_sim::math::Mat4([0, 16, 32, 48].map(|i| quad(ee, at + 0x270 + i)));
    }
    let c = |o: u32| t + o;
    m.last_cofg = quad(ee, c(0x250));
    m.ground_velocity = quad(ee, c(0x2a0));
    (m.latched, m.latch) = (flag(c(0x2bc)), flag(c(0x2c0)));
    (m.grounded, m.was_grounded) = (flag(c(0x530)), flag(c(0x534)));
    (m.ground_speed, m.alpha, m.side_slip) = (word(c(0x55c)), word(c(0x560)), word(c(0x564)));
    (m.world_rates, m.velocity, m.forward, m.up, m.euler) = (quad(ee, c(0x570)), quad(ee, c(0x580)), quad(ee, c(0x590)), quad(ee, c(0x5a0)), quad(ee, c(0x5b0)));
    m.matrix = game_sim::math::Mat4([0, 16, 32, 48].map(|i| quad(ee, c(0x5c0 + i))));
    (m.launch.timer, m.launch.start) = (word(c(0x42c)), word(c(0x430)));
    let e = ee.u32_at(c(0x544));
    let d = &mut m.drivetrain;
    d.launch_boost = word(e + 0x6c);
    d.gear = ee.u32_at(e + 0xcc) as i32;
    (d.ratio, d.inv_ratio, d.wheel_rpm) = (word(e + 0xe4), word(e + 0xe8), word(e + 0xec));
    (d.converter_torque, d.multiplication, d.output_torque) = (word(e + 0xf4), word(e + 0xf8), word(e + 0xfc));
    (d.cursor_index, d.cursor_frac) = (ee.u32_at(e + 0x104), word(e + 0x108));
    (d.engine_torque, d.braking_torque) = (word(e + 0x110), word(e + 0x114));
    d.drive_torque = [0x118, 0x11c, 0x120, 0x124].map(|o| word(e + o));
    (d.rpm, d.slip_rpm) = (word(e + 0x128), word(e + 0x12c));
}

/// The capture's word `w` (`body_pos`, `truck_vel`, ..) in row `f` as a vector.
fn r48_row(cap: &Rows, f: u32, w: &str) -> Vf {
    let c = |k: &str| cap.f(f, &format!("{w}.{k}"));
    Vf::new(c("x"), c("y"), c("z"), 0.0)
}

/// The meeting row: the first with the truck awake in R48's capture (the plane's rays on its hull).
const R48_MEET: u32 = 1569;

/// **R48's taxi on the Sim runs as the original's, to the truck** : the parked truck settles within
/// 0.15 ft of the original's (car_spawn.rs's check, here on the tape's own state), and on the tape's
/// pads the plane's throttle, place and velocity follow the capture row for row to the meeting (81
/// ticks, Cross ramping the throttle to 0.933 and holding it), the truck asleep throughout.
#[test]
fn r48_the_plane_taxis_to_the_truck_as_the_original() {
    let Some(cap) = r48_capture() else { return };
    let Some((ticks, off, _)) = r48_fly(R48_MEET - 1) else { return };
    assert!((0..3).all(|i| off.0[i].abs() < 0.15), "the truck settled off the original's by {:?}", off.0);
    for (k, t) in ticks.iter().enumerate() {
        let f = R48_FROM + 1 + k as u32;
        let near = |a: Vf, b: Vf, d: f32| (0..3).all(|i| (a.0[i] - b.0[i]).abs() < d);
        assert!((t.throttle - cap.f(f, "cmd_34c")).abs() < 1e-4, "row {f}: throttle {} vs {}", t.throttle, cap.f(f, "cmd_34c"));
        assert!(near(t.plane, r48_row(&cap, f, "body_pos"), 0.02), "row {f}: plane at {:?} vs {:?}", t.plane.0, r48_row(&cap, f, "body_pos").0);
        assert!(near(t.plane_vel, r48_row(&cap, f, "body_vel"), 0.1), "row {f}: plane moving {:?} vs {:?}", t.plane_vel.0, r48_row(&cap, f, "body_vel").0);
        assert!(!t.awake && cap.u(f, "truck_awake") == 0, "row {f}: the truck asleep before the meeting");
    }
}

/// The meeting tick's end in the original's replay ([`r48_the_originals_node_update_replays_the_push`]):
/// the plane's `z` speed and the truck's velocity. The capture's row 1569 is torn (read after the
/// first substep's impulse: plane 21.86, truck 14.02), so the Sim is held to these.
const R48_MEET_PLANE: f32 = 21.624;
const R48_MEET_TRUCK: [f32; 3] = [-0.406, -0.589, 14.535];

/// **The plane taxied into the parked truck wakes it and shoves it, on the original's tick** (R48):
/// row 1569 has the truck awake on both, and at the tick's end (the original's replayed, as the
/// row is torn) the truck shoved along the plane's `+z` at 14.54 ft/s and the plane's 32.4 ft/s cut to
/// 21.62, the Sim's within 0.02; the next row the truck has moved its way. The push after it:
/// [`r48_the_plane_keeps_pushing_the_truck`].
#[test]
fn r48_the_plane_taxied_into_the_truck_wakes_it_on_the_originals_tick() {
    let Some(cap) = r48_capture() else { return };
    let Some((ticks, _, _)) = r48_fly(R48_MEET + 1) else { return };
    let at = |f: u32| &ticks[(f - R48_FROM - 1) as usize];
    let woke = R48_FROM + 1 + ticks.iter().position(|t| t.awake).expect("the plane met the truck") as u32;
    assert_eq!(cap.u(R48_MEET - 1, "truck_awake"), 0);
    assert_eq!(cap.u(R48_MEET, "truck_awake"), 1);
    assert_eq!(woke, R48_MEET, "the truck wakes on the original's row");
    let t = at(R48_MEET);
    assert!((0..3).all(|i| (t.truck_vel.0[i] - R48_MEET_TRUCK[i]).abs() < 0.02), "the shove {:?} vs the original's {R48_MEET_TRUCK:?}", t.truck_vel.0);
    assert!((t.plane_vel.z() - R48_MEET_PLANE).abs() < 0.02, "the plane cut to {:?} vs the original's {R48_MEET_PLANE}", t.plane_vel.0);
    assert!(at(R48_MEET).plane_vel.z() < at(R48_MEET - 1).plane_vel.z() - 10.0, "the plane slowed by the truck");
    let moved = at(R48_MEET + 1).truck.z() - at(R48_MEET).truck.z();
    let moved_there = cap.f(R48_MEET + 1, "truck_pos.z") - cap.f(R48_MEET, "truck_pos.z");
    assert!(moved > 0.3 && moved_there > 0.3, "the truck moves its way next row: {moved} ft (the original's {moved_there})");
}

/// The last row [`r48_the_plane_keeps_pushing_the_truck`] holds the plane and the truck to the
/// capture's.
const R48_PUSH_THROUGH: u32 = 1604;

/// **The plane taxied into the truck keeps pushing it** (S487, S489a): the woken truck searches and
/// steps in the plane's dynamics world ([`game_sim::car::InPlane`]), and on a tick the plane strikes
/// it its controls drive, brakes off ([`game_sim::car::InPlane::struck`]). Through `R48_PUSH_THROUGH`
/// the plane and the truck follow the capture along `+z` (place within 0.3 ft, speed within 2.6 ft/s;
/// rows 1570-1579 within 0.01 ft and 0.6 ft/s: the rows 1571-1573 with no plane contact, the truck's
/// 18.9 ft/s kick at 1574, its climb to 25 by 1582), both awake to row 1640, and the truck driven past
/// z 3870 by row 1628 (the original's 3877). Since S489 the truck's own strikes on the plane's hull
/// keep its brakes off as well, and the truck follows the original's replay within 0.008 ft/s (each
/// tick's first substep) through row 1587. Not held: at row 1587 the plane's wing ray (its tip at `x` 7.5) meets the truck 0.0015 ft short of its
/// tip in the original and just misses it in the Sim; from there the push parts, the truck 1.7 ft/s
/// slow by row 1605.
#[test]
fn r48_the_plane_keeps_pushing_the_truck() {
    let Some(cap) = r48_capture() else { return };
    let Some((ticks, _, _)) = r48_fly(1640) else { return };
    let at = |f: u32| &ticks[(f - R48_FROM - 1) as usize];
    for f in R48_MEET..1640 {
        assert!(at(f).awake && cap.u(f, "truck_awake") == 1, "row {f}: the truck awake on both");
    }
    for f in R48_MEET + 1..=R48_PUSH_THROUGH {
        let t = at(f);
        let (p, pv) = (cap.f(f, "body_pos.z"), cap.f(f, "body_vel.z"));
        let (c, cv) = (cap.f(f, "truck_pos.z"), cap.f(f, "truck_vel.z"));
        assert!((t.plane.z() - p).abs() < 0.3 && (t.plane_vel.z() - pv).abs() < 2.6, "row {f}: plane z {} at {} vs {p} at {pv}", t.plane.z(), t.plane_vel.z());
        assert!((t.truck.z() - c).abs() < 0.3 && (t.truck_vel.z() - cv).abs() < 2.6, "row {f}: truck z {} at {} vs {c} at {cv}", t.truck.z(), t.truck_vel.z());
    }
    assert!(at(1628).truck.z() > 3870.0 && cap.f(1628, "truck_pos.z") > 3877.0, "the truck driven to {} by row 1628", at(1628).truck.z());
}

/// R48's world node in the tape's state: its count at `+0x30` and its objects' array at `+0x38`, the
/// plane's collision object (the plane's `+0x170`) first, the truck's (its `+0x170`) second.
const R48_NODE: u32 = 0x007d_0600;

/// **R48's push in the original, replayed** (S489): the original's node update (`0x26f800`) run on
/// the tape's state, each tick with the row's frame time and the plane's throttle word (`+0x34c`) off
/// the row, gives the capture's plane and truck (bodies' `+0x60`/`+0x70`) on every row 1489..=1640 but
/// the meeting's (row 1488 is the load's first tick, on the state's saved controls): row 1569 is torn, read between the plane's first resolve and the rest of the tick (the
/// replay's plane 21.62 ft/s and truck 14.54 at the tick's end; the row's 21.86 and 14.02 are what the
/// first substep's impulse left). It is the reference for the Sim's push: per tick 4 substeps of
/// "plane resolve, truck resolve" in one node, the plane's list its own entries then the mirrors of
/// the truck's, the truck's the plane's mirrors first (research/r48/index.md).
#[test]
fn r48_the_originals_node_update_replays_the_push() {
    const NODE_UPDATE: u32 = 0x0026_f800;
    let Some(cap) = r48_capture() else { return };
    let Some(mut ee) = r48_image() else { return };
    let objects = ee.u32_at(R48_NODE + 0x38);
    assert_eq!(ee.u32_at(R48_NODE + 0x30), 3, "R48's node holds the plane, the truck and one more");
    assert_eq!(ee.u32_at(objects), ee.u32_at(R48_PLANE + 0x170), "the plane's object first");
    assert_eq!(ee.u32_at(objects + 4), ee.u32_at(R48_TRUCK + 0x170), "the truck's second");
    let (plane, truck) = (ee.u32_at(R48_PLANE + 0x160), ee.u32_at(R48_TRUCK + 0x160));
    for f in R48_FROM + 1..=1640 {
        ee.set_f32(R48_PLANE + 0x34c, cap.f(f, "cmd_34c"));
        ee.call(NODE_UPDATE, &[u64::from(R48_NODE)], &[cap.f(f, "dt")], 2_000_000_000);
        let got = [quad(&ee, plane + 0x60), quad(&ee, plane + 0x70), quad(&ee, truck + 0x60), quad(&ee, truck + 0x70)];
        let want = ["body_pos", "body_vel", "truck_pos", "truck_vel"].map(|w| r48_row(&cap, f, w));
        if f == R48_FROM + 1 {
            // The first tick after the load ran on the state's saved controls: 0.002 ft off.
            continue;
        }
        if f == R48_MEET {
            assert!((got[1].z() - 21.62).abs() < 0.01 && (got[3].z() - 14.54).abs() < 0.01, "row {f}: plane {:?} truck {:?} at the tick's end", got[1].0, got[3].0);
            continue;
        }
        for (g, w) in got.iter().zip(&want) {
            assert!((0..3).all(|i| (g.0[i] - w.0[i]).abs() < 1e-3), "row {f}: {:?} vs the capture's {:?}", g.0, w.0);
        }
        assert_eq!(ee.u32_at(truck) != 0, cap.u(f, "truck_awake") == 1, "row {f}: the truck's awake word");
        // S489a: a tick the plane strikes the truck its enable's driven word outlives its controls
        // step, so its brakes (car +0x3f4/+0x3f8) come off; a tick it is not struck (1571) they're on.
        if f == R48_MEET + 1 || f == R48_MEET + 2 {
            assert_eq!(ee.f32_at(R48_TRUCK + 0x3f8), if f == R48_MEET + 1 { 0.0 } else { 1.0 }, "row {f}: the truck's rear brake");
        }
    }
}

// --- R53: P2's wreck bike rolled into free09's parked truck, on the original ---------------------------
//
// R53 booted free09 for two players with the plane and the truck parked (research/r53/index.md): P1
// switched into the plane (391 ft south of the truck, standing), P2 put 18 ft south of the truck facing
// it, its bike given 25 ft/s at the truck and its wreck test tripped by a poke (reason 4, the back
// hit). `context/capture/r53/wreck-truck.csv` (research/r53/r53-wreck-truck.toml) has a row a tick at
// 30 Hz from frame 348; `context/states/r53/r53-348.ram` is the wreck entered, its wreck body row 349's.
//
// Rows: tick `t` from the image ends on row `t + 1`, as R48's. Row 348 is read before the load, and
// row 349 a tick late (it holds the image's own state: its wreck body is 1.6 ft, two ticks, short of
// row 350's), so the first held row is 350. Row 358 is torn: its wreck columns repeat 357's.

const R53_CAPTURE: &str = "capture/r53/wreck-truck.csv";
/// The image's frame: tick `R53_FROM` writes row `R53_FROM + 1` (a late read, see above).
const R53_FROM: u32 = 348;
/// The row the truck wakes on, in the original: the wreck body's collision object its one contact.
const R53_WAKE: u32 = 358;
/// The first row the Sim parts from the capture ([`r53_the_sim_parts_from_the_original_on_the_bounce`]).
const R53_PARTS: u32 = 359;
/// The plane (the list's first entry, flown by P1), the truck (car vtable 0x0045edac at +0x88), P1's
/// rider and P2's (the list's type-4 entries) in R53's boot.
const R53_PLANE: u32 = 0x0060_4740;
const R53_TRUCK: u32 = 0x0133_9fd0;
const R53_P1: u32 = 0x0147_70e0;
const R53_P2: u32 = 0x0091_1c90;

fn r53_image() -> Option<Ee> {
    let path = ctx(&format!("states/r53/r53-{R53_FROM}.ram"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), R53_FROM, "r53-{R53_FROM}.ram is not frame {R53_FROM}");
    Some(ee)
}

fn r53_capture() -> Option<Rows> {
    let path = ctx(R53_CAPTURE)?;
    let file = std::io::BufReader::new(File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let keep = |c: &str| {
        ["dt", "truck_awake", "truck_contacts", "truck_contact0_kind", "truck_contact0_other", "truck_object", "wreck_object", "p2_reason"].contains(&c)
            || ["plane_pos.", "truck_pos.", "truck_vel.", "wreck_pos.", "wreck_vel."].iter().any(|p| c.starts_with(p))
    };
    let cap = game_sim::replay::two_p::read(file, keep).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    Some(Rows { first: cap.frames[0], cols: cap.columns.clone(), rows: cap.rows })
}

/// What [`r53_ride`] keeps of a tick: the truck awake, its place and velocity, the plane's place, and
/// P2's wreck body's place and velocity.
#[derive(Clone, Debug)]
struct R53Tick {
    awake: bool,
    truck: Vf,
    truck_vel: Vf,
    plane: Vf,
    wreck: Vf,
    wreck_vel: Vf,
}

/// **The Sim on R53's image**, ticks `R53_FROM..to` (each one's end is row `t + 1`'s state): P1 in
/// the plane seeded off it ([`seeded_plane_at`]) on idle pads, P1's rider standing at its bike, P2
/// down with reason 4 as the image has it ([`wrecked_p2`], its wreck clock the image's), free09's
/// truck parked at its map spot and put as the image holds it ([`seat_truck`]), on the capture's
/// frame times over the split free09.
fn r53_ride(to: u32) -> Option<Vec<R53Tick>> {
    use game_sim::car::{Car, CarModel, Kind, Parked};
    let ee = r53_image()?;
    let cap = r53_capture()?;
    let (plane, rays, hull) = machines()?;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let truck = {
        let res = disc.res("machine.res").ok()?;
        let kind = Kind::MonsterTruck;
        let car = Car::read_bytes(&res.read(kind.phy()).ok()?, kind);
        let model = CarModel::read(&res.read("monstertruck\\anim\\monstertruck.psg").ok()?).ok()?;
        let rays = track::Rays::parse(&res.read(kind.rays()).ok()?).ok()?;
        let hull = track::Mesh::parse(&res.read(kind.hull()).ok()?).ok()?;
        (car, model, rays, hull)
    };
    let tables = Arc::new(Tables::build());
    let (phy, col) = bike(&mut disc, &tables);
    let col = Arc::new(col);
    let mesh = bike_box(&mut disc);
    let mut other = standing_at(&ee, R53_P2, &phy, &col);
    let (part, wrecks) = wreck_files(&mut disc).expect("the rider's wreck files on the disc");
    other.carries_wreck(part, wrecks, game_sim::ride::WRECK_INERTIA);
    assert_eq!(ee.u32_at(R53_P2 + 0x748), 4, "P2's wreck reason in the image");
    wrecked_p2(&ee, R53_P2, 4, &mut other);
    {
        // The image is the entry's own tick: the wreck clock one frame in, the hands on the bars.
        let crash = other.crash.as_mut().unwrap();
        crash.clock = F(ee.f32_at(R53_P2 + 0x794));
        crash.holding = crash.clock < crash.throw.hand_hold_time;
        crash.landed = false;
    }
    let me = standing_at(&ee, R53_P1, &phy, &col);
    let mut sim = Sim::with_rider(0x5272, Arc::clone(&tables), phy.clone(), me);
    sim.add_player(phy, other);
    let wreck = wreck_hull(&mut disc);
    in_free09_ss(&mut disc, |world, area| {
        let flight = seeded_plane_at(&ee, R53_PLANE, &plane, &rays, &hull, mesh.as_ref(), area, &tables);
        sim.plane = Some(match wreck.clone() {
            Some(w) => flight.with_wreck_hull(w),
            None => flight,
        });
        let (car, model, rays, hull) = truck;
        let t = &tables.ride;
        let mut truck = Parked::settled(Arc::new(car), &model, Rays::of(&rays), Arc::new(hull), world, Vf::new(4688.0, 135.0, 3838.0, 1.0), Vf::new(0.0, 0.0, -1.0, 0.0), &t.acos, &t.curve);
        assert!(!truck.awake(), "the truck settled and asleep");
        seat_truck(&ee, R53_TRUCK, &mut truck);
        sim.park_cars(vec![truck]);
        let dt = |t: u32| F(cap.f(t + 1, "dt"));
        sim.poll(dt(R53_FROM), &[Report::idle(), Report::missing()]);
        let mut out = Vec::new();
        for t in R53_FROM..to {
            let controls = sim.poll(dt(t), &[Report::idle(), Report::missing()]);
            sim.step(dt(t), &controls, world);
            let c = &sim.parked_cars[0];
            let w = &sim.others[0].rider.crash.as_ref().expect("P2 still down").bike.body;
            out.push(R53Tick {
                awake: c.awake(),
                truck: c.machine.body.position,
                truck_vel: c.machine.body.velocity,
                plane: sim.plane.as_ref().unwrap().body.position,
                wreck: w.position,
                wreck_vel: w.velocity,
            });
        }
        Some(out)
    })
}

/// The capture's word `w` (`wreck_pos`, `truck_vel`, ..) in row `f` as a vector.
fn r53_row(cap: &Rows, f: u32, w: &str) -> Vf {
    let c = |k: &str| cap.f(f, &format!("{w}.{k}"));
    Vf::new(c("x"), c("y"), c("z"), 0.0)
}

fn apart(a: Vf, b: Vf) -> f32 {
    (0..3).map(|i| (a.0[i] - b.0[i]).powi(2)).sum::<f32>().sqrt()
}

/// **P2's wreck bike rolls at the parked truck as the original's** (S506, R53): seeded off the wreck's
/// entry (row 349, the image's own: 25 ft/s at the truck), the Sim's wreck body (place and velocity)
/// is the capture's bit for bit on every row 350..=357, 22.8 ft/s slowing to 16.9 as it tumbles; the truck asleep on both, its
/// body the image's to the bit, and the plane 391 ft off.
#[test]
fn r53_the_wreck_bike_rolls_at_the_truck_as_the_original() {
    let Some(cap) = r53_capture() else { return };
    let Some(ticks) = r53_ride(R53_WAKE - 1) else { return };
    let at = |f: u32| &ticks[(f - R53_FROM - 1) as usize];
    let bits = |v: Vf| three(v);
    // Row 349 is the image's own wreck body (a late read): the poke's 25 ft/s at the truck.
    let Some(ee) = r53_image() else { return };
    let body =ee.u32_at(ee.u32_at(R53_P2 + 0x7a8) + 0x110);
    assert_eq!(bits(quad(&ee, body + 0x70)), bits(r53_row(&cap, R53_FROM + 1, "wreck_vel")), "row 349 is the image's wreck body");
    assert!(cap.f(R53_FROM + 1, "wreck_vel.z") > 24.9, "the wreck bike starts at 25 ft/s at the truck");
    for f in R53_FROM + 1..R53_WAKE {
        let t = at(f);
        assert!(!t.awake && cap.u(f, "truck_awake") == 0, "row {f}: the truck asleep on both");
        assert!(apart(t.plane, t.truck) > 390.0, "row {f}: the plane {} ft from the truck", apart(t.plane, t.truck));
        if f == R53_FROM + 1 {
            continue;
        }
        assert_eq!(bits(t.wreck), bits(r53_row(&cap, f, "wreck_pos")), "row {f}: the wreck body at {:?} vs {:?}", t.wreck.0, r53_row(&cap, f, "wreck_pos").0);
        assert_eq!(bits(t.wreck_vel), bits(r53_row(&cap, f, "wreck_vel")), "row {f}: the wreck body moving {:?} vs {:?}", t.wreck_vel.0, r53_row(&cap, f, "wreck_vel").0);
        assert_eq!(bits(t.truck), bits(r53_row(&cap, f, "truck_pos")), "row {f}: the truck at {:?}", t.truck.0);
        assert_eq!(bits(t.truck_vel), bits(r53_row(&cap, f, "truck_vel")), "row {f}: the truck's velocity {:?}", t.truck_vel.0);
    }
    assert!(at(357).wreck_vel.z() < 17.0 && at(350).wreck_vel.z() > 22.0, "the wreck bike slows from 22.8 to 16.9 ft/s");
}

/// **The wreck bike wakes the truck on the original's row** (S506, R53): row 358 has the truck awake
/// on both, the original's with one own contact (kind 0) whose other object is the wreck body's
/// collision object (the body's `+0x1040`; in the Sim nothing else is in reach), the Sim's wreck body
/// within 0.1 ft of the original's last good gap to the truck's (10.6 ft, row 357's: row 358's wreck
/// columns repeat it, the truck has not moved) and the plane 391 ft off; the wreck bike bounces back off it (its `z` speed below zero on
/// row 359 on both). Replaces S486a's wreck bike set 8 ft beside the truck on C16's image (reason 7).
#[test]
fn r53_the_wreck_bike_wakes_the_truck_on_the_originals_row() {
    let Some(cap) = r53_capture() else { return };
    let Some(ticks) = r53_ride(R53_WAKE + 1) else { return };
    let at = |f: u32| &ticks[(f - R53_FROM - 1) as usize];
    let woke = R53_FROM + 1 + ticks.iter().position(|t| t.awake).expect("the wreck bike met the truck") as u32;
    assert_eq!(cap.u(R53_WAKE - 1, "truck_awake"), 0);
    assert_eq!(cap.u(R53_WAKE, "truck_awake"), 1);
    assert_eq!(woke, R53_WAKE, "the truck wakes on the original's row");
    assert_eq!((cap.u(R53_WAKE, "truck_contacts"), cap.u(R53_WAKE, "truck_contact0_kind")), (1, 0), "the original's one own contact");
    assert_eq!(cap.u(R53_WAKE, "truck_contact0_other"), cap.u(R53_WAKE, "wreck_object") + 0x1040, "the contact's other object is the wreck body's");
    // Row 358's wreck columns repeat 357's (torn), so the original's gap is row 357's.
    let gap = apart(r53_row(&cap, R53_WAKE - 1, "wreck_pos"), r53_row(&cap, R53_WAKE - 1, "truck_pos"));
    let t = at(R53_WAKE);
    assert!((gap - 10.6).abs() < 0.01, "the original's wreck body {gap} ft from the truck's on row 357");
    assert!((apart(t.wreck, t.truck) - gap).abs() < 0.1, "the Sim's wreck body {} ft from the truck's", apart(t.wreck, t.truck));
    assert!(apart(t.plane, t.truck) > 390.0 && apart(r53_row(&cap, R53_WAKE, "plane_pos"), r53_row(&cap, R53_WAKE, "truck_pos")) > 390.0, "the plane far off");
    assert!(at(R53_WAKE + 1).wreck_vel.z() < 0.0 && cap.f(R53_WAKE + 1, "wreck_vel.z") < 0.0, "the wreck bike bounces back on both");
}

/// **Where the Sim parts from R53's capture** (S506): the first row from 350 (row 358, torn, left
/// out) where the truck's awake word differs or the wreck body's velocity is 0.1 ft/s off. Not held
/// past it: on row 359 the original's wreck bike bounces back at -2.31 ft/s and the Sim's at -1.34;
/// the original's truck takes (0.02, -0.09, -0.04) ft/s off the touch and stays awake to row 363
/// (asleep 364), the Sim's keeps its own settle's -0.25 down, sleeps on 359, wakes 360 and 362 and
/// sleeps from 363; the original's second touch wakes it again on row 374, the Sim's wreck bike (by
/// then 0.3 ft further along `+z`) never does.
#[test]
fn r53_the_sim_parts_from_the_original_on_the_bounce() {
    let Some(cap) = r53_capture() else { return };
    let Some(ticks) = r53_ride(R53_FROM + 40) else { return };
    let parts = (R53_FROM + 2..R53_FROM + 41).filter(|&f| f != R53_WAKE).find(|&f| {
        let t = &ticks[(f - R53_FROM - 1) as usize];
        let want = r53_row(&cap, f, "wreck_vel");
        t.awake != (cap.u(f, "truck_awake") == 1) || (0..3).any(|i| (t.wreck_vel.0[i] - want.0[i]).abs() > 0.1)
    });
    assert_eq!(parts, Some(R53_PARTS), "the Sim parts from the capture on another row");
}

/// Prints the Sim beside R53's rows (`--ignored --nocapture`).
#[test]
#[ignore]
fn r53_rows_beside_the_sim() {
    let Some(cap) = r53_capture() else { return };
    let Some(ticks) = r53_ride(R53_FROM + 40) else { return };
    for (k, t) in ticks.iter().enumerate() {
        let f = R53_FROM + 1 + k as u32;
        println!("after {f}: sim awake {} wreck {:?} v {:?} truck v {:?}", t.awake as u8, t.wreck.0, t.wreck_vel.0, t.truck_vel.0);
        println!("   row {f}: cap awake {} wreck {:?} v {:?} truck v {:?}", cap.u(f, "truck_awake"), r53_row(&cap, f, "wreck_pos").0, r53_row(&cap, f, "wreck_vel").0, r53_row(&cap, f, "truck_vel").0);
    }
}
