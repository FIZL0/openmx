//! The car step (S149d) as a whole: S149's captured drives of the baja, the buggy and the monster
//! truck (`context/capture/s149b/<car>.csv`) replayed through the port on free07's ground, row by row
//! from each row's own state, and every word the original wrote by the next row compared to the bit.

use game_render::disc::Disc;
use game_sim::air::AcosTable;
use game_sim::biplane::Body;
use game_sim::camera::CurveTable;
use game_sim::car::{Car, Controls, Drivetrain, Kind, Launch, Machine, PreloadState, TireState, Wheel};
use game_sim::math::Mat4;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, World};
use ps2_float::{F, Vf, fpu};
use ps2_oracle::{Ee, Kit, skip};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const NAMES: [&str; 4] = ["fl", "fr", "rl", "rr"];

/// The tire object's words the replay reads.
mod tire_at {
    pub const SOLID: usize = 0xb4;
    pub const SURFACE: usize = 0x100;
    pub const GROUND_POINT: usize = 0x110;
    pub const NORMAL: usize = 0x120;
}

struct Raw {
    head: Vec<u8>,
}

impl Raw {
    fn word(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.head[o..o + 4].try_into().unwrap())
    }
    fn v(&self, o: usize) -> Vf {
        Vf::from_bits([0, 4, 8, 12].map(|i| self.word(o + i)))
    }
}

struct Row {
    frame_counter: u32,
    tires: [Raw; 4],
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn load_rows(name: &str) -> Option<Vec<Row>> {
    let kit = ps2_oracle::Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("capture").join("s149b").join(format!("{name}.csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} — S149b's capture.sh records it", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: HashMap<&str, usize> = lines.next()?.split(',').enumerate().map(|(i, h)| (h, i)).collect();
    Some(
        lines
            .map(|l| {
                let c: Vec<&str> = l.split(',').collect();
                let col = |n: &str| c[head[n]];
                Row {
                    frame_counter: col("frame_counter").parse().unwrap(),
                    tires: NAMES.map(|n| Raw { head: unhex(col(&format!("tire_{n}_raw"))) }),
                }
            })
            .collect(),
    )
}

fn on_free07<R>(then: impl FnOnce(&mut Ground<'_>) -> R) -> Option<R> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let res = disc.res("Tracks\\free07.res").unwrap_or_else(|e| panic!("free07: {e}"));
    let scene = Scene::parse(&res.read_suffix(".scn").unwrap()).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = res.read_suffix(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&bytes).expect("parse the terrain");
    let mut ground = Ground::new(&terrain, &scene.terrain);
    Some(then(&mut ground))
}

/// The ground under every solid tire's probe is the port's free07 query, point, normal and surface.
#[test]
fn the_probes_find_free07s_ground() {
    for name in ["baja", "buggy", "truck"] {
        let Some(rows) = load_rows(name) else { return };
        let r = on_free07(|g| {
            let (mut n, mut bad) = (0, 0);
            for row in &rows {
                for (k, t) in row.tires.iter().enumerate() {
                    if t.word(tire_at::SOLID) == 0 {
                        continue;
                    }
                    n += 1;
                    let p = t.v(tire_at::GROUND_POINT);
                    let (point, normal) = g.ground(Vf::new(p.x(), p.y() + 3.0, p.z(), p.w()));
                    let normal = normal.merge(ps2_float::Dest::XYZ, normal.mul_q(ps2_float::vu::rsqrt(1.0, ps2_float::vu::dot3(normal.0, normal.0))));
                    let want_n = t.v(tire_at::NORMAL);
                    let surface = g.wheel_surface(p);
                    let ok = point.y().to_bits() == p.y().to_bits()
                        && normal.0[..3].iter().zip(&want_n.0[..3]).all(|(a, b)| a.to_bits() == b.to_bits())
                        && surface.is_none_or(|s| u32::from(s) == t.word(tire_at::SURFACE) & 0xff);
                    if !ok {
                        bad += 1;
                        if bad <= 5 {
                            eprintln!(
                                "{name} fc {} {}: y {} want {}, n {:?} want {:?}, surface {:?} want {}",
                                row.frame_counter, NAMES[k], point.y(), p.y(), normal, want_n, surface, t.word(tire_at::SURFACE)
                            );
                        }
                    }
                }
            }
            (n, bad)
        });
        let Some((n, bad)) = r else { return };
        assert!(n > 0, "{name}: no solid tire");
        assert_eq!(bad, 0, "{name}: {bad} of {n} probes off");
        let _ = F::ZERO;
    }
}

// --- the whole step, row by row ---------------------------------------------------------------------

/// Where S149a's switch parks the car, in every drive.
const CAR: u32 = 0x00605270;
/// The car's raw block starts at car +0x200, the tire's tail at tire +0x9b0.
const CAR_RAW: usize = 0x200;
const TAIL: usize = 0x9b0;

/// The car object's words the step reads and writes.
mod car_at {
    pub const GRAVITY: u32 = 0x140;
    pub const BODY: u32 = 0x160;
    pub const LAST_COFG: usize = 0x250;
    pub const GROUND_VELOCITY: usize = 0x2a0;
    pub const LATCHED: usize = 0x2bc;
    pub const LATCH: usize = 0x2c0;
    pub const WRECKED: usize = 0x2c4;
    pub const WRECK_LIFTED: usize = 0x2c8;
    pub const AIRBORNE_TOUCH: usize = 0x2cc;
    pub const STUCK: usize = 0x2d0;
    pub const BRAKE: usize = 0x3dc;
    pub const GAS: usize = 0x3e0;
    pub const CLUTCH: usize = 0x3e4;
    pub const PAD_WORD: usize = 0x3e8;
    pub const THROTTLE: usize = 0x3ec;
    pub const ENGAGED_CLUTCH: usize = 0x3f0;
    pub const FRONT_BRAKE: usize = 0x3f4;
    pub const REAR_BRAKE: usize = 0x3f8;
    pub const HAND_BRAKE: usize = 0x3fc;
    pub const STEER: usize = 0x400;
    pub const REAR_STEER: usize = 0x404;
    pub const HAND_BRAKE_FADE: usize = 0x428;
    pub const LAUNCH: usize = 0x42c;
    pub const PRELOAD: usize = 0x440;
    pub const DRIVEN: usize = 0x4ac;
    pub const SQUARE: usize = 0x4b0;
    pub const DPAD: usize = 0x4b4;
    pub const STICK: usize = 0x4b8;
    pub const LEAN: usize = 0x4bc;
    pub const STEER_INPUT: usize = 0x4c0;
    pub const GROUNDED: usize = 0x530;
    pub const WAS_GROUNDED: usize = 0x534;
    pub const YAW_LIMIT: usize = 0x540;
    pub const GROUND_SPEED: usize = 0x55c;
    pub const ALPHA: usize = 0x560;
    pub const SIDE_SLIP: usize = 0x564;
    pub const WORLD_RATES: usize = 0x570;
    pub const VELOCITY: usize = 0x580;
    pub const FORWARD: usize = 0x590;
    pub const UP: usize = 0x5a0;
    pub const EULER: usize = 0x5b0;
    pub const MATRIX: usize = 0x5c0;
}

/// The tire object's words the step reads and writes.
mod wheel_at {
    pub const DROP: usize = 0x70;
    pub const MOUNT: usize = 0x80;
    pub const TOP_WHEEL_SPEED: usize = 0xa0;
    pub const SOLID: usize = 0xb4;
    pub const GROUNDED: usize = 0xc0;
    pub const LONE_FRONT: usize = 0xc4;
    pub const HANDED: usize = 0xc8;
    pub const HANDED_LOAD: usize = 0xcc;
    pub const PROBES: usize = 0xd8;
    pub const COMPRESSION: usize = 0xdc;
    pub const LAST_COMPRESSION: usize = 0xe0;
    pub const LOAD: usize = 0xe4;
    pub const LEVER: usize = 0xf0;
    pub const SURFACE: usize = 0x100;
    pub const GROUND_POINT: usize = 0x110;
    pub const NORMAL: usize = 0x120;
    pub const CAR_NORMAL: usize = 0x130;
    pub const CONTACT: usize = 0x140;
    pub const UPPER: usize = 0x150;
    pub const LOWER: usize = 0x160;
    pub const POINT_VELOCITY: usize = 0x170;
    pub const BODY_VELOCITY: usize = 0x180;
    pub const LOCAL_VELOCITY: usize = 0x190;
    pub const ANGLE: usize = 0x1a0;
    pub const SPIN: usize = 0x1a4;
    pub const PROBE_NORMAL: usize = 0x1d0;
    pub const GRIP: usize = 0x1e0;
    pub const SLIPPING: usize = 0x1e4;
    pub const SLIDING: usize = 0x1e8;
    pub const LOCKED: usize = 0x1ec;
    pub const TORQUE: usize = 0x1f0;
    pub const LAST_TORQUE: usize = 0x1f4;
    pub const SLIP_RATIO: usize = 0x20c;
    pub const SLIP_ANGLE: usize = 0x210;
    pub const SLIP_TAN: usize = 0x214;
    pub const VELOCITY: usize = 0x220;
    pub const LAST_VELOCITY: usize = 0x230;
    pub const FORCE: usize = 0x240;
    pub const LAST_FORCE: usize = 0x250;
    pub const STEER_ANGLE: usize = 0x260;
    pub const FRAME: usize = 0x270;
    pub const SIDE_RATIO: usize = 0x2b0;
    pub const FRICTION_SCALE: usize = 0x9b4;
    pub const CAMBER: usize = 0x9bc;
}

/// A block of bytes addressed by its object's offsets, `base` its first.
struct Block<'a> {
    bytes: &'a [u8],
    base: usize,
}

impl Block<'_> {
    fn w(&self, o: usize) -> u32 {
        let o = o - self.base;
        u32::from_le_bytes(self.bytes[o..o + 4].try_into().unwrap())
    }
    fn f(&self, o: usize) -> F {
        F::from_bits(self.w(o))
    }
    fn flag(&self, o: usize) -> bool {
        self.w(o) != 0
    }
    fn v(&self, o: usize) -> Vf {
        Vf::from_bits([0, 4, 8, 12].map(|i| self.w(o + i)))
    }
    fn m(&self, o: usize) -> Mat4 {
        Mat4([0, 16, 32, 48].map(|i| self.v(o + i)))
    }
}

/// A row as the replay reads it.
struct DriveRow {
    frame_counter: u32,
    dt: F,
    body: [Vf; 4],
    car: Vec<u8>,
    engine: Vec<u8>,
    heads: [Vec<u8>; 4],
    tails: [Vec<u8>; 4],
}

impl DriveRow {
    fn car(&self) -> Block<'_> {
        Block { bytes: &self.car, base: CAR_RAW }
    }
    fn engine(&self) -> Block<'_> {
        Block { bytes: &self.engine, base: 0 }
    }
    /// One tire's word, from the head or the tail block.
    fn tire(&self, k: usize, o: usize) -> u32 {
        if o >= TAIL {
            Block { bytes: &self.tails[k], base: TAIL }.w(o)
        } else {
            Block { bytes: &self.heads[k], base: 0 }.w(o)
        }
    }
    fn tf(&self, k: usize, o: usize) -> F {
        F::from_bits(self.tire(k, o))
    }
    fn tv(&self, k: usize, o: usize) -> Vf {
        Vf::from_bits([0, 4, 8, 12].map(|i| self.tire(k, o + i)))
    }
}

fn load_drive_rows(kit: &Kit, name: &str) -> Option<Vec<DriveRow>> {
    let path = kit.ctx("capture").join("s149b").join(format!("{name}.csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} — S149b's capture.sh records it", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: HashMap<&str, usize> = lines.next()?.split(',').enumerate().map(|(i, h)| (h, i)).collect();
    Some(
        lines
            .map(|l| {
                let c: Vec<&str> = l.split(',').collect();
                let col = |n: &str| c[head[n]];
                DriveRow {
                    frame_counter: col("frame_counter").parse().unwrap(),
                    dt: F(col("dt").parse::<f32>().unwrap()),
                    body: ["body_pos", "body_vel", "body_quat", "body_rate"].map(|b| {
                        let [x, y, z, w] = ["x", "y", "z", "w"].map(|l| col(&format!("{b}.{l}")).parse::<f32>().unwrap());
                        Vf::new(x, y, z, w)
                    }),
                    car: unhex(col("car_raw")),
                    engine: unhex(col("engine_raw")),
                    heads: NAMES.map(|n| unhex(col(&format!("tire_{n}_raw")))),
                    tails: NAMES.map(|n| unhex(col(&format!("tire_{n}_tail")))),
                }
            })
            .collect(),
    )
}

/// A car's file, out of `machine.res` in MX.PSR.
fn car_file(kit: &Kit, kind: Kind) -> Option<Vec<u8>> {
    use game_sim::archive::{Psr, Res};
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
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
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(kind.phy())).expect("the car's file");
    Some(res.read(&bytes, e).expect("its bytes"))
}

fn quad(ee: &Ee, at: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|i| ee.u32_at(at + i)))
}

/// The car's body as the image has it: what no row carries (the masses, the dampings) is constant.
fn body_of(ee: &Ee) -> Body {
    let b = ee.u32_at(CAR + car_at::BODY);
    Body {
        enabled: ee.u32_at(b) != 0,
        inv_mass: F(ee.f32_at(b + 0x10)),
        damping: quad(ee, b + 0x20),
        last_position: quad(ee, b + 0x50),
        position: quad(ee, b + 0x60),
        velocity: quad(ee, b + 0x70),
        last_force: quad(ee, b + 0x80),
        force: quad(ee, b + 0x90),
        accel: quad(ee, b + 0xa0),
        shock_peak: quad(ee, b + 0xb0),
        shock: quad(ee, b + 0xc0),
        step: quad(ee, b + 0xf0),
        inv_inertia: quad(ee, b + 0x120),
        ang_damping: quad(ee, b + 0x130),
        last_quat: quad(ee, b + 0x150),
        quat: quad(ee, b + 0x170),
        cofg: quad(ee, b + 0x180),
        moment: quad(ee, b + 0x190),
        last_moment: quad(ee, b + 0x1a0),
        rates: quad(ee, b + 0x1b0),
        ang_accel: quad(ee, b + 0x1c0),
        twist_peak: quad(ee, b + 0x1d0),
        twist: quad(ee, b + 0x1e0),
    }
}

/// The engine's words as S149c's test lays them.
fn fill(d: &mut Drivetrain, e: &Block<'_>) {
    let w = |o: usize| e.w(o);
    let f = |o: usize| e.f(o);
    d.four_wheel = w(0x38) != 0 && w(0x3c) != 0;
    d.idle_throttle = f(0x4c);
    d.engine_braking = f(0x50);
    d.final_gear_ratio = f(0x54);
    d.transmission_loss = f(0x58);
    d.inv_inertia = f(0x60);
    d.pump_torque_scale = f(0x64);
    d.max_k = f(0x68);
    d.launch_boost = f(0x6c);
    d.min_rpm = f(0x70);
    d.tcs = w(0x74) != 0;
    d.min_traction_slip = f(0x78);
    d.max_traction_slip = f(0x7c);
    d.nox_on = w(0x80) != 0;
    d.nox_left = f(0x84);
    d.nox_burn_rate = f(0x88);
    d.nox_hp_boost = f(0x8c);
    let s = &mut d.shift_points;
    s.low_load_rpm = f(0x90);
    d.high_load_rpm = f(0x94);
    s.low_load_up_shift_rpm = f(0x98);
    s.high_load_up_shift_rpm = f(0x9c);
    s.low_load_down_shift_rpm = f(0xa0);
    s.high_load_down_shift_rpm = f(0xa4);
    s.shift_time = f(0xa8);
    d.shift_timer = f(0xac);
    d.shifting_up = w(0xb0) != 0;
    s.shift_up_duration = f(0xb4);
    s.shift_up_throttle = f(0xb8);
    s.shift_down_duration = f(0xbc);
    s.shift_down_throttle = f(0xc0);
    d.shift_throttle_timer = f(0xc4);
    d.reverse = w(0xc8) != 0;
    d.gear = w(0xcc) as i32;
    d.ratio = f(0xe4);
    d.inv_ratio = f(0xe8);
    d.wheel_rpm = f(0xec);
    d.max_pump_torque = f(0xf0);
    d.converter_torque = f(0xf4);
    d.multiplication = f(0xf8);
    d.output_torque = f(0xfc);
    d.cursor_index = w(0x104);
    d.cursor_frac = f(0x108);
    d.engine_torque = f(0x110);
    d.braking_torque = f(0x114);
    d.drive_torque = [f(0x118), f(0x11c), f(0x120), f(0x124)];
    d.rpm = f(0x128);
    d.slip_rpm = f(0x12c);
    d.redline = f(0x130);
    d.sound_clock = f(0x134);
    d.pitch = f(0x138);
}

/// The engine words the step writes, but the pitch, which only the sound reads (it takes the
/// engine's noise).
fn engine_words(d: &Drivetrain) -> Vec<(&'static str, usize, u32)> {
    let b = |x: F| x.to_bits();
    vec![
        ("engine shift timer", 0xac, b(d.shift_timer)),
        ("engine shifting up", 0xb0, d.shifting_up as u32),
        ("engine shift throttle timer", 0xc4, b(d.shift_throttle_timer)),
        ("engine reverse", 0xc8, d.reverse as u32),
        ("engine gear", 0xcc, d.gear as u32),
        ("engine ratio", 0xe4, b(d.ratio)),
        ("engine wheel rpm", 0xec, b(d.wheel_rpm)),
        ("engine converter torque", 0xf4, b(d.converter_torque)),
        ("engine multiplication", 0xf8, b(d.multiplication)),
        ("engine output torque", 0xfc, b(d.output_torque)),
        ("engine cursor", 0x104, d.cursor_index),
        ("engine cursor frac", 0x108, b(d.cursor_frac)),
        ("engine torque", 0x110, b(d.engine_torque)),
        ("engine braking torque", 0x114, b(d.braking_torque)),
        ("engine drive torque 0", 0x118, b(d.drive_torque[0])),
        ("engine drive torque 1", 0x11c, b(d.drive_torque[1])),
        ("engine rpm", 0x128, b(d.rpm)),
        ("engine slip rpm", 0x12c, b(d.slip_rpm)),
        ("engine sound clock", 0x134, b(d.sound_clock)),
        ("engine launch boost", 0x6c, b(d.launch_boost)),
    ]
}

fn wheel_of(r: &DriveRow, k: usize) -> Wheel {
    use wheel_at::*;
    let f = |o| r.tf(k, o);
    let flag = |o| r.tire(k, o) != 0;
    let v = |o| r.tv(k, o);
    Wheel {
        s: TireState {
            surface: (r.tire(k, SURFACE) & 0xff) as usize,
            grounded: flag(GROUNDED),
            handoff: flag(HANDED).then(|| f(HANDED_LOAD)),
            compression: f(COMPRESSION),
            last_compression: f(LAST_COMPRESSION),
            load: f(LOAD),
            contact: v(CONTACT),
            angle: f(ANGLE),
            spin: f(SPIN),
            torque: f(TORQUE),
            last_torque: f(LAST_TORQUE),
            slip_ratio: f(SLIP_RATIO),
            slip_angle: f(SLIP_ANGLE),
            slip_tan: f(SLIP_TAN),
            velocity: v(VELOCITY),
            last_velocity: v(LAST_VELOCITY),
            force: v(FORCE),
            last_force: v(LAST_FORCE),
            steer: f(STEER_ANGLE),
            side_ratio: f(SIDE_RATIO),
            camber: f(CAMBER),
            friction_scale: f(FRICTION_SCALE),
            low_speed_grip: flag(GRIP),
            slipping: flag(SLIPPING),
            sliding: flag(SLIDING),
            locked: flag(LOCKED),
        },
        mount: v(MOUNT),
        top_wheel_speed: f(TOP_WHEEL_SPEED),
        probe_normal: v(PROBE_NORMAL),
        car_normal: v(CAR_NORMAL),
        drop: v(DROP),
        lever: v(LEVER),
        upper: v(UPPER),
        lower: v(LOWER),
        probes: r.tire(k, PROBES) as i32,
        solid: flag(SOLID),
        ground_point: v(GROUND_POINT),
        ground_normal: v(NORMAL),
        point_velocity: v(POINT_VELOCITY),
        body_velocity: v(BODY_VELOCITY),
        local_velocity: v(LOCAL_VELOCITY),
        frame: Mat4([0, 16, 32, 48].map(|i| v(FRAME + i))),
        lone_front: flag(LONE_FRONT),
    }
}

/// Every tire word the step writes, named, with its offset.
fn wheel_words(w: &Wheel) -> Vec<(&'static str, usize, u32)> {
    use wheel_at::*;
    let b = |x: F| x.to_bits();
    let mut out = vec![
        ("solid", SOLID, w.solid as u32),
        ("grounded", GROUNDED, w.s.grounded as u32),
        ("lone front", LONE_FRONT, w.lone_front as u32),
        ("handed", HANDED, w.s.handoff.is_some() as u32),
        ("probes", PROBES, w.probes as u32),
        ("compression", COMPRESSION, b(w.s.compression)),
        ("last compression", LAST_COMPRESSION, b(w.s.last_compression)),
        ("load", LOAD, b(w.s.load)),
        ("angle", ANGLE, b(w.s.angle)),
        ("spin", SPIN, b(w.s.spin)),
        ("grip", GRIP, w.s.low_speed_grip as u32),
        ("slipping", SLIPPING, w.s.slipping as u32),
        ("sliding", SLIDING, w.s.sliding as u32),
        ("locked", LOCKED, w.s.locked as u32),
        ("torque", TORQUE, b(w.s.torque)),
        ("last torque", LAST_TORQUE, b(w.s.last_torque)),
        ("slip ratio", SLIP_RATIO, b(w.s.slip_ratio)),
        ("slip angle", SLIP_ANGLE, b(w.s.slip_angle)),
        ("slip tan", SLIP_TAN, b(w.s.slip_tan)),
        ("steer angle", STEER_ANGLE, b(w.s.steer)),
        ("side ratio", SIDE_RATIO, b(w.s.side_ratio)),
        ("camber", CAMBER, b(w.s.camber)),
        ("friction scale", FRICTION_SCALE, b(w.s.friction_scale)),
        ("top wheel speed", TOP_WHEEL_SPEED, b(w.top_wheel_speed)),
    ];
    if let Some(load) = w.s.handoff {
        out.push(("handed load", HANDED_LOAD, b(load)));
    }
    let lanes = |out: &mut Vec<_>, name: &'static str, o: usize, v: Vf, n: usize| {
        for i in 0..n {
            out.push((name, o + 4 * i, v.0[i].to_bits()));
        }
    };
    for (name, o, v, n) in [
        ("drop", DROP, w.drop, 4),
        ("lever", LEVER, w.lever, 4),
        ("ground point", GROUND_POINT, w.ground_point, 3),
        ("ground normal", NORMAL, w.ground_normal, 4),
        ("car normal", CAR_NORMAL, w.car_normal, 4),
        ("probe normal", PROBE_NORMAL, w.probe_normal, 4),
        ("upper", UPPER, w.upper, 4),
        ("lower", LOWER, w.lower, 4),
        ("contact", CONTACT, w.s.contact, 4),
        ("point velocity", POINT_VELOCITY, w.point_velocity, 4),
        ("body velocity", BODY_VELOCITY, w.body_velocity, 4),
        ("local velocity", LOCAL_VELOCITY, w.local_velocity, 4),
        ("velocity", VELOCITY, w.s.velocity, 4),
        ("last velocity", LAST_VELOCITY, w.s.last_velocity, 4),
        ("force", FORCE, w.s.force, 4),
        ("last force", LAST_FORCE, w.s.last_force, 4),
    ] {
        lanes(&mut out, name, o, v, n);
    }
    if w.s.grounded {
        for i in 0..3 {
            lanes(&mut out, "frame", FRAME + 16 * i, w.frame.0[i], 3);
        }
    }
    out
}

fn machine_of(car: &Car, ee: &Ee, body: &Body, r: &DriveRow) -> Machine {
    use car_at::*;
    let c = r.car();
    let mut body = body.clone();
    [body.position, body.velocity, body.quat, body.rates] = r.body;
    let mut drivetrain = Drivetrain::new(&car.engine);
    fill(&mut drivetrain, &r.engine());
    let controls = Controls {
        driven: c.flag(DRIVEN),
        square_steering: c.flag(SQUARE),
        // The input step runs after the world's, so a row's input is what the next frame steps on.
        stick: c.f(STICK),
        lean: c.f(LEAN),
        dpad: c.flag(DPAD),
        pad_word: c.f(PAD_WORD),
        gas: c.f(GAS),
        brake: c.f(BRAKE),
        clutch: c.f(CLUTCH),
        hand_brake: c.f(HAND_BRAKE),
        steer_input: c.f(STEER_INPUT),
        steer: c.f(STEER),
        rear_steer: c.f(REAR_STEER),
        front_brake: c.f(FRONT_BRAKE),
        rear_brake: c.f(REAR_BRAKE),
        throttle: c.f(THROTTLE),
        engaged_clutch: c.f(ENGAGED_CLUTCH),
        hand_brake_fade: c.f(HAND_BRAKE_FADE),
        yaw_rate_limit: c.f(YAW_LIMIT),
        rear_friction: r.tf(2, wheel_at::FRICTION_SCALE),
    };
    let l = |i: usize| c.f(LAUNCH + 4 * i);
    let p = |i: usize| c.f(PRELOAD + 4 * i);
    Machine {
        body,
        gravity: quad(ee, CAR + GRAVITY),
        controls,
        drivetrain,
        wheels: [0, 1, 2, 3].map(|k| wheel_of(r, k)),
        last_cofg: c.v(LAST_COFG),
        human: true,
        steering_wheel: false,
        wrecked: c.flag(WRECKED),
        wreck_lifted: c.flag(WRECK_LIFTED),
        touching: false,
        airborne_touch: c.flag(AIRBORNE_TOUCH),
        stuck_time: c.f(STUCK),
        launch: Launch { timer: l(0), start: l(1), time: l(2), max_scale: l(3), max_scale_speed: l(4) },
        preload: PreloadState {
            ready: c.flag(PRELOAD),
            active: c.flag(PRELOAD + 4),
            armed: c.flag(PRELOAD + 8),
            time: p(3),
            timer: p(4),
            min_time: p(5),
            max_time: p(6),
            max_preload: p(7),
            force: c.v(PRELOAD + 0x20),
        },
        latch: c.flag(LATCH),
        latched: c.flag(LATCHED),
        grounded: c.flag(GROUNDED),
        was_grounded: c.flag(WAS_GROUNDED),
        ground_speed: c.f(GROUND_SPEED),
        velocity: c.v(VELOCITY),
        ground_velocity: c.v(GROUND_VELOCITY),
        world_rates: c.v(WORLD_RATES),
        alpha: c.f(ALPHA),
        side_slip: c.f(SIDE_SLIP),
        euler: c.v(EULER),
        matrix: c.m(MATRIX),
        forward: c.v(FORWARD),
        up: c.v(UP),
        event: None,
    }
}

/// Every car word the step writes, named, with its offset.
fn car_words(m: &Machine) -> Vec<(&'static str, usize, u32)> {
    use car_at::*;
    let b = |x: F| x.to_bits();
    let c = &m.controls;
    let mut out = vec![
        ("steer input", STEER_INPUT, b(c.steer_input)),
        ("steer", STEER, b(c.steer)),
        ("rear steer", REAR_STEER, b(c.rear_steer)),
        ("front brake", FRONT_BRAKE, b(c.front_brake)),
        ("rear brake", REAR_BRAKE, b(c.rear_brake)),
        ("throttle", THROTTLE, b(c.throttle)),
        ("engaged clutch", ENGAGED_CLUTCH, b(c.engaged_clutch)),
        ("hand brake fade", HAND_BRAKE_FADE, b(c.hand_brake_fade)),
        ("yaw limit", YAW_LIMIT, b(c.yaw_rate_limit)),
        ("wrecked", WRECKED, m.wrecked as u32),
        ("wreck lifted", WRECK_LIFTED, m.wreck_lifted as u32),
        ("airborne touch", AIRBORNE_TOUCH, m.airborne_touch as u32),
        ("stuck", STUCK, b(m.stuck_time)),
        ("launch timer", LAUNCH, b(m.launch.timer)),
        ("launch start", LAUNCH + 4, b(m.launch.start)),
        ("preload ready", PRELOAD, m.preload.ready as u32),
        ("preload active", PRELOAD + 4, m.preload.active as u32),
        ("preload armed", PRELOAD + 8, m.preload.armed as u32),
        ("preload timer", PRELOAD + 0x10, b(m.preload.timer)),
        ("preload force y", PRELOAD + 0x24, m.preload.force.0[1].to_bits()),
        ("latch", LATCH, m.latch as u32),
        ("latched", LATCHED, m.latched as u32),
        ("grounded", GROUNDED, m.grounded as u32),
        ("was grounded", WAS_GROUNDED, m.was_grounded as u32),
        ("ground speed", GROUND_SPEED, b(m.ground_speed)),
        ("alpha", ALPHA, b(m.alpha)),
        ("side slip", SIDE_SLIP, b(m.side_slip)),
    ];
    for (name, o, v, n) in [
        ("last cofg", LAST_COFG, m.last_cofg, 4),
        ("ground velocity", GROUND_VELOCITY, m.ground_velocity, 4),
        ("world rates", WORLD_RATES, m.world_rates, 4),
        ("velocity", VELOCITY, m.velocity, 4),
        ("forward", FORWARD, m.forward, 4),
        ("up", UP, m.up, 4),
        ("euler", EULER, m.euler, 4),
        ("matrix right", MATRIX, m.matrix.0[0], 4),
        ("matrix up", MATRIX + 16, m.matrix.0[1], 4),
        ("matrix forward", MATRIX + 32, m.matrix.0[2], 4),
        ("matrix origin", MATRIX + 48, m.matrix.0[3], 4),
    ] {
        for i in 0..n {
            out.push((name, o + 4 * i, v.0[i].to_bits()));
        }
    }
    out
}

/// Mismatches per word, with the first few spelled out.
#[derive(Default)]
struct Tally {
    rows: usize,
    skipped: usize,
    torn: usize,
    bad: BTreeMap<String, usize>,
    first: Vec<String>,
}

impl Tally {
    fn word(&mut self, what: String, fc: u32, got: u32, want: u32) {
        if got != want {

            if self.first.len() < 40 && !self.bad.contains_key(&what) {
                self.first.push(format!(
                    "fc {fc} {what}: port {:e} ({got:#010x}), game {:e} ({want:#010x})",
                    f32::from_bits(got),
                    f32::from_bits(want)
                ));
            }
            *self.bad.entry(what).or_default() += 1;
        }
    }
}

/// The truck's run into free07's scenery: its speed falls from 129 ft/s to 21 in a frame and the
/// collision object's touch shows at +0x2cc on the last of them. The car's collision object (its
/// collide step and the contacts' resolve) is not this port's.
const STRUCK: &[(&str, std::ops::RangeInclusive<u32>)] = &[("truck", 720..=727)];

/// A row whose tire blocks the recorder read apart from its body (car_tire.rs's TORN has it too):
/// the truck's fc 1105 holds every tire's probe points about a frame's travel behind the body it
/// was stepped to, beside an exact body. Its body, car and engine words are still compared.
const TORN: &[(&str, u32)] = &[("truck", 1105)];

/// The input step's words, which it writes after the world's step: a row's are what the next frame
/// steps on.
fn take_input(m: &mut Machine, r: &DriveRow) {
    use car_at::*;
    let c = r.car();
    let k = &mut m.controls;
    (k.stick, k.lean, k.dpad, k.pad_word) = (c.f(STICK), c.f(LEAN), c.flag(DPAD), c.f(PAD_WORD));
    (k.gas, k.brake, k.clutch, k.hand_brake) = (c.f(GAS), c.f(BRAKE), c.f(CLUTCH), c.f(HAND_BRAKE));
}

/// One drive replayed against the capture. Row by row (`carry` off), each row's state is stepped
/// once and compared with the next row; carried, the port runs on from its own state, seeded again
/// only after a row it cannot follow (paused, two frames in one, or struck).
fn replay(kind: Kind, name: &str, carry: bool) -> Option<Tally> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let rows = load_drive_rows(&kit, name)?;
    let car = Car::read_bytes(&car_file(&kit, kind)?, kind);
    let image = kit.ctx("states").join(format!("s149a-{name}-switched.ee.bin"));
    if !image.exists() {
        skip(&format!("no {} — S149a's switch recipe makes it", image.display()));
        return None;
    }
    let ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    let body = body_of(&ee);
    let (acos, curve) = (AcosTable::build(), CurveTable::build());
    on_free07(|g| {
        let mut t = Tally::default();
        let mut running: Option<Machine> = None;
        for pair in rows.windows(2) {
            let (r0, r1) = (&pair[0], &pair[1]);
            // A row a frame on, not paused, and not two frames in one (the sound clock moved by one
            // frame's substeps).
            let (n, h) = {
                let n = (r1.dt * F(115.0)).to_i32() + 1;
                (n, r1.dt / F(fpu::from_i32(n)))
            };
            let mut clock = r0.engine().f(0x134);
            for _ in 0..n {
                clock = clock + h;
            }
            let struck = STRUCK.iter().any(|(n, fcs)| *n == name && fcs.contains(&r1.frame_counter));
            if r1.frame_counter != r0.frame_counter + 1 || clock.to_bits() != r1.engine().w(0x134) || struck {
                t.skipped += 1;
                running = None;
                continue;
            }
            t.rows += 1;
            let mut m = match running.take().filter(|_| carry) {
                Some(mut m) => {
                    take_input(&mut m, r0);
                    m
                }
                None => machine_of(&car, &ee, &body, r0),
            };
            m.step(&car, g, r1.dt, &acos, &curve, &mut |_| F::ZERO);
            let fc = r1.frame_counter;
            for (i, lane) in ["x", "y", "z", "w"].iter().enumerate() {
                for (what, got, want) in [
                    ("pos", m.body.position, r1.body[0]),
                    ("vel", m.body.velocity, r1.body[1]),
                    ("quat", m.body.quat, r1.body[2]),
                    ("rate", m.body.rates, r1.body[3]),
                ] {
                    t.word(format!("body {what}.{lane}"), fc, got.0[i].to_bits(), want.0[i].to_bits());
                }
            }
            let c = r1.car();
            for (what, o, got) in car_words(&m) {
                t.word(format!("car {what} +{o:#x}"), fc, got, c.w(o));
            }
            let e = r1.engine();
            for (what, o, got) in engine_words(&m.drivetrain) {
                t.word(what.to_string(), fc, got, e.w(o));
            }
            let torn = TORN.contains(&(name, fc));
            t.torn += torn as usize;
            for (k, w) in m.wheels.iter().enumerate().filter(|_| !torn) {
                for (what, o, got) in wheel_words(w) {
                    t.word(format!("{} {what} +{o:#x}", NAMES[k]), fc, got, r1.tire(k, o));
                }
            }
            running = Some(m);
        }
        t
    })
}

fn check(kind: Kind, name: &str, carry: bool) {
    let Some(t) = replay(kind, name, carry) else { return };
    assert!(t.rows > 100, "{name}: only {} rows replayed", t.rows);
    let mut report = format!("{name}{}: {} rows ({} skipped, {} torn)\n", if carry { " carried" } else { "" }, t.rows, t.skipped, t.torn);
    for (what, n) in &t.bad {
        report += &format!("  {what}: {n}\n");
    }
    for l in &t.first {
        report += &format!("  {l}\n");
    }
    assert!(t.bad.is_empty(), "{report}");
}

#[test]
fn the_baja_drive_replays() {
    check(Kind::Baja, "baja", false);
}

#[test]
fn the_buggy_drive_replays() {
    check(Kind::Buggy, "buggy", false);
}

#[test]
fn the_truck_drive_replays() {
    check(Kind::MonsterTruck, "truck", false);
}

#[test]
fn the_baja_drive_runs_on_its_own() {
    check(Kind::Baja, "baja", true);
}

#[test]
fn the_buggy_drive_runs_on_its_own() {
    check(Kind::Buggy, "buggy", true);
}

#[test]
fn the_truck_drive_runs_on_its_own() {
    check(Kind::MonsterTruck, "truck", true);
}
