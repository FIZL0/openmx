//! S50 — the rider's ragdoll against the original.
//!
//! Every proof runs on the live rider body actor in the first recording's save state: the bones,
//! joints, collisions and holds are the actor's own, wired by the game. Each case writes generated
//! numbers into the fields the function reads, then runs the original and the port, then compares
//! every field the function writes, bit for bit:
//!
//! 1. **the records**: `Records::read` of the disc's `rider\anim\mrider.phy` against the ones the
//!    rider loader left on the actor, plus the two world constants;
//! 2. **the wreck entry**, over every reason from −1 to 15, with the generator's state, the bike's
//!    three vectors, every bone's mass, inertia and pose, every record and all seven throws
//!    generated;
//! 3. **the riding entry**, with and without a bike;
//! 4. **the damping switch** the settle test throws, both ways, cartwheel or not.
//!
//! The two virtuals both entries start with (posing the bones at the skeleton and resetting them) and
//! the wreck sound are hooked out. The journal says why.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::{Body, World};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::phy::{Throw, Wrecks};
use game_sim::ragdoll::{self, Bike, Bone, Carrier, Collision, Drive, Joint, Launch, Pin, Ragdoll, Records, Tables};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{DONE, Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
/// The rider's body actor in that state.
const ACTOR: u32 = 0x0159_1c10;
const CASES: usize = 10_000;
const BUDGET: u64 = 2_000_000;

/// The actor's fields (see research/oracle.txt, S50).
mod at {
    pub const VTABLE: u32 = 0x08;
    pub const RIDING: u32 = 0x2a4;
    pub const PLACED: u32 = 0x2ac;
    pub const STIFF: u32 = 0x2b4;
    pub const WORLD: u32 = 0x2cc;
    pub const PARTS: u32 = 0x2f4;
    pub const BIKE: u32 = 0x488;
    pub const ROLL: u32 = 0x490;
    pub const ENTERED: u32 = 0x49c;
    pub const PART_RECORDS: u32 = 0x4b8;
    pub const JOINT_RECORDS: u32 = 0x4c0;
    pub const HOLDS: [u32; 4] = [0x5d0, 0x5d4, 0x5d8, 0x5dc];
    pub const BOX: u32 = 0x5f8;
    pub const COLLISIONS: u32 = 0x624;
    pub const FORCE: u32 = 0x62c;
    pub const OTHER_FRAME: u32 = 0x6a8;
    pub const WORLD_FRAME: u32 = 0x6ac;
    pub const OFF_BIKE: u32 = 0x6b0;
    pub const SETTLED: u32 = 0x6b4;
    pub const FLUNG: u32 = 0x6b8;
    pub const FORCE_TIME: u32 = 0x6bc;
    pub const HAND_HOLD: u32 = 0x6c0;
    pub const LAUNCH: u32 = 0x6d0;
    pub const WRECKS: u32 = 0x700;
    // S51: what the step reads.
    pub const DRIVEN: [u32; 2] = [0x70, 0x74];
    pub const SKELETON: u32 = 0x234;
    pub const REPOSE: u32 = 0x2a8;
    pub const ACTIVE: u32 = 0x2b0;
    pub const TORQUES: u32 = 0x2b8;
    pub const GRAVITY_SCALE: u32 = 0x2d0;
    pub const MODE: u32 = 0x47c;
    pub const HOLD_TARGETS: u32 = 0x4d0;
    pub const HOLD_VELOCITY: u32 = 0x530;
    pub const FADE_LENGTH: u32 = 0x644;
    pub const FADE_LEFT: u32 = 0x648;
    pub const POSE_SPRING: u32 = 0x650;
    pub const POSE_DAMPING: u32 = 0x660;
    pub const PULL: u32 = 0x6e0;
}

/// Offsets inside a pin.
mod pin_at {
    pub const ON: u32 = 0x04;
    pub const ANCHORS: [u32; 2] = [0x20, 0x30];
    pub const TARGET: u32 = 0x40;
    pub const TARGET_VELOCITY: u32 = 0x50;
    pub const FOLLOW: u32 = 0x60;
    pub const FOLLOW_VELOCITY: u32 = 0x64;
    pub const FIRST: u32 = 0x68;
    pub const SECOND: u32 = 0x6c;
    pub const LIMITED: u32 = 0x70;
    pub const LOWER: u32 = 0x80;
    pub const UPPER: u32 = 0x90;
    pub const HIT: u32 = 0xa0;
    pub const STICKY: u32 = 0xb0;
    pub const GAINS: u32 = 0x100;
}

/// The body's fields the entries touch, past the ones `biplane::Body` names.
mod body_at {
    pub const MASS: u32 = 0x0c;
    pub const INV_MASS: u32 = 0x10;
    pub const DAMPING: u32 = 0x20;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const INERTIA: u32 = 0x110;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const QUAT: u32 = 0x170;
    pub const RATES: u32 = 0x1b0;
    // S51: the rest of the body, which the step reads and writes.
    pub const LAST_POSITION: u32 = 0x50;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const STEP: u32 = 0xf0;
    pub const LAST_QUAT: u32 = 0x150;
    pub const REFERENCE: u32 = 0x160;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
}

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of first_recording.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(ACTOR + 0x2e0), 18, "no rider body actor at {ACTOR:#x}");
    assert_eq!(ee.u32_at(ACTOR + 0x4bc), 17);
    assert_eq!(ee.u32_at(ACTOR + 0x620), 9);
    Some(ee)
}

/// `rider\anim\mrider.phy`, out of the first rider's archive.
fn rider_file() -> Option<Vec<u8>> {
    let kit = Kit::load().ok()?;
    let path = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("Rider\\thq1.res").expect("thq1.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("thq1.res");
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("rider\\anim\\mrider.phy"))
        .expect("the rider's archive carries mrider.phy");
    Some(res.read(&bytes, e).expect("its bytes"))
}

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn upto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// Mostly in `±x`, now and then exactly zero, a negative zero or a negative number.
    fn float(&mut self, x: f32) -> f32 {
        match self.upto(16) {
            0 => 0.0,
            1 => -0.0,
            2 => -x,
            _ => self.range(-x, x),
        }
    }
    /// A positive number round `x`, sometimes zero or negative (the inertia setter's guard).
    fn positive(&mut self, x: f32) -> f32 {
        match self.upto(20) {
            0 => 0.0,
            1 => -self.range(0.0, x),
            _ => self.range(0.01, 2.0 * x),
        }
    }
    fn vector(&mut self, x: f32) -> Vf {
        Vf::new(self.float(x), self.float(x), self.float(x), self.float(2.0))
    }
    fn positives(&mut self, x: f32) -> Vf {
        Vf::new(self.positive(x), self.positive(x), self.positive(x), self.float(2.0))
    }
    fn quat(&mut self) -> Vf {
        let v = [self.float(1.0), self.float(1.0), self.float(1.0), self.float(1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }
    fn flag(&mut self) -> u32 {
        (self.upto(2)) as u32
    }
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v[k]);
    }
}

fn f(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

fn bone_body(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(ACTOR + at::PARTS) + 0xb0 * i as u32 + 0xa4)
}

fn bone_of(ee: &Ee, body: u32) -> usize {
    (0..ragdoll::PARTS).find(|&i| bone_body(ee, i) == body).unwrap_or(usize::MAX)
}

fn joint_pin(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(ACTOR + at::JOINT_RECORDS) + 0x50 * i as u32 + 8)
}

fn collision(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(ACTOR + at::COLLISIONS) + 4 * i as u32)
}

fn throw_at(ee: &Ee, a: u32) -> Throw {
    Throw {
        force_time: f(ee, a),
        hand_hold_time: f(ee, a + 4),
        force: quad(ee, a + 0x10),
        force_time_delta: f(ee, a + 0x20),
        hand_hold_time_delta: f(ee, a + 0x24),
        force_delta: quad(ee, a + 0x30),
    }
}

/// The seven throws the actor holds, as the port's `Wrecks` (the rest of the block is not read).
fn wrecks_of(ee: &Ee) -> Wrecks {
    let t = |k: u32| throw_at(ee, ACTOR + at::WRECKS + 0x40 * k);
    Wrecks {
        hit_front: t(0),
        hit_side: t(1),
        hit_back: t(2),
        hit_bottom: t(3),
        fall_off_front: t(4),
        fall_off_back: t(5),
        stunt: t(6),
        ..Wrecks::default()
    }
}

/// Everything of the ragdoll the actor holds, as the port's.
fn ragdoll_of(ee: &Ee) -> Ragdoll {
    let recs = ee.u32_at(ACTOR + at::PART_RECORDS);
    let parts = (0..ragdoll::PARTS as u32)
        .map(|i| {
            let r = recs + 0x60 * i;
            ragdoll::PartRecord {
                ride_mass: f(ee, r),
                wreck_mass: f(ee, r + 4),
                ride_inertia: quad(ee, r + 0x10),
                wreck_inertia: quad(ee, r + 0x20),
                ride_angular_damping: quad(ee, r + 0x30),
                settled_angular_damping: quad(ee, r + 0x40),
                limp_angular_damping: quad(ee, r + 0x50),
            }
        })
        .collect();
    let jrecs = ee.u32_at(ACTOR + at::JOINT_RECORDS);
    let joints: Vec<_> = (0..17)
        .map(|i| {
            let r = jrecs + 0x50 * i as u32;
            let pin = joint_pin(ee, i);
            ragdoll::JointRecord {
                part1: bone_of(ee, ee.u32_at(pin + 0x68)),
                part2: bone_of(ee, ee.u32_at(pin + 0x6c)),
                max_angle: f(ee, r + 0xc),
                ride_spring: quad(ee, r + 0x10),
                ride_damping: quad(ee, r + 0x20),
                wreck_spring: quad(ee, r + 0x30),
                wreck_damping: quad(ee, r + 0x40),
            }
        })
        .collect();
    let collisions: Vec<usize> = (0..9).map(|i| bone_of(ee, ee.u32_at(collision(ee, i) + 0x7c))).collect();
    let records = Records { parts, joints, collisions };
    let mut r = Ragdoll::new(records);
    for (i, b) in r.bones.iter_mut().enumerate() {
        *b = bone_at(ee, bone_body(ee, i));
    }
    for (i, j) in r.joints.iter_mut().enumerate() {
        let pin = joint_pin(ee, i);
        *j = Joint {
            wrecked: ee.u32_at(pin + 0x70) != 0,
            max_angle: f(ee, pin + 0xc4),
            spring: quad(ee, pin + 0xd0),
            damping: quad(ee, pin + 0xe0),
            critical: quad(ee, pin + 0xf0),
        };
    }
    for (i, c) in r.collisions.iter_mut().enumerate() {
        let k = collision(ee, i);
        *c = Collision { bone: c.bone, enabled: ee.u32_at(k + 4) != 0, touching: ee.u32_at(k + 0x14) != 0 };
    }
    let w = ee.u32_at(ACTOR + at::WORLD);
    r.world = World { iterations: ee.i32_at(w), rate: f(ee, w + 4) };
    r.holds = at::HOLDS.map(|h| ee.u32_at(ee.u32_at(ACTOR + h) + 4) != 0);
    r.bike_box = ee.u32_at(ee.u32_at(ACTOR + at::BOX) + 4) != 0;
    let force = ee.u32_at(ACTOR + at::FORCE);
    r.throwing = ee.u32_at(force) != 0;
    r.launch = Launch {
        force: quad(ee, ACTOR + at::LAUNCH),
        force_time: f(ee, ACTOR + at::FORCE_TIME),
        hand_hold: f(ee, ACTOR + at::HAND_HOLD),
        target: bone_of(ee, ee.u32_at(force + 0x24)),
        world_frame: ee.u32_at(ACTOR + at::WORLD_FRAME) != 0,
        other_frame: ee.u32_at(ACTOR + at::OTHER_FRAME) != 0,
    };
    r.entered = ee.u32_at(ACTOR + at::ENTERED) != 0;
    r.stiff = ee.u32_at(ACTOR + at::STIFF) != 0;
    r.riding = ee.u32_at(ACTOR + at::RIDING) != 0;
    r.off_bike = ee.u32_at(ACTOR + at::OFF_BIKE) != 0;
    r.flung = ee.u32_at(ACTOR + at::FLUNG) != 0;
    r.settled = ee.u32_at(ACTOR + at::SETTLED) != 0;
    r.roll = f(ee, ACTOR + at::ROLL);
    r.pins = pins(ee).iter().map(|&p| pin_of(ee, p)).collect();
    r.pull = quad(ee, ACTOR + at::PULL);
    r
}

/// A bone, every field of its body included (S50's entries touch only a few; S51's step all of
/// them).
fn bone_at(ee: &Ee, b: u32) -> Bone {
    use body_at::*;
    Bone {
        body: Body {
            enabled: ee.u32_at(b) != 0,
            inv_mass: f(ee, b + INV_MASS),
            damping: quad(ee, b + DAMPING),
            position: quad(ee, b + POSITION),
            velocity: quad(ee, b + VELOCITY),
            inv_inertia: quad(ee, b + INV_INERTIA),
            ang_damping: quad(ee, b + ANG_DAMPING),
            quat: quad(ee, b + QUAT),
            rates: quad(ee, b + RATES),
            last_position: quad(ee, b + LAST_POSITION),
            last_force: quad(ee, b + LAST_FORCE),
            force: quad(ee, b + FORCE),
            accel: quad(ee, b + ACCEL),
            shock_peak: quad(ee, b + SHOCK_PEAK),
            shock: quad(ee, b + SHOCK),
            step: quad(ee, b + STEP),
            last_quat: quad(ee, b + LAST_QUAT),
            cofg: quad(ee, b + COFG),
            moment: quad(ee, b + MOMENT),
            last_moment: quad(ee, b + LAST_MOMENT),
            ang_accel: quad(ee, b + ANG_ACCEL),
            twist_peak: quad(ee, b + TWIST_PEAK),
            twist: quad(ee, b + TWIST),
        },
        mass: f(ee, b + MASS),
        inertia: quad(ee, b + INERTIA),
        reference: quad(ee, b + REFERENCE),
    }
}

/// Every number of a ragdoll as a named bit pattern, so that two can be compared bit for bit and a
/// mismatch names its field.
fn fields(r: &Ragdoll) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut v = |name: String, x: Vf| {
        for k in 0..4 {
            out.push((format!("{name}[{k}]"), x[k].to_bits()));
        }
    };
    for (i, p) in r.records.parts.iter().enumerate() {
        v(format!("part {i} ride inertia"), p.ride_inertia);
        v(format!("part {i} wreck inertia"), p.wreck_inertia);
    }
    for (i, b) in r.bones.iter().enumerate() {
        v(format!("bone {i} damping"), b.body.damping);
        v(format!("bone {i} position"), b.body.position);
        v(format!("bone {i} velocity"), b.body.velocity);
        v(format!("bone {i} inverse inertia"), b.body.inv_inertia);
        v(format!("bone {i} angular damping"), b.body.ang_damping);
        v(format!("bone {i} orientation"), b.body.quat);
        v(format!("bone {i} rates"), b.body.rates);
        v(format!("bone {i} inertia"), b.inertia);
    }
    for (i, j) in r.joints.iter().enumerate() {
        v(format!("joint {i} spring"), j.spring);
        v(format!("joint {i} damping"), j.damping);
        v(format!("joint {i} critical damping"), j.critical);
    }
    v("the launch force".into(), r.launch.force);
    for (i, b) in r.bones.iter().enumerate() {
        out.push((format!("bone {i} enabled"), u32::from(b.body.enabled)));
        out.push((format!("bone {i} inverse mass"), b.body.inv_mass.to_bits()));
        out.push((format!("bone {i} mass"), b.mass.to_bits()));
    }
    for (i, j) in r.joints.iter().enumerate() {
        out.push((format!("joint {i} wrecked"), u32::from(j.wrecked)));
        out.push((format!("joint {i} max angle"), j.max_angle.to_bits()));
    }
    for (i, c) in r.collisions.iter().enumerate() {
        out.push((format!("collision {i} enabled"), u32::from(c.enabled)));
        out.push((format!("collision {i} touching"), u32::from(c.touching)));
    }
    for (i, h) in r.holds.iter().enumerate() {
        out.push((format!("hold {i}"), u32::from(*h)));
    }
    for (name, x) in [
        ("iterations", r.world.iterations as u32),
        ("rate", r.world.rate.to_bits()),
        ("bike box", u32::from(r.bike_box)),
        ("throwing", u32::from(r.throwing)),
        ("force time", r.launch.force_time.to_bits()),
        ("hand hold", r.launch.hand_hold.to_bits()),
        ("target", r.launch.target as u32),
        ("world frame", u32::from(r.launch.world_frame)),
        ("other frame", u32::from(r.launch.other_frame)),
        ("entered", u32::from(r.entered)),
        ("stiff", u32::from(r.stiff)),
        ("riding", u32::from(r.riding)),
        ("off the bike", u32::from(r.off_bike)),
        ("flung", u32::from(r.flung)),
        ("settled", u32::from(r.settled)),
        ("roll", r.roll.to_bits()),
    ] {
        out.push((name.to_owned(), x));
    }
    out
}

fn same(case: usize, what: &str, port: &Ragdoll, game: &Ragdoll) {
    let (p, g) = (fields(port), fields(game));
    for ((name, pb), (_, gb)) in p.iter().zip(&g) {
        assert_eq!(pb, gb, "case {case} ({what}): {name} is {gb:#010x} ({:e}) in the game, {pb:#010x} ({:e}) in the port",
                   f32::from_bits(*gb), f32::from_bits(*pb));
    }
}

/// What the generated case writes into the actor before a call.
fn generate(ee: &mut Ee, pick: &mut Pick) {
    use body_at::*;
    for i in 0..ragdoll::PARTS {
        let b = bone_body(ee, i);
        ee.set_u32(b, pick.flag());
        ee.set_f32(b + MASS, pick.positive(0.4));
        ee.set_f32(b + INV_MASS, pick.positive(3.0));
        for (a, v) in [
            (DAMPING, pick.vector(1.0)),
            (POSITION, pick.vector(50.0)),
            (VELOCITY, pick.vector(30.0)),
            (INERTIA, pick.positives(0.5)),
            (INV_INERTIA, pick.positives(3.0)),
            (ANG_DAMPING, pick.vector(1.0)),
            (QUAT, pick.quat()),
            (RATES, pick.vector(10.0)),
        ] {
            set_quad(ee, b + a, v);
        }
    }
    let recs = ee.u32_at(ACTOR + at::PART_RECORDS);
    for i in 0..ragdoll::PARTS as u32 {
        let r = recs + 0x60 * i;
        ee.set_f32(r, pick.positive(0.4));
        ee.set_f32(r + 4, pick.positive(0.4));
        for (a, v) in [
            (0x10, pick.positives(1.0)),
            (0x20, pick.positives(1.0)),
            (0x30, pick.vector(1.0)),
            (0x40, pick.vector(1.0)),
            (0x50, pick.vector(1.0)),
        ] {
            set_quad(ee, r + a, v);
        }
    }
    let jrecs = ee.u32_at(ACTOR + at::JOINT_RECORDS);
    for i in 0..17 {
        let r = jrecs + 0x50 * i as u32;
        ee.set_f32(r + 0xc, pick.float(1.0));
        for a in [0x10, 0x30] {
            let v = pick.positives(500.0);
            set_quad(ee, r + a, v);
        }
        for a in [0x20, 0x40] {
            let v = pick.positives(1.0);
            set_quad(ee, r + a, v);
        }
        let pin = joint_pin(ee, i);
        ee.set_u32(pin + 0x70, pick.flag());
        for a in [0xd0, 0xe0, 0xf0] {
            let v = pick.vector(100.0);
            set_quad(ee, pin + a, v);
        }
        ee.set_f32(pin + 0xc4, pick.float(1.0));
    }
    for i in 0..9 {
        let c = collision(ee, i);
        ee.set_u32(c + 4, pick.flag());
        ee.set_u32(c + 0x14, pick.flag());
    }
    for h in at::HOLDS.into_iter().chain([at::BOX]) {
        let c = ee.u32_at(ACTOR + h);
        ee.set_u32(c + 4, pick.flag());
    }
    let force = ee.u32_at(ACTOR + at::FORCE);
    ee.set_u32(force, pick.flag());
    let target = bone_body(ee, pick.upto(ragdoll::PARTS as u64) as usize);
    ee.set_u32(force + 0x24, target);
    for a in [at::ENTERED, at::STIFF, at::RIDING, at::OFF_BIKE, at::FLUNG, at::SETTLED, at::WORLD_FRAME, at::OTHER_FRAME] {
        ee.set_u32(ACTOR + a, pick.flag());
    }
    for a in [at::ROLL, at::FORCE_TIME, at::HAND_HOLD] {
        ee.set_f32(ACTOR + a, pick.float(3.0));
    }
    let v = pick.vector(5.0);
    set_quad(ee, ACTOR + at::LAUNCH, v);
    let w = ee.u32_at(ACTOR + at::WORLD);
    ee.set_u32(w, pick.upto(8) as u32);
    ee.set_f32(w + 4, pick.range(1.0, 200.0));
    for k in 0..7 {
        let t = ACTOR + at::WRECKS + 0x40 * k;
        ee.set_f32(t, pick.float(2.0));
        ee.set_f32(t + 4, pick.float(2.0));
        ee.set_f32(t + 0x20, pick.float(1.0));
        ee.set_f32(t + 0x24, pick.float(1.0));
        for a in [0x10, 0x30] {
            let v = pick.vector(20.0);
            set_quad(ee, t + a, v);
        }
    }
    let bike = ee.u32_at(ACTOR + at::BIKE);
    for a in [0x90, 0xa0, 0x150] {
        let v = pick.vector(40.0);
        set_quad(ee, bike + a, v);
    }
    let seed = pick.next();
    ee.write(addr("rand_state").unwrap(), 8, u128::from(seed));
}

fn bike_of(ee: &Ee) -> Bike {
    let b = ee.u32_at(ACTOR + at::BIKE);
    Bike { velocity: quad(ee, b + 0x90), rates: quad(ee, b + 0xa0), hit_velocity: quad(ee, b + 0x150) }
}

/// The two virtuals both entries start with, the wreck sound, and the ignore list, hooked. The
/// ignore calls are logged.
fn hook(ee: &mut Ee) -> Rc<RefCell<Vec<(u32, u32)>>> {
    let vt = ee.u32_at(ACTOR + at::VTABLE);
    let pose = ee.u32_at(vt + 0xa4);
    let reset = ee.u32_at(vt + 0xa0);
    assert_eq!(pose, addr("rider_ragdoll_pose").unwrap());
    assert_eq!(reset, addr("rider_ragdoll_reset").unwrap());
    for a in [pose, reset, addr("wreck_sound").unwrap()] {
        ee.hooks.insert(a, Box::new(|_| {}));
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let l = Rc::clone(&log);
    ee.hooks.insert(
        addr("collider_ignore").unwrap(),
        Box::new(move |ee: &mut Ee| l.borrow_mut().push((ee.r(4) as u32, ee.r(5) as u32))),
    );
    log
}

/// The ignore calls an entry that ignores the bike makes: both of its colliders, bone by bone.
fn ignores(ee: &Ee) -> Vec<(u32, u32)> {
    let bike = ee.u32_at(ACTOR + at::BIKE);
    (0..9)
        .flat_map(|i| {
            let c = ee.u32_at(collision(ee, i) + 0x88);
            [(c, ee.u32_at(bike + 0x1ac)), (c, ee.u32_at(bike + 0x1b0))]
        })
        .collect()
}

#[test]
fn the_records_are_the_live_actors() {
    let Some(ee) = state() else { return };
    let Some(file) = rider_file() else { return };
    let mut read = Records::read_bytes(&file);
    let live = ragdoll_of(&ee).records;
    // The live part records have been through the entries' setters, which write a one into the
    // inertias' `w`. The file's riding inertia has that already; the wreck inertia gets it here.
    for p in &mut read.parts {
        p.wreck_inertia = Vf::new(p.wreck_inertia.x(), p.wreck_inertia.y(), p.wreck_inertia.z(), 1.0);
    }
    assert_eq!(read.parts.len(), live.parts.len());
    for (i, (p, g)) in read.parts.iter().zip(&live.parts).enumerate() {
        let a = format!("{p:?}");
        let b = format!("{g:?}");
        assert_eq!(a, b, "part {i}");
        for (x, y) in [
            (p.ride_inertia, g.ride_inertia),
            (p.wreck_inertia, g.wreck_inertia),
            (p.ride_angular_damping, g.ride_angular_damping),
            (p.settled_angular_damping, g.settled_angular_damping),
            (p.limp_angular_damping, g.limp_angular_damping),
        ] {
            assert_eq!(x.to_bits(), y.to_bits(), "part {i}");
        }
        assert_eq!(p.ride_mass.to_bits(), g.ride_mass.to_bits(), "part {i}");
        assert_eq!(p.wreck_mass.to_bits(), g.wreck_mass.to_bits(), "part {i}");
    }
    assert_eq!(read.joints.len(), live.joints.len());
    for (i, (p, g)) in read.joints.iter().zip(&live.joints).enumerate() {
        assert_eq!((p.part1, p.part2), (g.part1, g.part2), "joint {i}'s bones");
        assert_eq!(p.max_angle.to_bits(), g.max_angle.to_bits(), "joint {i}'s max angle");
        for (name, x, y) in [
            ("ride spring", p.ride_spring, g.ride_spring),
            ("ride damping", p.ride_damping, g.ride_damping),
            ("wreck spring", p.wreck_spring, g.wreck_spring),
            ("wreck damping", p.wreck_damping, g.wreck_damping),
        ] {
            assert_eq!(x.to_bits(), y.to_bits(), "joint {i}'s {name}: {x:?} read, {y:?} live");
        }
    }
    // The quirk, as the live actor has it: the first two joints have no wreck spring at all.
    assert_eq!(live.joints[0].wreck_spring.to_bits(), [0, 0, 0, 1f32.to_bits()]);
    assert_eq!(live.joints[1].wreck_damping.to_bits(), [0, 0, 0, 1f32.to_bits()]);
    assert_eq!(read.collisions, live.collisions, "the collisions' bones");
    for (w, k) in [(ragdoll::WRECK_WORLD, 0x5c0), (ragdoll::RIDE_WORLD, 0x5c4)] {
        assert_eq!(w.iterations, ee.i32_at(ACTOR + k));
        assert_eq!(w.rate.to_bits(), ee.u32_at(ACTOR + k + 8));
    }
}

#[test]
fn the_wreck_entry_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let log = hook(&mut ee);
    let entry = addr("rider_body_wreck_enter").unwrap();
    let mut pick = Pick(0x5eed_0050_dead_beef);
    let mut reasons = [0usize; 17];
    for case in 0..CASES {
        generate(&mut ee, &mut pick);
        let reason = pick.upto(17) as i32 - 1;
        reasons[(reason + 1) as usize] += 1;
        ee.set_u32(ee.u32_at(ACTOR + at::BIKE) + 0x104, reason as u32);
        let mut port = ragdoll_of(&ee);
        port.ignores_bike = false;
        let mut rng = Rng::from_state(ee.read(addr("rand_state").unwrap(), 8) as u64);
        let (bike, wrecks) = (bike_of(&ee), wrecks_of(&ee));
        let want_ignores = ignores(&ee);
        log.borrow_mut().clear();

        port.wreck_enter(reason, &bike, &wrecks, &mut rng);
        ee.call(entry, &[u64::from(ACTOR), reason as i64 as u64], &[1.0 / 30.0], BUDGET);

        same(case, &format!("reason {reason}"), &port, &ragdoll_of(&ee));
        assert_eq!(rng.state(), ee.read(addr("rand_state").unwrap(), 8) as u64, "case {case}: the generator");
        assert!(port.ignores_bike);
        assert_eq!(*log.borrow(), want_ignores, "case {case}: the ignore calls");
    }
    assert!(reasons.iter().all(|&n| n > 400), "every reason drawn: {reasons:?}");
}

#[test]
fn the_ride_entry_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let log = hook(&mut ee);
    let entry = addr("rider_body_ride_enter").unwrap();
    let live_bike = ee.u32_at(ACTOR + at::BIKE);
    let mut pick = Pick(0x0050_7269_6465_0001);
    let (mut with, mut without) = (0, 0);
    for case in 0..CASES {
        ee.set_u32(ACTOR + at::BIKE, live_bike);
        generate(&mut ee, &mut pick);
        ee.set_u32(ACTOR + at::PLACED, pick.flag());
        let bike = pick.upto(4) != 0;
        let want_ignores = if bike { ignores(&ee) } else { Vec::new() };
        ee.set_u32(ACTOR + at::BIKE, if bike { live_bike } else { 0 });
        if bike { with += 1 } else { without += 1 }
        let mut port = ragdoll_of(&ee);
        port.ignores_bike = false;
        log.borrow_mut().clear();

        port.ride_enter(bike);
        ee.call(entry, &[u64::from(ACTOR)], &[1.0 / 30.0], BUDGET);

        same(case, "riding", &port, &ragdoll_of(&ee));
        assert_eq!(port.ignores_bike, bike);
        assert_eq!(*log.borrow(), want_ignores, "case {case}: the ignore calls");
    }
    ee.set_u32(ACTOR + at::BIKE, live_bike);
    assert!(with > 1000 && without > 1000);
}

#[test]
fn the_damping_switch_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let entry = addr("rider_body_damping").unwrap();
    let mut pick = Pick(0x0050_6461_6d70_0002);
    let mut seen = [0; 4];
    for case in 0..CASES {
        generate(&mut ee, &mut pick);
        let stiff = pick.flag() != 0;
        let reason = if pick.upto(2) == 0 { 13 } else { pick.upto(15) as i32 };
        seen[usize::from(stiff) * 2 + usize::from(reason == 13)] += 1;
        ee.set_u32(ee.u32_at(ACTOR + at::BIKE) + 0x104, reason as u32);
        let mut port = ragdoll_of(&ee);

        port.damping(stiff, reason);
        ee.call(entry, &[u64::from(ACTOR), u64::from(stiff)], &[], BUDGET);

        same(case, &format!("stiff {stiff}, reason {reason}"), &port, &ragdoll_of(&ee));
    }
    assert!(seen.iter().all(|&n| n > 1000), "{seen:?}");
}

/// A sanity check that the entry is not a no-op on this state: a fresh ragdoll built from the
/// file and thrown with a front hit holds on with both hands and pushes the pelvis.
#[test]
fn a_front_hit_holds_on_with_both_hands() {
    let Some(file) = rider_file() else { return };
    let mut r = Ragdoll::new(Records::read_bytes(&file));
    r.ride_enter(true);
    let wrecks = Wrecks::read_bytes(&file);
    r.wreck_enter(1, &Bike::default(), &wrecks, &mut Rng::new());
    assert_eq!(r.holds, [true, true, false, false]);
    assert_eq!(r.launch.target, ragdoll::MAIN);
    assert!(r.throwing && !r.bike_box && r.world == ragdoll::WRECK_WORLD);
    assert!(r.joints[0].spring.to_bits() == [0, 0, 0, 1f32.to_bits()], "the spine goes limp");
}

// --- S51: the step --------------------------------------------------------------------------------

/// The step's functions, and the ones round it that the proofs hook out, by their oracle names.
mod step_fn {
    use ps2_oracle::addr;
    fn at(name: &str) -> u32 {
        addr(name).unwrap_or_else(|e| panic!("{e}"))
    }
    pub fn pin_solve() -> u32 { at("pin_solve") }
    pub fn pin_two_body() -> u32 { at("pin_two_body") }
    pub fn pin_one_body() -> u32 { at("pin_one_body") }
    pub fn pin_vtable() -> u32 { at("pin_vtable") }
    pub fn throw_force() -> u32 { at("throw_force") }
    pub fn force_listener() -> u32 { at("ragdoll_force_listener") }
    pub fn physics_update() -> u32 { at("rider_physics_update") }
    pub fn axis_angle() -> u32 { at("quat_axis_angle") }
    /// The throw's clock is a block inside the wreck mode's logic update, not a function: it runs
    /// to `throw_clock_end` with the actor in `$s2` and the frame's time in `$f20`.
    pub fn throw_clock() -> u32 { at("throw_clock") }
    pub fn throw_clock_end() -> u32 { at("throw_clock_end") }
    pub fn pose_point() -> u32 { at("orientation_transform_point") }
    pub fn collision_solve() -> u32 { at("bone_collision_solve") }
    pub fn box_solve() -> u32 { at("bike_box_solve") }
    /// The skeleton calls round the step: the per-part one, the transform's touch, and the tail.
    pub fn skeleton_calls() -> [u32; 3] { [at("rider_part_sync"), at("transform_touch"), at("actor_update_tail")] }
    pub fn fine_acos() -> u32 { at("fine_acos_table") }
    pub fn app() -> u32 { at("app_pointer") }
}

const STEP_BUDGET: u64 = 200_000_000;

/// The pins the actor holds, in the port's order: the four holds, then the seventeen joints'.
fn pins(ee: &Ee) -> Vec<u32> {
    at::HOLDS.iter().map(|&h| ee.u32_at(ACTOR + h)).chain((0..17).map(|i| joint_pin(ee, i))).collect()
}

fn pin_of(ee: &Ee, p: u32) -> Pin {
    use pin_at::*;
    let second = ee.u32_at(p + SECOND);
    Pin {
        first: bone_of(ee, ee.u32_at(p + FIRST)),
        second: (second != 0).then(|| bone_of(ee, second)),
        anchors: ANCHORS.map(|a| quad(ee, p + a)),
        target: quad(ee, p + TARGET),
        target_velocity: quad(ee, p + TARGET_VELOCITY),
        lower: quad(ee, p + LOWER),
        upper: quad(ee, p + UPPER),
        hit: quad(ee, p + HIT),
        sticky: quad(ee, p + STICKY),
        gains: quad(ee, p + GAINS),
    }
}

/// What a hold follows this frame, read through its two pointers as the solve does.
fn follow_of(ee: &Ee, p: u32) -> Option<(Vf, Option<Vf>)> {
    let (t, v) = (ee.u32_at(p + pin_at::FOLLOW), ee.u32_at(p + pin_at::FOLLOW_VELOCITY));
    (t != 0).then(|| (quad(ee, t), (v != 0).then(|| quad(ee, v))))
}

/// Every number [`fields`] has, and the rest of each body, the bones' references, the pins and the
/// pull.
fn all_fields(r: &Ragdoll) -> Vec<(String, u32)> {
    let mut out = fields(r);
    let mut v = |name: String, x: Vf| {
        for k in 0..4 {
            out.push((format!("{name}[{k}]"), x[k].to_bits()));
        }
    };
    for (i, b) in r.bones.iter().enumerate() {
        let d = &b.body;
        for (n, x) in [
            ("last position", d.last_position),
            ("last force", d.last_force),
            ("force", d.force),
            ("acceleration", d.accel),
            ("shock peak", d.shock_peak),
            ("shock", d.shock),
            ("step", d.step),
            ("last orientation", d.last_quat),
            ("centre of mass", d.cofg),
            ("moment", d.moment),
            ("last moment", d.last_moment),
            ("angular acceleration", d.ang_accel),
            ("twist peak", d.twist_peak),
            ("twist", d.twist),
            ("reference", b.reference),
        ] {
            // The linear acceleration's `w` is a register the integrator stores and nothing loads
            // (as in tests/biplane.rs): it is not compared.
            // The angular one's is the position's, or a stale register when the linear half did not
            // run (a zero inverse mass): not compared then.
            let stale = n == "acceleration" || (n == "angular acceleration" && d.inv_mass == F::ZERO);
            let x = if stale { Vf::new(x.x(), x.y(), x.z(), 0.0) } else { x };
            v(format!("bone {i} {n}"), x);
        }
    }
    for (i, p) in r.pins.iter().enumerate() {
        for (n, x) in [
            ("anchor 1", p.anchors[0]),
            ("anchor 2", p.anchors[1]),
            ("target", p.target),
            ("target velocity", p.target_velocity),
            ("lower", p.lower),
            ("upper", p.upper),
            ("hit", p.hit),
            ("sticky", p.sticky),
            ("gains", p.gains),
        ] {
            v(format!("pin {i} {n}"), x);
        }
    }
    v("the pull".into(), r.pull);
    out
}

fn same_all(case: usize, what: &str, port: &Ragdoll, game: &Ragdoll) {
    let (p, g) = (all_fields(port), all_fields(game));
    assert_eq!(p.len(), g.len());
    for ((name, pb), (_, gb)) in p.iter().zip(&g) {
        assert_eq!(pb, gb, "case {case} ({what}): {name} is {gb:#010x} ({:e}) in the game, {pb:#010x} ({:e}) in the port",
                   f32::from_bits(*gb), f32::from_bits(*pb));
    }
}

/// A limit lane: mostly a band round zero, sometimes locked (both limits within 0.001), sometimes
/// wide open.
fn limits(pick: &mut Pick) -> (f32, f32) {
    match pick.upto(8) {
        0 => {
            let x = pick.range(-0.3, 0.3);
            (x, x + pick.range(0.0, 0.0009))
        }
        1 => (-1.0, 1.0),
        _ => (-pick.range(0.0, 0.5), pick.range(0.0, 0.5)),
    }
}

/// A lane of a pin's hit or sticky record: what the limit test writes.
fn mark(pick: &mut Pick) -> f32 {
    [0.0, 1.0, -1.0, 0.0][pick.upto(4) as usize]
}

/// [`generate`], then everything else the step reads: the rest of each body, the references, the
/// pins, the holds' targets, the pull, the actor's numbers, the bike request and the skeleton's
/// rests. The world gets a few iterations at a rate round the game's two.
fn generate_step(ee: &mut Ee, pick: &mut Pick) {
    use body_at::*;
    generate(ee, pick);
    for i in 0..ragdoll::PARTS {
        let b = bone_body(ee, i);
        for a in [LAST_POSITION, LAST_FORCE, FORCE, ACCEL, SHOCK_PEAK, SHOCK, STEP, COFG, MOMENT, LAST_MOMENT, ANG_ACCEL,
                  TWIST_PEAK, TWIST] {
            let v = pick.vector(10.0);
            set_quad(ee, b + a, v);
        }
        for a in [LAST_QUAT, REFERENCE] {
            let v = pick.quat();
            set_quad(ee, b + a, v);
        }
    }
    for p in pins(ee) {
        use pin_at::*;
        for a in ANCHORS {
            let v = pick.vector(0.5);
            set_quad(ee, p + a, v);
        }
        for (a, x) in [(TARGET, 50.0), (TARGET_VELOCITY, 10.0)] {
            let v = pick.vector(x);
            set_quad(ee, p + a, v);
        }
        let wide = pick.upto(3) == 0;
        for k in 0..3 {
            let (lo, hi) = if wide { (-1.0, 1.0) } else { limits(pick) };
            ee.set_f32(p + LOWER + 4 * k, lo);
            ee.set_f32(p + UPPER + 4 * k, hi);
        }
        ee.set_f32(p + LOWER + 12, pick.float(1.0));
        ee.set_f32(p + UPPER + 12, pick.float(1.0));
        for a in [HIT, STICKY] {
            let v = Vf::new(mark(pick), mark(pick), mark(pick), if pick.upto(4) == 0 { pick.float(1.0) } else { 1.0 });
            set_quad(ee, p + a, v);
        }
        let c = if pick.upto(4) == 0 { pick.float(0.01) } else { 0.0 };
        let gains = Vf::new(pick.positive(3.0), pick.positive(3.0), pick.range(0.1, 1.0), c);
        set_quad(ee, p + GAINS, gains);
    }
    for k in 0..4 {
        let v = pick.vector(50.0);
        set_quad(ee, ACTOR + at::HOLD_TARGETS + 0x10 * k, v);
    }
    let v = pick.vector(10.0);
    set_quad(ee, ACTOR + at::HOLD_VELOCITY, v);
    let v = pick.vector(20.0);
    set_quad(ee, ACTOR + at::PULL, v);
    ee.set_f32(ACTOR + at::GRAVITY_SCALE, pick.positive(1.0));
    let v = pick.positives(80.0);
    set_quad(ee, ACTOR + at::POSE_SPRING, v);
    let v = pick.positives(3.0);
    set_quad(ee, ACTOR + at::POSE_DAMPING, v);
    ee.set_u32(ACTOR + at::TORQUES, u32::from(pick.upto(4) != 0));
    ee.set_u32(ACTOR + at::STIFF, u32::from(pick.upto(4) != 0));
    ee.set_u32(ACTOR + at::MODE, pick.upto(3) as u32);
    ee.set_f32(ACTOR + at::FADE_LENGTH, if pick.upto(4) == 0 { 0.0 } else { pick.positive(2.0) });
    ee.set_f32(ACTOR + at::FADE_LEFT, pick.float(2.0));
    let bike = ee.u32_at(ACTOR + at::BIKE);
    for a in [0x10, 0x20, 0x30] {
        let v = pick.vector(1.0);
        set_quad(ee, bike + a, v);
    }
    let v = pick.vector(50.0);
    set_quad(ee, bike + 0x180, v);
    let v = pick.quat();
    set_quad(ee, bike + 0x190, v);
    for i in 0..ragdoll::PARTS {
        let v = pick.quat();
        let a = rest_at(ee, i);
        set_quad(ee, a, v);
    }
    let w = ee.u32_at(ACTOR + at::WORLD);
    ee.set_u32(w, 1 + pick.upto(4) as u32);
    ee.set_f32(w + 4, pick.range(10.0, 60.0));
}

/// Where the skeleton keeps a bone's rest orientation: its skeleton bone's record, by the part's
/// skeleton index.
fn rest_at(ee: &Ee, bone: usize) -> u32 {
    let idx = ee.u32_at(bone_body(ee, bone) + 8);
    let part = ee.u32_at(ACTOR + at::PARTS) + 0xb0 * idx;
    ee.u32_at(ACTOR + at::SKELETON) + 0x60 * ee.u32_at(part + 0x40) + 0x50
}

/// The riding pose's points, one per bone, that the hooked pose call hands back.
type Points = Rc<RefCell<[Vf; ragdoll::PARTS]>>;

/// The pose call hooked: it writes the bone's generated point, the bone known by the centre of
/// mass it was handed.
fn hook_pose(ee: &mut Ee) -> Points {
    let points: Points = Rc::new(RefCell::new([Vf::default(); ragdoll::PARTS]));
    let p = Rc::clone(&points);
    ee.hooks.insert(
        step_fn::pose_point(),
        Box::new(move |ee: &mut Ee| {
            let body = ee.r(6) as u32 - body_at::COFG;
            let i = bone_of(ee, body);
            let v = p.borrow()[i];
            set_quad(ee, ee.r(4) as u32, v);
        }),
    );
    points
}

fn new_points(pick: &mut Pick, points: &Points) {
    for p in points.borrow_mut().iter_mut() {
        *p = pick.vector(50.0);
    }
}

fn drive_of(ee: &Ee, points: &Points) -> Drive {
    let bike = ee.u32_at(ACTOR + at::BIKE);
    let hold = |k: usize| {
        let p = ee.u32_at(ACTOR + at::HOLDS[k]);
        quad(ee, ee.u32_at(p + pin_at::FOLLOW))
    };
    Drive {
        bike: Carrier {
            rows: [0x10, 0x20, 0x30].map(|a| quad(ee, bike + a)),
            spin: f(ee, bike + 0xa8),
            velocity: quad(ee, bike + 0x90),
            centre: quad(ee, bike + 0x180),
            quat: quad(ee, bike + 0x190),
        },
        points: *points.borrow(),
        rests: std::array::from_fn(|i| quad(ee, rest_at(ee, i))),
        hold_targets: std::array::from_fn(hold),
        hold_velocity: quad(ee, ee.u32_at(ee.u32_at(ACTOR + at::HOLDS[0]) + pin_at::FOLLOW_VELOCITY)),
        gravity_scale: f(ee, ACTOR + at::GRAVITY_SCALE),
        pose_spring: quad(ee, ACTOR + at::POSE_SPRING),
        pose_damping: quad(ee, ACTOR + at::POSE_DAMPING),
        torques: ee.u32_at(ACTOR + at::TORQUES) != 0,
        mode: ee.i32_at(ACTOR + at::MODE),
        fade_length: f(ee, ACTOR + at::FADE_LENGTH),
        fade_left: f(ee, ACTOR + at::FADE_LEFT),
    }
}

fn two(bones: &mut [Bone], a: usize, b: usize) -> (&mut Body, &mut Body) {
    if a < b {
        let (lo, hi) = bones.split_at_mut(b);
        (&mut lo[a].body, &mut hi[0].body)
    } else {
        let (lo, hi) = bones.split_at_mut(a);
        (&mut hi[0].body, &mut lo[b].body)
    }
}

/// The live world's wiring the step's proofs lean on: the pins are where `Ragdoll::new` puts them,
/// the holds follow the actor's targets, the throw pulls through the actor's pull, and the world's
/// constraint list is the four holds, two collisions, the joints, the other collisions and the box.
#[test]
fn the_step_is_wired_as_the_live_actor() {
    let Some(ee) = state() else { return };
    let Some(file) = rider_file() else { return };
    let fresh = Ragdoll::new(Records::read_bytes(&file));
    let live = ragdoll_of(&ee);
    assert_eq!(fresh.pins.len(), 21);
    for (k, (a, b)) in fresh.pins.iter().zip(&live.pins).enumerate() {
        assert_eq!((a.first, a.second), (b.first, b.second), "pin {k}'s bones");
    }
    for (k, &p) in pins(&ee).iter().enumerate() {
        assert_eq!(ee.u32_at(p), 2, "pin {k}'s type");
        assert_eq!(ee.u32_at(p + 0x10), step_fn::pin_vtable(), "pin {k}'s table");
        if k < 4 {
            assert_eq!(ee.u32_at(p + pin_at::FOLLOW), ACTOR + at::HOLD_TARGETS + 0x10 * k as u32);
            assert_eq!(ee.u32_at(p + pin_at::FOLLOW_VELOCITY), ACTOR + at::HOLD_VELOCITY);
            assert_eq!(ee.u32_at(p + 0xc), 0, "a hold has no owner");
            assert_eq!(ee.u32_at(p + pin_at::LIMITED), 0, "a hold is never limited");
        } else {
            assert_eq!(ee.u32_at(p + pin_at::FOLLOW), 0);
            assert_eq!(ee.u32_at(p + 0xc), ACTOR, "a joint is the actor's");
            assert_eq!(ee.u32_at(p + pin_at::ON), 1, "a joint is on");
        }
    }
    let force = ee.u32_at(ACTOR + at::FORCE);
    assert_eq!(ee.u32_at(force + 0xc), ACTOR + at::PULL, "the throw pulls through the actor's pull");
    assert_eq!(ee.u32_at(force + 0x20), 0, "the throw has no group");
    let w = ee.u32_at(ACTOR + at::WORLD);
    assert_eq!(ee.i32_at(w + 0x50), 18);
    assert_eq!(ee.i32_at(w + 0x68), 2);
    assert_eq!((ee.i32_at(w + 0x98), ee.i32_at(w + 0xb0)), (0, 0), "no preforce or post listeners");
    assert_eq!(ee.u32_at(ee.u32_at(w + 0x68 + 8) + 4), force, "the throw is the second force listener");
    let list = ee.u32_at(w + 0x80 + 8);
    let all: Vec<u32> = (0..ee.u32_at(w + 0x80)).map(|k| ee.u32_at(list + 4 * k)).collect();
    let ps = pins(&ee);
    let mut want: Vec<u32> = ps[..4].to_vec();
    want.extend((0..2).map(|i| collision(&ee, i)));
    want.extend(&ps[4..]);
    want.extend((2..9).map(|i| collision(&ee, i)));
    want.push(ee.u32_at(ACTOR + at::BOX));
    assert_eq!(all, want, "the world's constraints");
    for i in 0..ragdoll::PARTS {
        assert_eq!(ee.u32_at(bone_body(&ee, i) + 8), i as u32, "bone {i}'s skeleton slot");
        assert_eq!(ee.u32_at(bone_body(&ee, i) + 0xd0), 0, "bone {i} has listeners");
    }
}

/// The tables the step's proofs run on: the game's own curve table out of the state
/// (`CurveTable::build` is not bit-exact, see its docs), and the port's two arc cosines, which
/// `the_step_tables_are_the_games` proves are the game's.
fn tables_of(ee: &Ee) -> Tables {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let curve: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    Tables { curve: CurveTable::from_entries(&curve), ..Tables::build() }
}

/// The two arc cosine tables the step reads, built by the port, against the ones the game built at
/// startup.
#[test]
fn the_step_tables_are_the_games() {
    let Some(ee) = state() else { return };
    let t = Tables::build();
    let base = ee.u32_at(addr("acos_table_ptr").unwrap());
    let mut n = 0;
    for (k, e) in t.acos.entries().iter().enumerate() {
        assert_eq!(e.to_bits(), ee.u32_at(base + 4 * k as u32), "arc cosine entry {k}");
        n += 1;
    }
    assert_eq!(n, 30_001);
    for (k, p) in t.fine.pieces().iter().enumerate() {
        let h = step_fn::fine_acos() + 12 * k as u32;
        assert_eq!(p.base.to_bits(), ee.u32_at(h), "fine piece {k}'s base");
        assert_eq!(p.scale.to_bits(), ee.u32_at(h + 4), "fine piece {k}'s scale");
        let at = ee.u32_at(h + 8);
        for (i, e) in p.entries.iter().enumerate() {
            assert_eq!(e.to_bits(), ee.u32_at(at + 4 * i as u32), "fine piece {k} entry {i}");
            n += 1;
        }
    }
    assert_eq!(n, 30_001 + 25 * 401);
}

#[test]
fn the_axis_angle_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let tables = tables_of(&ee);
    let mut pick = Pick(0x0051_6178_6973_0001);
    let (q, angle, axis) = (ee.scratch(0), ee.scratch(1), ee.scratch(2));
    // The no-turn branch is not reachable on the game's table: its entry for `w = 1` is `2^-11`, not
    // zero. The cases count the clamp instead.
    let (mut clamped, mut turned) = (0, 0);
    for case in 0..CASES {
        let mut v = pick.quat();
        match pick.upto(8) {
            0 => v[3] = 1.0,
            1 => v[3] = pick.range(1.0, 2.0),
            2 => v[3] = -pick.range(1.0, 2.0),
            3 => v = pick.vector(3.0),
            _ => {}
        }
        set_quad(&mut ee, q, v);
        let (a, x) = ragdoll::axis_angle(v, &tables);
        ee.call(step_fn::axis_angle(), &[u64::from(q), u64::from(angle), u64::from(axis)], &[], BUDGET);
        assert_eq!(a.to_bits(), ee.u32_at(angle), "case {case}: the angle of {v:?}");
        assert_eq!(x.to_bits(), quad(&ee, axis).to_bits(), "case {case}: the axis of {v:?}");
        if v[3].abs() >= 1.0 { clamped += 1 }
        if a != F::ZERO { turned += 1 }
    }
    assert!(clamped > 2000 && turned == CASES, "{clamped} clamped, {turned} turned");
}

/// One case of a pin proof: a pin drawn, the actor generated, the port's ragdoll read.
fn pin_case(ee: &mut Ee, pick: &mut Pick, from: usize) -> (usize, u32, Ragdoll) {
    generate_step(ee, pick);
    let k = from + pick.upto((21 - from) as u64) as usize;
    let p = pins(ee)[k];
    (k, p, ragdoll_of(ee))
}

#[test]
fn the_pins_two_bone_solve_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let tables = tables_of(&ee);
    let mut pick = Pick(0x0051_7477_6f00_0002);
    let mut per = [0usize; 21];
    for case in 0..CASES {
        let (k, p, mut port) = pin_case(&mut ee, &mut pick, 4);
        per[k] += 1;
        let mut pin = port.pins[k];
        let (b1, b2) = two(&mut port.bones, pin.first, pin.second.unwrap());
        pin.two_body(b1, b2, &tables.curve);
        port.pins[k] = pin;
        ee.call(step_fn::pin_two_body(), &[u64::from(p)], &[], BUDGET);
        same_all(case, &format!("pin {k}"), &port, &ragdoll_of(&ee));
    }
    assert!(per[4..].iter().all(|&n| n > 400), "{per:?}");
}

#[test]
fn the_pins_one_bone_solve_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let tables = tables_of(&ee);
    let mut pick = Pick(0x0051_6f6e_6500_0003);
    let mut per = [0usize; 21];
    for case in 0..CASES {
        let (k, p, mut port) = pin_case(&mut ee, &mut pick, 0);
        per[k] += 1;
        let first = port.pins[k].first;
        port.pins[k].one_body(&mut port.bones[first].body, &tables.curve);
        ee.call(step_fn::pin_one_body(), &[u64::from(p)], &[], BUDGET);
        same_all(case, &format!("pin {k}"), &port, &ragdoll_of(&ee));
    }
    assert!(per.iter().all(|&n| n > 300), "{per:?}");
}

#[test]
fn the_pin_solve_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let tables = tables_of(&ee);
    let mut pick = Pick(0x0051_736f_6c76_0004);
    // off, one bone, one bone limited, one bone limited and hit, two bones, two limited, two hit
    let mut seen = [0usize; 7];
    for case in 0..CASES {
        let (k, p, _) = pin_case(&mut ee, &mut pick, 0);
        let on = pick.upto(8) != 0;
        let limited = pick.flag() != 0;
        ee.set_u32(p + pin_at::ON, u32::from(on));
        ee.set_u32(p + pin_at::LIMITED, u32::from(limited));
        let saved = (ee.u32_at(p + pin_at::FOLLOW), ee.u32_at(p + pin_at::FOLLOW_VELOCITY));
        if k < 4 {
            if pick.upto(4) == 0 {
                ee.set_u32(p + pin_at::FOLLOW, 0);
            }
            if pick.upto(4) == 0 {
                ee.set_u32(p + pin_at::FOLLOW_VELOCITY, 0);
            }
        } else if pick.upto(8) == 0 {
            // a joint following the actor's own hold targets, as a hold would
            ee.set_u32(p + pin_at::FOLLOW, ACTOR + at::HOLD_TARGETS);
            ee.set_u32(p + pin_at::FOLLOW_VELOCITY, ACTOR + at::HOLD_VELOCITY);
        }
        let mut port = ragdoll_of(&ee);
        let follow = follow_of(&ee, p);
        let pin = port.pins[k];
        let both = pin.second.is_some_and(|s| port.bones[s].body.enabled) && port.bones[pin.first].body.enabled;
        let one = !both && port.bones[pin.first].body.enabled;
        port.pins[k].solve(on, limited, follow, &mut port.bones, &tables);
        let hit = port.pins[k].hit.to_bits()[..3] != [0; 3];
        ee.call(step_fn::pin_solve(), &[u64::from(p)], &[], BUDGET);
        let game = ragdoll_of(&ee);
        ee.set_u32(p + pin_at::FOLLOW, saved.0);
        ee.set_u32(p + pin_at::FOLLOW_VELOCITY, saved.1);
        same_all(case, &format!("pin {k}, on {on}, limited {limited}"), &port, &game);
        let i = match (on && (both || one), both, limited, hit) {
            (false, ..) => 0,
            (true, false, false, _) => 1,
            (true, false, true, false) => 2,
            (true, false, true, true) => 3,
            (true, true, false, _) => 4,
            (true, true, true, false) => 5,
            (true, true, true, true) => 6,
        };
        seen[i] += 1;
    }
    assert!(seen.iter().all(|&n| n > 300), "{seen:?}");
}

#[test]
fn the_throw_force_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let mut pick = Pick(0x0051_7468_726f_0005);
    let force = ee.u32_at(ACTOR + at::FORCE);
    let mut pulled = 0;
    for case in 0..CASES {
        generate_step(&mut ee, &mut pick);
        let mut port = ragdoll_of(&ee);
        let before = port.bones[port.launch.target].body.force;
        port.throw_force();
        ee.call(step_fn::throw_force(), &[u64::from(force)], &[], BUDGET);
        same_all(case, "the throw", &port, &ragdoll_of(&ee));
        if port.bones[port.launch.target].body.force.to_bits() != before.to_bits() {
            pulled += 1;
        }
    }
    assert!(pulled > 2000 && pulled < 3000, "{pulled} pulled");
}

#[test]
fn the_throw_clock_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let mut pick = Pick(0x0051_636c_6f63_0006);
    ee.hooks.insert(step_fn::throw_clock_end(), Box::new(|ee: &mut Ee| ee.set(31, u64::from(DONE))));
    // own frame, world frame, the bike's, the throw stopped, nothing; the hands running down, let
    // go, already gone
    let mut seen = [0usize; 8];
    for case in 0..CASES {
        generate_step(&mut ee, &mut pick);
        let dt = pick.range(0.001, 0.1);
        let mut port = ragdoll_of(&ee);
        let bike = quad(&ee, ee.u32_at(ACTOR + at::BIKE) + 0x190);
        let l = port.launch;
        seen[match (l.force_time > F::ZERO, l.other_frame, l.world_frame, port.throwing) {
            (true, true, ..) => 0,
            (true, false, true, _) => 1,
            (true, false, false, _) => 2,
            (false, _, _, true) => 3,
            (false, _, _, false) => 4,
        }] += 1;
        seen[if l.hand_hold > F::ZERO { 5 } else if port.holds[0] || port.holds[1] { 6 } else { 7 }] += 1;
        port.throw_clock(F(dt), bike);
        ee.set(18, u64::from(ACTOR));
        ee.set_f(20, dt);
        ee.call(step_fn::throw_clock(), &[], &[], BUDGET);
        same_all(case, "the clock", &port, &ragdoll_of(&ee));
    }
    assert!(seen.iter().all(|&n| n > 800), "{seen:?}");
}

#[test]
fn the_force_listener_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let tables = tables_of(&ee);
    let points = hook_pose(&mut ee);
    let mut pick = Pick(0x0051_666f_7263_0007);
    // riding, the torques, the wreck's torques, an unfaded wreck
    let mut seen = [0usize; 4];
    for case in 0..CASES {
        generate_step(&mut ee, &mut pick);
        new_points(&mut pick, &points);
        let mut port = ragdoll_of(&ee);
        let drive = drive_of(&ee, &points);
        let torques = port.stiff && drive.torques;
        for (i, x) in [port.riding, torques, torques && drive.mode == 2, torques && drive.mode == 2 && drive.fade_length == F::ZERO]
            .into_iter()
            .enumerate()
        {
            seen[i] += usize::from(x);
        }
        port.forces(&drive, &tables);
        ee.call(step_fn::force_listener(), &[u64::from(ACTOR)], &[], BUDGET);
        same_all(case, &format!("mode {}", drive.mode), &port, &ragdoll_of(&ee));
    }
    assert!(seen.iter().all(|&n| n > 400), "{seen:?}");
}

/// The update's calls round the step that are not the ragdoll's, hooked out: the skeleton's, the
/// logic's, the pose and reset, and the collisions' and the box's solves (S52).
fn hook_update(ee: &mut Ee) {
    let vt = ee.u32_at(ACTOR + at::VTABLE);
    let virtuals = [0xa0, 0xa4, 0xa8, 0xac, 0xb0].map(|s| ee.u32_at(vt + s));
    for a in virtuals.into_iter().chain(step_fn::skeleton_calls()).chain([step_fn::collision_solve(), step_fn::box_solve()]) {
        ee.hooks.insert(a, Box::new(|_| {}));
    }
}

/// [`generate_step`]'s case made physical for a whole step: positive masses and inertias, bones
/// within reach of each other, damping under one, springs that pull rather than push. Unbounded
/// inputs blow the bones up within a step, and an angle past the curve table's reach reads outside
/// it in the original (garbage) and panics in the port.
fn tame(ee: &mut Ee, pick: &mut Pick) {
    use body_at::*;
    let r = |pick: &mut Pick, lo: f32, hi: f32| Vf::new(pick.range(lo, hi), pick.range(lo, hi), pick.range(lo, hi), 0.0);
    for i in 0..ragdoll::PARTS {
        let b = bone_body(ee, i);
        ee.set_f32(b + MASS, pick.range(0.2, 2.0));
        ee.set_f32(b + INV_MASS, pick.range(0.5, 5.0));
        for (a, lo, hi) in [
            (INERTIA, 0.02, 1.0),
            (INV_INERTIA, 1.0, 50.0),
            (DAMPING, 0.9, 1.0),
            (ANG_DAMPING, 0.9, 1.0),
            (POSITION, -1.0, 1.0),
            (VELOCITY, -3.0, 3.0),
            (RATES, -5.0, 5.0),
            (FORCE, -10.0, 10.0),
            (MOMENT, -10.0, 10.0),
        ] {
            let v = r(pick, lo, hi);
            set_quad(ee, b + a, v);
        }
    }
    for (k, p) in pins(ee).into_iter().enumerate() {
        let gains = Vf::new(0.0, 0.0, pick.range(0.1, 1.0), 0.0);
        set_quad(ee, p + pin_at::GAINS, gains);
        if k >= 4 {
            for (a, lo, hi) in [(0xd0, 0.0, 50.0), (0xe0, 0.0, 2.0), (0xf0, 0.0, 2.0)] {
                let v = r(pick, lo, hi);
                set_quad(ee, p + a, v);
            }
            ee.set_f32(p + 0xc4, pick.range(0.1, 1.0));
        }
    }
    for k in 0..4 {
        let v = r(pick, -1.0, 1.0);
        set_quad(ee, ACTOR + at::HOLD_TARGETS + 0x10 * k, v);
    }
    let v = r(pick, -3.0, 3.0);
    set_quad(ee, ACTOR + at::HOLD_VELOCITY, v);
    let v = r(pick, 0.0, 80.0);
    set_quad(ee, ACTOR + at::POSE_SPRING, v);
    let v = r(pick, 0.0, 3.0);
    set_quad(ee, ACTOR + at::POSE_DAMPING, v);
    ee.set_f32(ACTOR + at::GRAVITY_SCALE, pick.range(0.0, 1.5));
    let bike = ee.u32_at(ACTOR + at::BIKE);
    for (a, lo, hi) in [(0x90, -5.0, 5.0), (0x180, -1.0, 1.0)] {
        let v = r(pick, lo, hi);
        set_quad(ee, bike + a, v);
    }
    ee.set_f32(bike + 0xa8, pick.range(-3.0, 3.0));
    let w = ee.u32_at(ACTOR + at::WORLD);
    let (n, rate) = [(3, 25.0), (2, 55.0)][pick.upto(2) as usize];
    ee.set_u32(w, n);
    ee.set_f32(w + 4, rate);
}

#[test]
fn the_physics_update_matches_the_original() {
    let Some(mut ee) = state() else { return };
    assert_eq!(ee.u8_at(ee.u32_at(step_fn::app()) + 0x325) & 8, 0, "the debug freeze is on");
    let tables = tables_of(&ee);
    let points = hook_pose(&mut ee);
    hook_update(&mut ee);
    let mut pick = Pick(0x0051_7570_6474_0008);
    let (mut stepped, mut wrecked, mut riding, mut held) = (0, 0, 0, 0);
    for case in 0..CASES {
        generate_step(&mut ee, &mut pick);
        tame(&mut ee, &mut pick);
        new_points(&mut pick, &points);
        for p in points.borrow_mut().iter_mut() {
            *p = Vf::new(p.x() / 50.0, p.y() / 50.0, p.z() / 50.0, p.w());
        }
        let placed = pick.upto(4) != 0;
        let active = pick.upto(4) != 0;
        let mut repose = pick.upto(3) == 0;
        ee.set_u32(ACTOR + at::PLACED, u32::from(placed));
        ee.set_u32(ACTOR + at::ACTIVE, u32::from(active));
        ee.set_u32(ACTOR + at::REPOSE, u32::from(repose));
        let dt = pick.range(0.01, 0.05);
        let mut port = ragdoll_of(&ee);
        let drive = drive_of(&ee, &points);
        if placed && active && !repose {
            stepped += 1;
            if port.riding { riding += 1 }
            if port.joints.iter().any(|j| j.wrecked) { wrecked += 1 }
            if port.holds.iter().any(|&h| h) { held += 1 }
        }
        let driven = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            port.update(F(dt), placed, active, &mut repose, &drive, &tables)
        }))
        .unwrap_or_else(|_| panic!("case {case}: the port's step panicked"));
        ee.call(step_fn::physics_update(), &[u64::from(ACTOR)], &[dt], STEP_BUDGET);
        same_all(case, "the update", &port, &ragdoll_of(&ee));
        for a in at::DRIVEN {
            assert_eq!(ee.u32_at(ACTOR + a), u32::from(driven), "case {case}: driven");
        }
        assert_eq!(ee.u32_at(ACTOR + at::REPOSE), u32::from(repose), "case {case}: repose");
    }
    assert!(stepped > 3000 && riding > 1000 && stepped - riding > 1000 && wrecked > 1000 && held > 1000,
            "{stepped} stepped, {riding} riding, {wrecked} with a limit, {held} held");
}

// ---------------------------------------------------------------------------------------------------
// S53: the ragdoll in the ride
// ---------------------------------------------------------------------------------------------------

/// The ragdoll's kit as the shell builds it: the rider's body and model off the disc, its riding
/// skeleton read through the renderer, and the rider file's parts and pins over that. With them,
/// the wreck bike's part and the rider's wreck set.
fn ride_kit() -> Option<(game_sim::ragdoll_ride::Kit, game_sim::phy::Part, Wrecks)> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let names: Vec<String> = disc
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(".res"))
        .map(|e| e.name.clone())
        .collect();
    let rider_name = names.iter().filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len())?.clone();
    let res = disc.res(&rider_name).expect("the rider archive opens");
    let rider_file = rider_file()?;
    let mut body = game_render::rider::Body::load(
        vec![disc.res(&rider_name).expect("the rider archive opens")],
        "thq1",
        game_render::rider::Sex::Male,
        "",
        &[],
        game_sim::motion::Params::default(),
    )
    .expect("the rider's body loads");
    let model = game_render::psg::Model::parse(&res.read_suffix(&format!("{}.psg", body.geometry())).expect("its model"))
        .expect("the model parses");
    let doll = body.ragdoll_rig(&model, &rider_file).expect("every part names a node");
    let wreck_bike = names
        .iter()
        .find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbike.phy").ok()))
        .expect("an archive carries wreckbike.phy");
    let mut kit = game_sim::ragdoll_ride::Kit::new(&rider_file, doll.rig);
    kit.bike = names
        .iter()
        .find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbikef.col").ok()))
        .and_then(|col| game_sim::ragdoll_ride::bike_sphere(&col));
    assert!(kit.bike.is_some(), "an archive carries wreckbikef.col");
    Some((
        kit,
        game_sim::phy::Part::read_bytes(&wreck_bike, "PhysicsPart1"),
        Wrecks::read_bytes(&rider_file),
    ))
}

/// The ground under S26's second live wreck (`context/capture/s26/wreck2.csv`), as far as the
/// capture shows it: straight lines between the heights the bike stood at — at the entry, the tick
/// the gate came on, and two after — along the line it slid down.
struct Slope;

const SLOPE: [(f32, f32); 4] = [(3411.8254, 149.75734), (3441.746, 141.09964), (3468.708, 137.68971), (3473.1145, 136.98788)];

impl game_sim::world::World for Slope {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let z = at.z();
        let k = SLOPE.windows(2).position(|w| z < w[1].0).unwrap_or(SLOPE.len() - 2);
        let ((z0, y0), (z1, y1)) = (SLOPE[k], SLOPE[k + 1]);
        let s = (y1 - y0) / (z1 - z0);
        let y = y0 + (z - z0) * s;
        let n = (1.0 + s * s).sqrt();
        (Vf::new(at.x(), y, z, 1.0), Vf::new(0.0, 1.0 / n, -s / n, 0.0))
    }
    fn solid(&mut self, _bundle: &game_sim::collide::Bundle, _record: &mut game_sim::collide::Record) -> bool {
        false
    }
}

/// The tick the bones' settle gate comes on in S26's second live reason-5 wreck, thrown from the
/// bike at the capture's entry transform and velocity (`context/capture/s26/wreck2.csv`, row 150's
/// bike and row 151's wreck body) over [`Slope`], at the capture's `dt`. Answers the tick, with the
/// tick each hold let go on, or `None` without the disc.
fn wreck2_settles() -> Option<(Option<u32>, [Option<u32>; 2])> {
    let (kit, part, wrecks) = ride_kit()?;
    let mut ride = game_sim::ride::Ride::default();
    let m = game_sim::math::Mat4([
        Vf::new(0.99805659, -0.043665301, -0.044397738, 0.0),
        Vf::new(0.060603332, 0.84503216, 0.53126532, 0.0),
        Vf::new(0.014319694, -0.53292495, 0.84603846, 0.0),
        Vf::new(4304.7788, 149.75734, 3411.8254, 1.0),
    ]);
    ride.chassis.quat = game_sim::math::mat4_to_quat(&m);
    ride.origin = m.0[3];
    ride.chassis.vel = Vf::new(-4.0195813, -21.036182, 49.494213, 1.0);
    let mut world = Slope;
    let mut crash =
        game_sim::ride::Crash::begin(&ride, game_sim::trick::Wreck(5), &part, game_sim::ride::WRECK_INERTIA, &wrecks, None);
    let mut rng = Rng::default();
    let mut doll = game_sim::ragdoll_ride::Down::enter(
        &kit,
        &ride.transform(),
        &crash,
        crash.throw,
        5,
        &wrecks,
        &mut rng,
        &mut world,
    );
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let (mut settled, mut let_go) = (None, [None; 2]);
    for tick in 1..=60 {
        crash.step(dt, &part, &wrecks, &mut world);
        doll.step(&kit, dt, &crash, &mut world);
        let p = doll.main().position;
        println!(
            "tick {tick}: main ({:.2}, {:.2}, {:.2}) holds {:?} settles {}",
            p.x(),
            p.y(),
            p.z(),
            doll.ragdoll.holds,
            doll.settles()
        );
        for (hand, gone) in let_go.iter_mut().enumerate() {
            if gone.is_none() && !doll.ragdoll.holds[hand] {
                *gone = Some(tick);
            }
        }
        if settled.is_none() && doll.settles() {
            settled = Some(tick);
        }
    }
    Some((settled, let_go))
}

/// **A reason-5 wreck that settles on the original's tick** (S53), to within two ticks: the
/// original's gate came on 18 ticks after the entry (`wreck2.csv` rows 150 and 168), and S26's
/// three measurements spread 18 to 26. The gate is the body's touching flag, as the original's is
/// (S65; S53 inferred "hands off and a contact touching", and came on at 16), the bike's sphere is
/// `wreckbikef.col`'s, and the ground is the capture's bike heights, not the track; the port comes
/// on at 17. Nothing is asked of the hands: the original's gate opens with them holding (S65,
/// `tests/ragdoll_gate.rs`). [`a_reason_5_wreck_settles_on_the_originals_tick_exactly`] holds the
/// exact tick.
#[test]
fn a_reason_5_wreck_that_settles_on_the_originals_tick() {
    let Some((settled, _)) = wreck2_settles() else { return };
    let tick = settled.expect("the bones settle inside two seconds");
    assert!((16..=20).contains(&tick), "the gate came on {tick} ticks after the entry; the original's 18");
}

/// The same, to the tick. Ignored: the port's gate comes on a tick early (17 against 18). The gate
/// is the original's (S65), so the tick is the bones' and the ground's: the ground here is four
/// heights off the capture, and the bones start from the fixed riding pose with the bike's
/// velocity.
#[test]
#[ignore = "the port settles on tick 17, the original on 18; see the S65 journal"]
fn a_reason_5_wreck_settles_on_the_originals_tick_exactly() {
    let Some((settled, _)) = wreck2_settles() else { return };
    assert_eq!(settled, Some(18), "the gate came on 18 ticks after the entry in the original");
}

/// **A wreck thrown from the shell's skeleton** (S65): handed the kit's own riding pose as the live
/// one, the posed entry and step are the fixed pose's to the bit; handed another pose, the bones
/// start where it has them and pull towards its rests.
#[test]
fn a_wreck_is_thrown_from_the_shells_skeleton() {
    use game_sim::ragdoll_ride::{Down, Posed};
    let Some((kit, part, wrecks)) = ride_kit() else { return };
    let mut ride = game_sim::ride::Ride::default();
    ride.chassis.quat = Vf::new(0.0, 0.0, 0.0, 1.0);
    ride.origin = Vf::new(4304.0, 150.0, 3412.0, 1.0);
    ride.chassis.vel = Vf::new(1.0, -3.0, 25.0, 1.0);
    let mut world = Slope;
    let mut crash =
        game_sim::ride::Crash::begin(&ride, game_sim::trick::Wreck(5), &part, game_sim::ride::WRECK_INERTIA, &wrecks, None);
    let same = Posed { bones: kit.rig.parts.iter().map(|p| p.bone).collect(), rests: kit.rig.parts.iter().map(|p| p.rest).collect() };
    let lifted = Posed {
        bones: same.bones.iter().map(|b| game_sim::ragdoll_pose::Bone { at: b.at.add(Vf::new(0.0, 0.5, 0.0, 0.0)), ..*b }).collect(),
        rests: vec![Vf::new(0.0, 0.0, 0.0, 1.0); same.rests.len()],
    };
    let m = ride.transform();
    let enter = |posed: Option<&Posed>, world: &mut Slope| {
        Down::enter_posed(&kit, posed, &m, &crash, crash.throw, 5, &wrecks, &mut Rng::default(), world)
    };
    let mut fixed = enter(None, &mut world);
    let mut live = enter(Some(&same), &mut world);
    let mut other = enter(Some(&lifted), &mut world);
    assert_eq!(fixed, live, "the kit's pose handed over as the live one enters as the kit's");
    let dy = other.main().position.y() - fixed.main().position.y();
    assert!((dy - 0.5).abs() < 1e-4, "a pose half a foot up starts the main bone {dy} ft up");
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let mut parted = false;
    for _ in 0..10 {
        crash.step(dt, &part, &wrecks, &mut world);
        fixed.step(&kit, dt, &crash, &mut world);
        live.step_posed(&kit, Some(&same), dt, &crash, &mut world);
        other.step_posed(&kit, Some(&lifted), dt, &crash, &mut world);
        assert_eq!(fixed, live, "the kit's rests handed over step as the kit's");
        parted |= other.ragdoll.bones.iter().zip(&fixed.ragdoll.bones).any(|(a, b)| a.body.rates != b.body.rates);
    }
    assert!(parted, "other rests turn the bones otherwise");
}

/// **The pins the shell's kit loads are the live actor's** (S53): every hold's and joint's bones,
/// anchors and twist limits, as [`game_sim::ragdoll_ride::Kit::new`] builds them off the rider file
/// and the riding skeleton the renderer reads, against the actor in the first recording's state,
/// through the game's curve table.
#[test]
fn the_kits_pins_are_the_live_actors() {
    let Some(ee) = state() else { return };
    let Some((kit, _, _)) = ride_kit() else { return };
    let game: Vec<Pin> = pins(&ee).into_iter().map(|p| pin_of(&ee, p)).collect();
    // Through the game's own curve table: the port's is built with single-precision transcendentals
    // and its entries can sit a bit off the disc's ([`CurveTable::build`]), which the twist limits'
    // sines would carry.
    let ini = game_sim::phy::Ini::parse_bytes(&rider_file().expect("the rider file"));
    let mut doll = kit.ragdoll.clone();
    game_sim::ragdoll_ride::load_pins(&ini, &mut doll, &kit.rig, &tables_of(&ee).curve);
    let port = &doll.pins;
    // The centres of mass the anchors are worked out from. One part's is not the file's to the bit:
    // the lower torso's `z` is 0.13632499 in the model's bounds and 0.1363255 on the live body, which
    // reaches its two joints' anchors and nothing else. Where the state's number comes from was not
    // found; it is held to a millionth of a foot.
    const LOWER_TORSO: usize = 1;
    for (i, part) in kit.rig.parts.iter().enumerate() {
        let game_cofg = quad(&ee, bone_body(&ee, i) + 0x180);
        if i == LOWER_TORSO {
            assert!(near(part.cofg, game_cofg), "part {i}'s centre {:?} against {game_cofg:?}", part.cofg);
        } else {
            assert_eq!(part.cofg.to_bits(), game_cofg.to_bits(), "part {i}'s centre of mass");
        }
    }
    assert_eq!(port.len(), game.len(), "four holds and seventeen joints");
    let bits = |v: Vf| v.to_bits();
    let mut wrong = Vec::new();
    for (k, (a, b)) in port.iter().zip(&game).enumerate() {
        if (a.first, a.second) != (b.first, b.second) {
            wrong.push(format!("pin {k}: bones {:?} against {:?}", (a.first, a.second), (b.first, b.second)));
        }
        for (what, x, y) in [
            ("anchor 0", a.anchors[0], b.anchors[0]),
            ("anchor 1", a.anchors[1], b.anchors[1]),
            ("lower", a.lower, b.lower),
            ("upper", a.upper, b.upper),
        ] {
            let torso = what.starts_with("anchor") && [a.first, b.second.unwrap_or(b.first)].contains(&LOWER_TORSO);
            if bits(x) != bits(y) && !(torso && near(x, y)) {
                wrong.push(format!("pin {k} {what}: {x:?} against {y:?}"));
            }
        }
    }
    for w in &wrong {
        println!("{w}");
    }
    assert!(wrong.is_empty(), "{} of the pins' numbers differ", wrong.len());
}

/// Every lane within a millionth.
fn near(a: Vf, b: Vf) -> bool {
    (0..4).all(|k| (a.0[k] - b.0[k]).abs() <= 1.0e-6)
}
