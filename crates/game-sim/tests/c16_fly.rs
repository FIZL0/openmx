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
    let p = plane0(ee);
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
    let r = p2(ee);
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
#[derive(Debug, Clone, Copy)]
struct Tick {
    t: u32,
    state: [[u32; 3]; 2],
    crash: u32,
    /// P2's chassis velocity and centre, and the wreck its frame said ([`fly_two`]; in [`fly`], which
    /// does not step P2's rider, a downed P2's wreck bike's body velocity and position, S276).
    p2: [[u32; 3]; 2],
    p2_wreck: Option<u8>,
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
    in_free09_ss(&mut disc, |world, area| {
        sim.plane = Some(seeded_plane(&ee, &plane, &rays, &hull, mesh.as_ref(), area, &tables));
        // The pad the image's tick read, so the first tick's presses are not edges.
        sim.poll(dt_of(from - 1), &[pad_of(from - 1), Report::missing()]);
        let p = sim.plane.as_ref().unwrap();
        let seed = [three(p.body.velocity), three(p.body.position)];
        let mut out = Vec::new();
        for t in from..to {
            let dt = dt_of(t);
            let mut controls = sim.poll(dt, &[pad_of(t), Report::missing()]);
            controls.respawn = respawn_of(t);
            sim.step(dt, &controls, world);
            let p = sim.plane.as_ref().unwrap();
            let p2 = sim.others[0].rider.crash.as_ref().map_or([[0; 3]; 2], |c| [three(c.bike.body.velocity), three(c.bike.body.position)]);
            out.push(Tick { t, state: [three(p.body.velocity), three(p.body.position)], crash: p.crash.reason, p2, p2_wreck: None });
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
    in_free09_ss(&mut disc, |world, area| {
        seats[0].plane = Some(seeded_plane(&ee, &plane, &rays, &hull, mesh.as_ref(), area, &tables));
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
                f.frame_met(dt, pad, world, &tables.ride, &bikes, Some(&mut met));
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
            out.push(Tick {
                t,
                state: [three(p.body.velocity), three(p.body.position)],
                crash: p.crash.reason,
                p2: [three(ch.vel), three(ch.cofg_world)],
                p2_wreck: said.wreck.map(|w| w.0),
            });
        }
        Some((seed, out))
    })
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
    let body = &mut bike.body;
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

/// How many of the port's ticks the capture confirms, bit for bit (tests/fly_and_collide.rs's rule):
/// each row from the image's frame on reads one of the port's states in order, the image's then each
/// tick's, or repeats the one before, or skips one, or is torn between two axis by axis. A row that is
/// none of those parts the run.
fn held(cap: &Rows, seed: [[u32; 3]; 2], ticks: &[Tick]) -> usize {
    let states: Vec<[[u32; 3]; 2]> = std::iter::once(seed).chain(ticks.iter().map(|k| k.state)).collect();
    held_by(cap, &states, ticks[0].t, ticks.last().unwrap().t, |r| cap.state(r))
}

/// [`held`] on P2's chassis, velocity and centre.
fn p2_held(cap: &Rows, seed: [[u32; 3]; 2], ticks: &[Tick]) -> usize {
    let states: Vec<[[u32; 3]; 2]> = std::iter::once(seed).chain(ticks.iter().map(|k| k.p2)).collect();
    held_by(cap, &states, ticks[0].t, ticks.last().unwrap().t, |r| [cap.v3(r, "p2_ch_vel"), cap.v3(r, "p2_ch_centre")])
}

fn held_by(cap: &Rows, states: &[[[u32; 3]; 2]], from: u32, last: u32, read: impl Fn(u32) -> [[u32; 3]; 2]) -> usize {
    let mut at = 0;
    for r in from..=last + 2 {
        if cap.row(r).is_none() {
            continue;
        }
        let row = read(r);
        let reads = |j: usize| {
            let pair = [states[j.saturating_sub(1)], states[j]];
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
/// 50cc limit: rider_contact::wreck_contacts) and the wreck, and the plane to the tick after the
/// wreck; from 137810 P2, standing, parts at 137827.
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
        println!("joust from {from}: the plane holds {n}, P2 {m} of {} ticks; P2's wreck port {port:?} (its frame), original {theirs:?} (first row)", ticks.len());
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
/// ([`wrecked_p2`]); its wreck bike holds, the plane parts on its first tick, the wreck bike's mirror
/// on its list (S274) pushing it short of the original's (research/s274).
const JOUSTS: [(u32, u32, usize, usize); 3] = [(137810, 137900, 46, 17), (137845, 137900, 11, 55), (137910, 138010, 0, 100)];

/// The whole tape as images every 100 frames cut it: each stretch from its image to the next, and
/// how many of its ticks hold (S272's pins). Two part: 137810's on the joust's hit (137853's step), the
/// Sim's plane lane standing P2 still; and 137910's on its first tick, the plane bouncing on P2's
/// wreck. Since S276 [`fly`] seeds P2 down there and its wreck bike meets the plane in the Sim's lane
/// ([`the_sim_lane_meets_p2s_wreck_bike`]), but the plane's resolve against the wreck bike's mirror is
/// not yet the original's (S274), so it still parts on its first tick.
const STRETCHES: [usize; 29] = [
    8, 100, 100, 100, 100, 43, 0, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100,
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
/// original's. Why JOUSTS' 137910 stretch "parts on its first tick": [`held`] wants the tick's end.
#[test]
fn row_137911_is_the_wreck_bikes_first_substep() {
    let dt = |_| F(f32::from_bits(0x3c888882));
    let row = ["-24.94349", "8.60006", "2.21980"];
    let digits = |v: Vf| [0, 1, 2].map(|k| format!("{:.5}", v.0[k]));
    let first = |reason| {
        let (flown, kept) = game_sim::flight::substeps(|| fly_by(137910, 137911, &dt, &segment_pad, &|_| false, reason));
        flown.map(|(_, ticks)| (kept, ticks[0]))
    };
    let Some((kept, tick)) = first(7) else { return };
    assert_eq!(kept.len(), 2, "two substeps a tick");
    assert_eq!(digits(kept[0].velocity), row, "the first substep, P2 down");
    assert_ne!(tick.state[0].map(f32::from_bits).map(|v| format!("{v:.5}")), row, "the tick's end is past the row");
    let Some((standing, _)) = first(0) else { return };
    assert_ne!(digits(standing[0].velocity), row, "the first substep, P2 standing");
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

/// What [`fly_with_far`] keeps of a tick: the near plane's speed and spot, P2's wreck bike's.
type NearTick = ([[u32; 3]; 2], [[u32; 3]; 2]);

/// **P2's wreck bike under plane 0, alone or flown together with a far plane** (S276a), from
/// 137910's image as [`fly_by`]: P2 down with wreck 7, Cross held, nothing stepping P2 but the
/// planes. `far` `None` flies plane 0 alone ([`game_sim::wreck_plane::search_downed`],
/// [`Flight::frame_met`], [`game_sim::wreck_plane::step_downed`]); `Some(first)` flies it with a
/// second plane 400 ft above, out of everyone's reach, in one [`flight::frames_together`]
/// ([`game_sim::wreck_plane::search_downed_by`], [`game_sim::wreck_plane::step_downed_by`]), plane 0
/// first among them or second.
fn fly_with_far(far: Option<bool>) -> Option<Vec<NearTick>> {
    use game_sim::flight::{Fly, Met};
    use game_sim::wreck_plane;
    let (from, to) = (137910, 137920);
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
    in_free09_ss(&mut disc, |world, area| {
        let mut near = seeded_plane(&ee, &plane, &rays, &hull, mesh.as_ref(), area, &tables);
        let mut high = near.clone();
        let up = near.spawn_origin().add(Vf::new(0.0, 400.0, 0.0, 0.0));
        high.place(up, Vf::new(0.0, 0.0, 1.0, 0.0), &tables.ride.acos);
        sim.poll(dt, &[cross_held(from - 1), Report::missing()]);
        let mut out = Vec::new();
        for t in from..to {
            sim.poll(dt, &[cross_held(t), Report::missing()]);
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
            out.push(([three(near.body.velocity), three(near.body.position)], [three(c.bike.body.velocity), three(c.bike.body.position)]));
        }
        Some(out)
    })
}

/// **Planes flown together meet a downed rider's wreck bike** (S276a): with a second plane far
/// above, flown in one joint frame, plane 0 and P2's wreck bike go bit for bit as plane 0 alone
/// takes them ([`fly_with_far`]), plane 0 first among the fleet or second — the wreck bike's search
/// with both hulls, its mirror on plane 0's list alone, one body the joint resolve pushes, and its
/// step with both planes' bodies. That the lone plane meets it at all is
/// [`the_sim_lane_meets_p2s_wreck_bike`]'s.
#[test]
fn planes_flown_together_meet_p2s_wreck_bike() {
    let Some(alone) = fly_with_far(None) else { return };
    for k in &alone {
        println!("alone: plane {:?}; wreck bike {:?}", k.0.map(|v| v.map(f32::from_bits)), k.1.map(|v| v.map(f32::from_bits)));
    }
    for first in [true, false] {
        let Some(together) = fly_with_far(Some(first)) else { return };
        assert_eq!(together, alone, "plane 0 {} the far plane: as alone", if first { "ahead of" } else { "after" });
    }
}
