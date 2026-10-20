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
use game_sim::phy::{Throw, Wrecks};
use game_sim::ragdoll::{self, Bike, Bone, Collision, Joint, Launch, Ragdoll, Records};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
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
    r
}

/// The fields of a bone body the entries read or write. The rest of `Body` is left at its default
/// on both sides: neither entry touches it.
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
            ..Body::default()
        },
        mass: f(ee, b + MASS),
        inertia: quad(ee, b + INERTIA),
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
