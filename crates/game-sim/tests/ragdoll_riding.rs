//! S109 — the rider's ragdoll **while riding**, tick by tick, against the user's animandpump ride.
//!
//! The original runs the rider's eighteen bones on every tick, not only in a wreck: the riding entry
//! sets each bone's riding mass and damping, the four holds pin the hands to the grips and the feet
//! to the pegs, the bike box keeps the pelvis round the seat, and the riding pose's spring pulls each
//! bone to where the animated skeleton has it, carried round with the bike. After the step every
//! bone is written back into the skeleton's node, which is what is drawn: the body that leans and
//! sways on the bike is the ragdoll's, not the clip's.
//!
//! The capture (`context/capture/s109/run1.csv`, one row a tick, made by the scripts in
//! `research/journal/2026-10-23-s109-animandpump/capture/`) is the user's animandpump.p2m2 played
//! back on the original from its own save state, 1204 ticks of riding with no wreck. The state's RAM image (`./kit state ram context/recordings/animandpump.p2m2_SaveState.p2s
//! --out context/capture/s109/start.ee.bin`) and its scratchpad (`start.spr.bin`) are the tick before
//! the first row.

use game_sim::rng::Rng;
use ps2_float::Vf;
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::HashMap;
use std::path::PathBuf;

/// The rider body actor in animandpump's state (the human's vehicle, the seventh actor, less 0x400).
const ACTOR: u32 = 0x015b_96f0;
const PARTS: u32 = 0x2f4;
/// The state's own frame counter: the capture's first row is the tick after it.
const START: u32 = 1938;

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (S109's capture: research/journal/2026-10-23-s109-animandpump)", path.display()));
        return None;
    }
    Some(path)
}

/// Where an exploration writes what it found (a passing check keeps no log).
fn note(text: &str) {
    use std::io::Write;
    let Some(path) = ctx("capture/s109").map(|d| d.join("notes.txt")) else { return };
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("the notes");
    writeln!(f, "{text}").expect("write the notes");
}

/// A capture: its header's columns and its rows by frame counter.
struct Capture {
    cols: HashMap<String, usize>,
    rows: HashMap<u32, Vec<String>>,
}

impl Capture {
    fn load(name: &str) -> Option<Capture> {
        let text = std::fs::read_to_string(ctx(&format!("capture/s109/{name}.csv"))?).expect("read the capture");
        let mut lines = text.lines();
        let cols: HashMap<String, usize> =
            lines.next().expect("a header").split(',').enumerate().map(|(i, c)| (c.to_string(), i)).collect();
        let fc = cols["frame_counter"];
        let rows = lines
            .map(|l| l.split(',').map(str::to_string).collect::<Vec<_>>())
            .map(|r| (r[fc].parse().expect("a frame counter"), r))
            .collect();
        Some(Capture { cols, rows })
    }

    fn text(&self, frame: u32, col: &str) -> &str {
        &self.rows[&frame][*self.cols.get(col).unwrap_or_else(|| panic!("no column {col}"))]
    }

    fn get(&self, frame: u32, col: &str) -> f32 {
        let s = self.text(frame, col);
        s.parse::<f64>().unwrap_or_else(|_| panic!("{col} at {frame}: {s:?}")) as f32
    }

    fn int(&self, frame: u32, col: &str) -> u64 {
        let s = self.text(frame, col);
        s.parse().unwrap_or_else(|_| panic!("{col} at {frame}: {s:?}"))
    }

    fn quad(&self, frame: u32, col: &str) -> Vf {
        Vf::new(
            self.get(frame, &format!("{col}.x")),
            self.get(frame, &format!("{col}.y")),
            self.get(frame, &format!("{col}.z")),
            self.get(frame, &format!("{col}.w")),
        )
    }

    fn bytes(&self, frame: u32, col: &str) -> Vec<u8> {
        let s = self.text(frame, col);
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
    }

    fn has(&self, frame: u32) -> bool {
        self.rows.contains_key(&frame)
    }
}

fn state() -> Option<Ee> {
    let path = ctx("capture/s109/start.ee.bin")?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Some(spr) = ctx("capture/s109/start.spr.bin") {
        ee.spr = std::fs::read(spr).expect("the scratchpad");
    }
    assert_eq!(ee.u32_at(ACTOR + 0x2e0), 18, "no rider body actor at {ACTOR:#x}");
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), START, "the state is not frame {START}");
    Some(ee)
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn bone_body(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(ACTOR + PARTS) + 0xb0 * i as u32 + 0xa4)
}

/// What the capture holds of each bone's body, by column and offset.
const BODY: [(&str, u32); 6] =
    [("last_pos", 0x50), ("pos", 0x60), ("vel", 0x70), ("last_quat", 0x150), ("quat", 0x170), ("rates", 0x1b0)];

/// Each bone's captured lanes that differ from `bones` (per bone, its six quads), as names.
fn differ(cap: &Capture, frame: u32, bones: &[[Vf; 6]]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, b) in bones.iter().enumerate() {
        for (k, (name, _)) in BODY.iter().enumerate() {
            let want = cap.quad(frame, &format!("bone{i}.{name}"));
            for l in 0..4 {
                if want[l].to_bits() != b[k][l].to_bits() {
                    out.push(format!("bone{i}.{name}[{l}] {:e} want {:e}", b[k][l], want[l]));
                }
            }
        }
    }
    out
}

fn ee_bones(ee: &Ee) -> Vec<[Vf; 6]> {
    (0..18).map(|i| BODY.map(|(_, o)| quad(ee, bone_body(ee, i) + o))).collect()
}

fn set_bytes(ee: &mut Ee, a: u32, bytes: &[u8]) {
    let at = (a & 0x01ff_ffff) as usize;
    ee.ram[at..at + bytes.len()].copy_from_slice(bytes);
}

/// What the body's update reads off the rest of the game in tick `frame`, laid into `ee` from the
/// capture: the riding request (and the hand dummies 0x200 before it) and the bike round the record
/// its +0x1ac points at.
fn lay_in(ee: &mut Ee, cap: &Capture, frame: u32) {
    let req = ee.u32_at(ACTOR + 0x488);
    set_bytes(ee, req - 0x200, &cap.bytes(frame, "req_area"));
    let far = ee.u32_at(req + 0x1ac);
    set_bytes(ee, far - 0x980, &cap.bytes(frame, "bike_a"));
    set_bytes(ee, far + 0x680, &cap.bytes(frame, "bike_b"));
    set_bytes(ee, far, &cap.bytes(frame, "req_far"));
}

fn rng_at(ee: &Ee) -> u64 {
    let at = addr("rand_state").unwrap();
    u64::from(ee.u32_at(at)) | u64::from(ee.u32_at(at + 4)) << 32
}

fn set_rng(ee: &mut Ee, state: u64) {
    set_bytes(ee, addr("rand_state").unwrap(), &state.to_le_bytes());
}

/// The draws between two states of the generator, if `to` is `from` advanced fewer than `most` times.
fn draws(from: u64, to: u64, most: u32) -> Option<u32> {
    let mut r = Rng::from_state(from);
    (0..most).find(|_| {
        let hit = r.state() == to;
        r.rand();
        hit
    })
}

/// Everything a call can change, to run a tick again from the same start.
struct Snapshot {
    ram: Vec<u8>,
    spr: Vec<u8>,
    gpr: [u128; 32],
    fpr: [u32; 32],
    vf: [[f32; 4]; 32],
    vacc: [f32; 4],
    q: f32,
    acc: u32,
}

impl Snapshot {
    fn of(ee: &Ee) -> Snapshot {
        Snapshot {
            ram: ee.ram.clone(),
            spr: ee.spr.clone(),
            gpr: ee.gpr,
            fpr: ee.fpr,
            vf: ee.vf,
            vacc: ee.vacc,
            q: ee.q,
            acc: ee.acc,
        }
    }

    fn back(&self, ee: &mut Ee) {
        ee.ram.copy_from_slice(&self.ram);
        ee.spr.copy_from_slice(&self.spr);
        (ee.gpr, ee.fpr, ee.vf, ee.vacc, ee.q, ee.acc) = (self.gpr, self.fpr, self.vf, self.vacc, self.q, self.acc);
    }
}

/// One tick of the original's own update in the oracle, the bike's side laid in from the capture and
/// the generator from the row before, checked against every captured bone.
fn originals_tick(ee: &mut Ee, cap: &Capture, frame: u32) -> Vec<String> {
    let update = addr("rider_body_update").unwrap();
    lay_in(ee, cap, frame);
    if cap.has(frame - 1) {
        set_rng(ee, cap.int(frame - 1, "rng_state"));
    }
    ee.call(update, &[u64::from(ACTOR)], &[cap.get(frame, "dt")], 2_000_000_000);
    differ(cap, frame, &ee_bones(ee))
}

/// Exploration: the original's own update through the whole ride from the state, laid in tick by
/// tick; how far it holds every captured bone.
#[test]
#[ignore = "exploration"]
fn the_originals_update_replays_the_ride() {
    let Some(cap) = Capture::load("run1") else { return };
    let Some(mut ee) = state() else { return };
    let mut frame = START + 1;
    let mut parted = Vec::new();
    while cap.has(frame) {
        let d = originals_tick(&mut ee, &cap, frame);
        if !d.is_empty() {
            parted.push((frame, d.len(), d.into_iter().take(3).collect::<Vec<_>>()));
            if parted.len() >= 8 {
                break;
            }
        }
        frame += 1;
    }
    note(&format!("replay from {START}: {} ticks run, partings {parted:#?}", frame - START - 1));
}

/// Each bone's captured body put into the Ee's: the tick before's row as the next tick's start.
fn seed_bones(ee: &mut Ee, cap: &Capture, frame: u32) {
    for i in 0..18 {
        let body = bone_body(ee, i);
        for (name, o) in BODY {
            let v = cap.quad(frame, &format!("bone{i}.{name}"));
            for l in 0..4 {
                let at = ((body + o + 4 * l as u32) & 0x01ff_ffff) as usize;
                ee.ram[at..at + 4].copy_from_slice(&v[l].to_le_bytes());
            }
        }
    }
}

/// Exploration: the original's own update from the state, each tick's bones seeded from the row
/// before; how many ticks come out exact, and how far the rest are.
#[test]
#[ignore = "exploration"]
fn the_originals_tick_from_each_row() {
    let Some(cap) = Capture::load("run1") else { return };
    let Some(mut ee) = state() else { return };
    let (mut exact, mut ticks, mut worst) = (0, 0, Vec::new());
    for frame in START + 2.. {
        if !cap.has(frame) {
            break;
        }
        seed_bones(&mut ee, &cap, frame - 1);
        let d = originals_tick(&mut ee, &cap, frame);
        ticks += 1;
        exact += usize::from(d.is_empty());
        if !d.is_empty() && worst.len() < 30 {
            worst.push(format!("{frame}: {} lanes, {:?}", d.len(), &d[..d.len().min(2)]));
        }
    }
    note(&format!("seeded: {exact} of {ticks} ticks exact\n{}", worst.join("\n")));
}

// What follows is tests/ragdoll_wrecks.rs's listener and port tick (S61, S62), copied as it is: the
// actor's layout, reading the port's actor off the RAM, the original's world answers, and the
// comparison of every number the update writes.

use game_sim::biplane::{Body, World as Stepped};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::collide::{Bundle, Record};
use game_sim::math::Mat4;
use game_sim::phy::{Throw, Wrecks};
use game_sim::ragdoll::{self, Bone, Collision, Joint, Launch, Pin, Ragdoll, Records, Tables};
use game_sim::ragdoll_body::{Actor, Cut, Fade, Look, Outside, Request, Scenery, Skeleton};
use game_sim::contact::{Collider, Mover, Node, Other, Side, Vehicle, Weighed};
use game_sim::ragdoll_contact::{Against, Contact, Scene, Sphere, moving};
use game_sim::rider_anim::Mode;
use ps2_float::F;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// The actor's fields (research/oracle.txt, S50, S51 and S61).
mod at {
    pub const VTABLE: u32 = 0x08;
    pub const RIDING: u32 = 0x2a4;
    pub const REPOSE: u32 = 0x2a8;
    pub const PLACED: u32 = 0x2ac;
    pub const ACTIVE: u32 = 0x2b0;
    pub const STIFF: u32 = 0x2b4;
    pub const TORQUES: u32 = 0x2b8;
    pub const WORLD: u32 = 0x2cc;
    pub const GRAVITY_SCALE: u32 = 0x2d0;
    pub const SKELETON: u32 = 0x234;
    pub const PARTS: u32 = 0x2f4;
    pub const CUT: u32 = 0x420;
    pub const LOOK_ON: u32 = 0x44c;
    pub const LOOK_TARGET: u32 = 0x454;
    pub const LOOK_FAST: u32 = 0x464;
    pub const LOOK_SLOW: u32 = 0x46c;
    pub const LOOK_AT: u32 = 0x474;
    pub const MODE: u32 = 0x47c;
    pub const PREV: u32 = 0x480;
    pub const BIKE: u32 = 0x488;
    pub const TOUCHING: u32 = 0x48c;
    pub const ROLL: u32 = 0x490;
    pub const CUT_NOW: u32 = 0x494;
    pub const CUT_WAS: u32 = 0x498;
    pub const ENTERED: u32 = 0x49c;
    pub const PART_RECORDS: u32 = 0x4b8;
    pub const JOINT_RECORDS: u32 = 0x4c0;
    pub const HOLD_TARGETS: u32 = 0x4d0;
    pub const FOOT_POINTS: u32 = 0x510;
    pub const HOLD_VELOCITY: u32 = 0x530;
    pub const BOX_CENTRE: u32 = 0x540;
    pub const BOX_HALF: u32 = 0x550;
    pub const BIKE_FRAME: u32 = 0x580;
    pub const HOLDS: [u32; 4] = [0x5d0, 0x5d4, 0x5d8, 0x5dc];
    pub const BOX: u32 = 0x5f8;
    pub const COLLISIONS: u32 = 0x624;
    pub const FORCE: u32 = 0x62c;
    pub const FOOT_FADE: u32 = 0x638;
    pub const FADE_LENGTH: u32 = 0x644;
    pub const FADE_LEFT: u32 = 0x648;
    pub const POSE_SPRING: u32 = 0x650;
    pub const POSE_DAMPING: u32 = 0x660;
    pub const POSE_DAMPING_BASE: u32 = 0x670;
    pub const POSED: u32 = 0x680;
    pub const LATCHED: u32 = 0x684;
    pub const OTHER_FRAME: u32 = 0x6a8;
    pub const WORLD_FRAME: u32 = 0x6ac;
    pub const OFF_BIKE: u32 = 0x6b0;
    pub const SETTLED: u32 = 0x6b4;
    pub const FLUNG: u32 = 0x6b8;
    pub const FORCE_TIME: u32 = 0x6bc;
    pub const HAND_HOLD: u32 = 0x6c0;
    pub const LAUNCH: u32 = 0x6d0;
    pub const PULL: u32 = 0x6e0;
    pub const IMPACT: u32 = 0x6f0;
    pub const WRECKS: u32 = 0x700;
    pub const GROUND_DAMPING: u32 = 0x8c0;
    pub const FLAIL_HEIGHT: u32 = 0x8d0;
    pub const ROLL_TIME: u32 = 0x8d4;
    pub const ROLL_SPEED: u32 = 0x8d8;
    pub const CLIP: u32 = 0x8f4;
    pub const LOOK_POSE: u32 = 0x9f0;
    pub const GATE_STEP: u32 = 0xa0c;
    pub const STAND_SPEED: u32 = 0xa14;
    pub const SLOW_SPEED: u32 = 0xa20;
    pub const GATE_CLOCK: u32 = 0xa28;
    pub const IDLE: u32 = 0xa2c;
}

/// The riding request's fields the update reads and the three its tail writes.
mod req_at {
    pub const LIVE: u32 = 0x08;
    pub const HELD: u32 = 0x0c;
    pub const ROWS: u32 = 0x10;
    pub const FRAME: u32 = 0x50;
    pub const VELOCITY: u32 = 0x90;
    pub const RATES: u32 = 0xa0;
    pub const HANDS: u32 = 0xb0;
    pub const FEET: u32 = 0xd0;
    pub const BOX: [u32; 2] = [0xf0, 0xf4];
    pub const LOOKING: u32 = 0xf8;
    pub const POSED: u32 = 0xfc;
    pub const REASON: u32 = 0x104;
    pub const IMPACT: u32 = 0x130;
    pub const CUT: u32 = 0x140;
    pub const HIT_VELOCITY: u32 = 0x150;
    pub const TUCKED: u32 = 0x16c;
    pub const PITCH: u32 = 0x170;
    pub const SPEED: u32 = 0x178;
    pub const CENTRE: u32 = 0x180;
    pub const QUAT: u32 = 0x190;
    pub const BIKE: u32 = 0x1ac;
    pub const TOUCHING: u32 = 0x1bc;
    pub const MAIN: u32 = 0x1c0;
    pub const EFFECT: u32 = 0x238;
    pub const FADE: u32 = 0x23c;
    pub const FADE_FROM: u32 = 0x240;
}

/// Offsets inside a pin, a bone collision and a bone's body.
mod pin_at {
    pub const ON: u32 = 0x04;
    pub const ANCHORS: [u32; 2] = [0x20, 0x30];
    pub const TARGET: u32 = 0x40;
    pub const TARGET_VELOCITY: u32 = 0x50;
    pub const FIRST: u32 = 0x68;
    pub const SECOND: u32 = 0x6c;
    pub const LOWER: u32 = 0x80;
    pub const UPPER: u32 = 0x90;
    pub const HIT: u32 = 0xa0;
    pub const STICKY: u32 = 0xb0;
    pub const GAINS: u32 = 0x100;
}

mod coll_at {
    pub const ON: u32 = 0x04;
    pub const TOUCHED: u32 = 0x14;
    pub const GROUND: u32 = 0x18;
    pub const SCENE: u32 = 0x1c;
    pub const SWEPT: u32 = 0x20;
    pub const SWEEP: u32 = 0x24;
    pub const SWEEP_BACK: u32 = 0x28;
    pub const UNPENETRATION: u32 = 0x2c;
    pub const ROTATION: u32 = 0x30;
    pub const RESTITUTION: u32 = 0x34;
    pub const FRICTION: u32 = 0x38;
    pub const POINT: u32 = 0x40;
    pub const NORMAL: u32 = 0x50;
    pub const DV: u32 = 0x60;
    pub const TERRAIN: u32 = 0x78;
    pub const BODY: u32 = 0x7c;
    pub const SPHERE: u32 = 0x88;
    pub const SEGMENT: u32 = 0x8c;
    /// The terrain's box.
    pub const BOX_MIN: u32 = 0x30;
    pub const BOX_MAX: u32 = 0x40;
    /// A collider's hit flag (bit 5 of the byte) and its record.
    pub const FLAGS: u32 = 0x116;
    pub const RECORD: u32 = 0x6c;
}

mod body_at {
    pub const MASS: u32 = 0x0c;
    pub const INV_MASS: u32 = 0x10;
    pub const RADIUS: u32 = 0x14;
    pub const DAMPING: u32 = 0x20;
    pub const LAST_POSITION: u32 = 0x50;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const STEP: u32 = 0xf0;
    pub const INERTIA: u32 = 0x110;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const LAST_QUAT: u32 = 0x150;
    pub const REFERENCE: u32 = 0x160;
    pub const QUAT: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const RATES: u32 = 0x1b0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
}

fn f(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

fn bits(v: Vf) -> [u32; 4] {
    [v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), v[3].to_bits()]
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

fn bones_of(ee: &Ee) -> Vec<Bone> {
    (0..ragdoll::PARTS).map(|i| bone_at(ee, bone_body(ee, i))).collect()
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

/// The actor's `[Wrecks]`: the seven throws and the settle test's four numbers.
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
        ground_angular_rate_damping: quad(ee, ACTOR + at::GROUND_DAMPING),
        rider_flail_height: f(ee, ACTOR + at::FLAIL_HEIGHT),
        rider_roll_time: f(ee, ACTOR + at::ROLL_TIME),
        rider_roll_speed: f(ee, ACTOR + at::ROLL_SPEED),
        ..Wrecks::default()
    }
}

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
    let collisions: Vec<usize> = (0..9).map(|i| bone_of(ee, ee.u32_at(collision(ee, i) + coll_at::BODY))).collect();
    let mut r = Ragdoll::new(Records { parts, joints, collisions });
    r.bones = bones_of(ee);
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
        *c = Collision { bone: c.bone, enabled: ee.u32_at(k + coll_at::ON) != 0, touching: ee.u32_at(k + coll_at::TOUCHED) != 0 };
    }
    let w = ee.u32_at(ACTOR + at::WORLD);
    r.world = Stepped { iterations: ee.i32_at(w), rate: f(ee, w + 4) };
    r.holds = at::HOLDS.map(|h| ee.u32_at(ee.u32_at(ACTOR + h) + pin_at::ON) != 0);
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

fn contact_of(ee: &Ee, c: u32) -> Contact {
    use coll_at::*;
    Contact {
        touched: ee.u32_at(c + TOUCHED) != 0,
        ground: ee.u32_at(c + GROUND) != 0,
        scene: ee.u32_at(c + SCENE) != 0,
        swept: ee.u32_at(c + SWEPT) != 0,
        point: quad(ee, c + POINT),
        normal: quad(ee, c + NORMAL),
        dv: quad(ee, c + DV),
    }
}

fn sphere_of(ee: &Ee, c: u32, bone: usize) -> Sphere {
    use coll_at::*;
    Sphere {
        bone,
        radius: f(ee, ee.u32_at(c + BODY) + body_at::RADIUS),
        unpenetration: f(ee, c + UNPENETRATION),
        rotation: f(ee, c + ROTATION),
        restitution: f(ee, c + RESTITUTION),
        friction: f(ee, c + FRICTION),
        sweep: ee.u32_at(c + SWEEP) != 0,
        sweep_back: f(ee, c + SWEEP_BACK),
    }
}

fn mode_of(m: i32) -> Mode {
    match m {
        0 => Mode::Held,
        1 => Mode::Riding,
        2 => Mode::Wrecked,
        3 => Mode::Posed,
        4 => Mode::Gate,
        _ => panic!("mode {m}"),
    }
}

fn mat_at(ee: &Ee, a: u32) -> Mat4 {
    Mat4([0, 0x10, 0x20, 0x30].map(|k| quad(ee, a + k)))
}

/// The actor as the port's, off the RAM.
fn actor_of(ee: &Ee) -> Actor {
    let a = |o: u32| ACTOR + o;
    let pair = |o: u32| [f(ee, a(o)), f(ee, a(o + 4))];
    let mut actor = Actor::new(ragdoll_of(ee), 9);
    actor.contacts = (0..9).map(|i| contact_of(ee, collision(ee, i))).collect();
    actor.mode = mode_of(ee.i32_at(a(at::MODE)));
    actor.prev = mode_of(ee.i32_at(a(at::PREV)));
    actor.posed = ee.u32_at(a(at::POSED)) != 0;
    actor.latched = ee.u32_at(a(at::LATCHED)) != 0;
    actor.idle = ee.u32_at(a(at::IDLE)) != 0;
    actor.look_pose = ee.i32_at(a(at::LOOK_POSE));
    actor.clip = ee.i32_at(a(at::CLIP));
    actor.touching = ee.u32_at(a(at::TOUCHING)) != 0;
    actor.fade = Fade { length: f(ee, a(at::FADE_LENGTH)), left: f(ee, a(at::FADE_LEFT)) };
    actor.foot_fade = pair(at::FOOT_FADE);
    actor.look = Look {
        on: [0, 4].map(|k| ee.u32_at(a(at::LOOK_ON + k)) != 0),
        fast: pair(at::LOOK_FAST),
        slow: pair(at::LOOK_SLOW),
        target: pair(at::LOOK_TARGET),
        at: pair(at::LOOK_AT),
    };
    actor.pose_spring = quad(ee, a(at::POSE_SPRING));
    actor.pose_damping = quad(ee, a(at::POSE_DAMPING));
    actor.pose_damping_base = quad(ee, a(at::POSE_DAMPING_BASE));
    actor.stand_speed = f(ee, a(at::STAND_SPEED));
    actor.slow_speed = f(ee, a(at::SLOW_SPEED));
    actor.gate_step = ee.i32_at(a(at::GATE_STEP));
    actor.gate_clock = f(ee, a(at::GATE_CLOCK));
    actor.foot_points = [0, 0x10].map(|k| quad(ee, a(at::FOOT_POINTS + k)));
    actor.hold_targets = std::array::from_fn(|k| quad(ee, a(at::HOLD_TARGETS + 0x10 * k as u32)));
    actor.hold_velocity = quad(ee, a(at::HOLD_VELOCITY));
    actor.bike_box = (quad(ee, a(at::BOX_CENTRE)), quad(ee, a(at::BOX_HALF)));
    actor.bike_frame = mat_at(ee, a(at::BIKE_FRAME));
    actor.placed = ee.u32_at(a(at::PLACED)) != 0;
    actor.active = ee.u32_at(a(at::ACTIVE)) != 0;
    actor.repose = ee.u32_at(a(at::REPOSE)) != 0;
    actor.torques = ee.u32_at(a(at::TORQUES)) != 0;
    actor.gravity_scale = f(ee, a(at::GRAVITY_SCALE));
    let cut = ee.u32_at(a(at::CUT));
    actor.cut = (ee.u32_at(cut + 0x34) == 2)
        .then(|| Cut { now: ee.u32_at(a(at::CUT_NOW)) != 0, was: ee.u32_at(a(at::CUT_WAS)) != 0 });
    actor.impact = quad(ee, a(at::IMPACT));
    actor
}

/// The riding request as the port's, off the RAM.
fn request_of(ee: &Ee) -> Request {
    use req_at::*;
    let r = ee.u32_at(ACTOR + at::BIKE);
    let on = |o: u32| ee.u32_at(r + o) != 0;
    let q = |o: u32| quad(ee, r + o);
    let (bx, half) = (ee.u32_at(r + BOX[0]), ee.u32_at(r + BOX[1]));
    let bike = ee.u32_at(r + BIKE);
    Request {
        live: on(LIVE),
        held: on(HELD),
        rows: [0, 0x10, 0x20].map(|k| q(ROWS + k)),
        frame: mat_at(ee, r + FRAME),
        velocity: q(VELOCITY),
        rates: q(RATES),
        hands: [q(HANDS), q(HANDS + 0x10)],
        feet: [q(FEET), q(FEET + 0x10)],
        bike_box: (bx != 0 && half != 0).then(|| (quad(ee, bx), quad(ee, half))),
        looking: on(LOOKING),
        posed: on(POSED),
        reason: ee.i32_at(r + REASON),
        cut: on(CUT),
        hit_velocity: q(HIT_VELOCITY),
        tucked: on(TUCKED),
        pitch: f(ee, r + PITCH),
        speed: f(ee, r + SPEED),
        centre: q(CENTRE),
        quat: q(QUAT),
        bike: (bike != 0).then(|| (quad(ee, bike + 0x40), f(ee, bike + 0x60))),
        effect: ee.i32_at(r + EFFECT),
        fade: f(ee, r + FADE),
        fade_from: f(ee, r + FADE_FROM),
    }
}

fn rest_at(ee: &Ee, bone: usize) -> u32 {
    let idx = ee.u32_at(bone_body(ee, bone) + 8);
    let part = ee.u32_at(ACTOR + at::PARTS) + 0xb0 * idx;
    ee.u32_at(ACTOR + at::SKELETON) + 0x60 * ee.u32_at(part + 0x40) + 0x50
}

fn tables_of(ee: &Ee) -> Tables {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let curve: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    Tables { curve: CurveTable::from_entries(&curve), ..Tables::build() }
}

/// Run the hooked function itself from inside its hook, on a stack below the caller's, and come
/// back to the hook's caller as if nothing had been hooked.
fn through(ee: &mut Ee) {
    let (pc, ra, sp, steps, stack) = (ee.pc, ee.r(31), ee.r(29), ee.steps, ee.stack);
    let args: Vec<u64> = (4..12).map(|k| ee.r(k)).collect();
    ee.stack = (sp as u32 - 0x800) & !0xf;
    ee.call(pc, &args, &[], 2_000_000_000);
    ee.set(31, ra);
    ee.set(29, sp);
    (ee.steps, ee.stack, ee.pc) = (steps, stack, pc);
}

/// What the world answered the original in one tick, in the order it asked.
#[derive(Default)]
struct Heard {
    /// The ground queries: the point asked about, and the point and normal answered.
    ground: VecDeque<([u32; 4], Vf, Vf)>,
    /// Each bone collision's sphere against the scene: its centre, the hits, and what each hit is
    /// with when that is another collidable (the record names it).
    spheres: VecDeque<([u32; 4], Option<Vec<(Vf, Vf)>>, Vec<Option<Moving>>)>,
    /// The others of the last sphere test the port made.
    last: Vec<Option<Moving>>,
    /// Each sweep: the segment, and the hit.
    segments: VecDeque<(([u32; 4], [u32; 4]), Option<(Vf, Vf)>)>,
    centre: [u32; 4],
    segment: ([u32; 4], [u32; 4]),
    /// The bones as each pose-and-reset left them.
    poses: VecDeque<Vec<Bone>>,
    /// The riding pose's point for each bone, as the skeleton handed it, and the skeleton's rests
    /// as the step read them (the update moves the skeleton before the step).
    points: [Option<Vf>; ragdoll::PARTS],
    rests: Option<[Vf; ragdoll::PARTS]>,
    /// What the port asked that the original did not.
    odd: Vec<String>,
    /// The sphere hits against another collider (the record names it).
    moving: usize,
}

/// The other side of a sphere's hit, read off the original's RAM as the hit was made: what the
/// push-out measures the bone's velocity against (S62, research/oracle.txt).
#[derive(Clone)]
enum Moving {
    Still,
    Mover(Mover),
    Body(Collider, Body),
    Vehicle(Collider, Vehicle),
}

impl Moving {
    /// The collidable at `c`: its collision object (`+0x80`) on a dynamics body (`+0xf4`) or on a
    /// vehicle's chassis (`+0xf0`, its transform `+0xf8`), else its owner (`+0x7c`) when that is a
    /// mover (class `+0x1b0` is 2).
    fn of(ee: &mut Ee, c: u32) -> Moving {
        let (owner, object) = (ee.u32_at(c + 0x7c), ee.u32_at(c + 0x80));
        let node_at = |ee: &Ee, n: u32| {
            assert_ne!(ee.u32_at(n + 0x110), 0, "a transform not up to date at {n:#x}");
            Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 0x10 * k)), origin: quad(ee, n + 0xb0) }
        };
        if object != 0 {
            let collider = Collider { cofg: quad(ee, object + 0x90), cofg_world: quad(ee, object + 0xa0), ..Collider::default() };
            let body = ee.u32_at(object + 0xf4);
            if body != 0 {
                use body_at::*;
                let b = Body {
                    position: quad(ee, body + POSITION),
                    velocity: quad(ee, body + VELOCITY),
                    quat: quad(ee, body + QUAT),
                    rates: quad(ee, body + RATES),
                    ..Body::default()
                };
                return Moving::Body(collider, b);
            }
            let v = ee.u32_at(object + 0xf0);
            let vehicle = Vehicle {
                node: node_at(ee, ee.u32_at(object + 0xf8)),
                cofg: quad(ee, v + 0x190),
                velocity: quad(ee, v + 0x260),
                rates: quad(ee, v + 0x290),
                ..Vehicle::default()
            };
            return Moving::Vehicle(collider, vehicle);
        }
        if owner == 0 || ee.u32_at(owner + 0x1b0) != 2 {
            return Moving::Still;
        }
        // The mover's spin and velocity are its two virtuals' answers.
        let vtable = ee.u32_at(owner + 8);
        let out = ee.scratch(0);
        let ask = |ee: &mut Ee, slot: u32| {
            let (pc, ra, sp, steps, stack) = (ee.pc, ee.r(31), ee.r(29), ee.steps, ee.stack);
            ee.stack = (sp as u32 - 0x800) & !0xf;
            ee.call(ee.u32_at(vtable + slot), &[u64::from(out), u64::from(owner)], &[], 2_000_000_000);
            ee.set(31, ra);
            ee.set(29, sp);
            (ee.steps, ee.stack, ee.pc) = (steps, stack, pc);
            quad(ee, out)
        };
        let spin = ask(ee, 0xbc);
        let velocity = ask(ee, 0xb8);
        Moving::Mover(Mover { spin, velocity, node: node_at(ee, ee.u32_at(owner + 0x254)), origin: quad(ee, owner + 0x190) })
    }

    /// Its velocity at `at`, as the port's push-out has it.
    fn at(&self, at: Vf) -> Vf {
        match self.clone() {
            Moving::Still => moving(at, Against::Still),
            Moving::Mover(m) => moving(at, Against::Mover(&m)),
            Moving::Body(mut collider, mut body) => {
                let mut w = Weighed { body: &mut body, mass: F::ONE };
                moving(at, Against::Other(&mut Other { collider: &mut collider, side: Side::Body(&mut w) }))
            }
            Moving::Vehicle(mut collider, mut vehicle) => {
                moving(at, Against::Other(&mut Other { collider: &mut collider, side: Side::Vehicle(&mut vehicle) }))
            }
        }
    }
}

/// Every call of the update's that reaches the world, hooked to run as it is and to write down
/// what it answered.
fn listen(ee: &mut Ee) -> Rc<RefCell<Heard>> {
    let heard = Rc::new(RefCell::new(Heard::default()));
    let spheres: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(ee, i) + coll_at::SPHERE)).collect();
    let segments: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(ee, i) + coll_at::SEGMENT)).collect();
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("ground_query").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (point, normal) = (ee.r(5) as u32, ee.r(6) as u32);
            let asked = bits(quad(ee, point));
            through(ee);
            let n = if normal == 0 { Vf::default() } else { quad(ee, normal) };
            h.borrow_mut().ground.push_back((asked, quad(ee, point), n));
        }),
    );
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("collider_set_position").unwrap(),
        Box::new(move |ee: &mut Ee| {
            h.borrow_mut().centre = bits(quad(ee, ee.r(5) as u32));
            through(ee);
        }),
    );
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("probe_bundle_set_points").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let p = ee.r(5) as u32;
            if p != 0 {
                h.borrow_mut().segment = (bits(quad(ee, p)), bits(quad(ee, p + 0x10)));
            }
            through(ee);
        }),
    );
    let collide = |h: Rc<RefCell<Heard>>, spheres: Vec<u32>, segments: Vec<u32>| {
        Box::new(move |ee: &mut Ee| {
            let c = ee.r(4) as u32;
            through(ee);
            let hit = ee.u8_at(c + coll_at::FLAGS) & 0x20 != 0;
            let rec = ee.u32_at(c + coll_at::RECORD);
            let mut h = h.borrow_mut();
            if segments.contains(&c) {
                let answer = hit.then(|| (quad(ee, rec + 0x30), quad(ee, rec + 0x20)));
                let s = h.segment;
                h.segments.push_back((s, answer));
            } else if spheres.contains(&c) {
                let n = if hit { ee.u32_at(rec + 4) } else { 0 };
                let answer = hit.then(|| (0..n).map(|i| (quad(ee, rec + 0x60 + 0x10 * i), quad(ee, rec + 0x30 + 0x10 * i))).collect());
                let others: Vec<u32> = (0..n).map(|i| ee.u32_at(rec + 0x90 + 4 * i)).collect();
                h.moving += others.iter().filter(|&&o| o != 0).count();
                let others = others.into_iter().map(|o| (o != 0).then(|| Moving::of(ee, o))).collect();
                let c = h.centre;
                h.spheres.push_back((c, answer, others));
            }
        }) as Box<dyn FnMut(&mut Ee)>
    };
    ee.hooks.insert(addr("collide_against_search").unwrap(), collide(Rc::clone(&heard), spheres.clone(), segments.clone()));
    ee.hooks.insert(addr("probe_collide_world").unwrap(), collide(Rc::clone(&heard), spheres, segments));
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("orientation_transform_point").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (out, body) = (ee.r(4) as u32, (ee.r(6) as u32).wrapping_sub(body_at::COFG));
            through(ee);
            let i = bone_of(ee, body);
            if i < ragdoll::PARTS {
                h.borrow_mut().points[i] = Some(quad(ee, out));
            }
        }),
    );
    // The normalisation right after the step: the skeleton's rests as the step read them.
    let normalise = ee.u32_at(ee.u32_at(ACTOR + at::VTABLE) + 0xac);
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        normalise,
        Box::new(move |ee: &mut Ee| {
            h.borrow_mut().rests = Some(std::array::from_fn(|i| quad(ee, rest_at(ee, i))));
            through(ee);
        }),
    );
    let reset = ee.u32_at(ee.u32_at(ACTOR + at::VTABLE) + 0xa0);
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        reset,
        Box::new(move |ee: &mut Ee| {
            through(ee);
            h.borrow_mut().poses.push_back(bones_of(ee));
        }),
    );
    heard
}

/// The world, the scene and the skeleton as the original heard them, for the port.
struct Replay<'a>(&'a RefCell<Heard>);

impl game_sim::world::World for Replay<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let mut h = self.0.borrow_mut();
        match h.ground.pop_front() {
            Some((asked, p, n)) => {
                if asked != bits(at) {
                    h.odd.push(format!("the ground at {at:?}, the original's at {asked:x?}"));
                }
                (p, n)
            }
            None => {
                h.odd.push(format!("the ground at {at:?}, which the original did not ask"));
                (at, Vf::new(0.0, 1.0, 0.0, 0.0))
            }
        }
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

impl Scene for Replay<'_> {
    fn touching(&mut self, centre: Vf, _radius: F) -> Option<Vec<(Vf, Vf)>> {
        let mut h = self.0.borrow_mut();
        match h.spheres.pop_front() {
            Some((asked, hits, others)) => {
                if asked != bits(centre) {
                    h.odd.push(format!("a sphere at {centre:?}, the original's at {asked:x?}"));
                }
                h.last = others;
                hits
            }
            None => {
                h.odd.push(format!("a sphere at {centre:?}, which the original did not test"));
                h.last.clear();
                None
            }
        }
    }
    fn other(&mut self, i: usize) -> Option<usize> {
        self.0.borrow().last.get(i).and_then(Option::as_ref).map(|_| i)
    }
    fn velocity(&mut self, other: usize, at: Vf) -> Vf {
        let h = self.0.borrow();
        h.last[other].as_ref().expect("an other the port named").at(at)
    }
    fn sweep(&mut self, from: Vf, to: Vf) -> Option<(Vf, Vf)> {
        let mut h = self.0.borrow_mut();
        match h.segments.pop_front() {
            Some((asked, hit)) => {
                if asked != (bits(from), bits(to)) {
                    h.odd.push(format!("a sweep {from:?} to {to:?}, the original's {asked:x?}"));
                }
                hit
            }
            None => {
                h.odd.push(format!("a sweep {from:?} to {to:?}, which the original did not make"));
                None
            }
        }
    }
}

struct Posed<'a>(&'a RefCell<Heard>);

impl Outside for Posed<'_> {
    fn pose(&mut self, doll: &mut Ragdoll, _dt: F, _carried: bool) {
        let mut h = self.0.borrow_mut();
        match h.poses.pop_front() {
            Some(bones) => doll.bones = bones,
            None => h.odd.push("a pose the original did not make".into()),
        }
    }
    fn wreck_sound(&mut self, _reason: i32, rng: &mut Rng) {
        rng.rand();
    }
}

/// Every number of the actor the update writes, by name, as bits.
fn written(a: &Actor) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut v = |name: String, x: Vf| out.extend((0..4).map(|k| (format!("{name}[{k}]"), x[k].to_bits())));
    let r = &a.ragdoll;
    for (i, b) in r.bones.iter().enumerate() {
        let d = &b.body;
        // The linear acceleration's `w` is a register the integrator stores and nothing loads, and so
        // is the angular one's on a pinned body (as in tests/ragdoll.rs): neither is compared.
        let stale = |x: Vf, is: bool| if is { Vf::new(x[0], x[1], x[2], 0.0) } else { x };
        for (n, x) in [
            ("position", d.position),
            ("velocity", d.velocity),
            ("quat", d.quat),
            ("rates", d.rates),
            ("last_position", d.last_position),
            ("last_quat", d.last_quat),
            ("damping", d.damping),
            ("ang_damping", d.ang_damping),
            ("inv_inertia", d.inv_inertia),
            ("force", d.force),
            ("last_force", d.last_force),
            ("accel", stale(d.accel, true)),
            ("shock_peak", d.shock_peak),
            ("shock", d.shock),
            ("step", d.step),
            ("moment", d.moment),
            ("last_moment", d.last_moment),
            ("ang_accel", stale(d.ang_accel, d.inv_mass == F::ZERO)),
            ("twist_peak", d.twist_peak),
            ("twist", d.twist),
            ("cofg", d.cofg),
            ("inertia", b.inertia),
            ("reference", b.reference),
            ("mass/inv_mass/enabled", Vf::new(b.mass.0, d.inv_mass.0, f32::from(u8::from(d.enabled)), 0.0)),
        ] {
            v(format!("bone{i}.{n}"), x);
        }
    }
    for (i, p) in r.pins.iter().enumerate() {
        for (n, x) in [("target", p.target), ("target_velocity", p.target_velocity), ("hit", p.hit), ("sticky", p.sticky), ("gains", p.gains)] {
            v(format!("pin{i}.{n}"), x);
        }
    }
    for (i, j) in r.joints.iter().enumerate() {
        for (n, x) in [("spring", j.spring), ("damping", j.damping), ("critical", j.critical)] {
            v(format!("joint{i}.{n}"), x);
        }
        v(format!("joint{i}.wrecked/max"), Vf::new(f32::from(u8::from(j.wrecked)), j.max_angle.0, 0.0, 0.0));
    }
    for (i, c) in a.contacts.iter().enumerate() {
        v(format!("contact{i}.point"), c.point);
        v(format!("contact{i}.normal"), c.normal);
        v(format!("contact{i}.dv"), c.dv);
        let flags = [c.touched, c.ground, c.scene, c.swept, r.collisions[i].enabled].map(|b| f32::from(u8::from(b)));
        v(format!("contact{i}.touched/ground/scene/swept"), Vf(flags[..4].try_into().unwrap()));
        v(format!("collision{i}.on"), Vf::new(flags[4], 0.0, 0.0, 0.0));
    }
    let b = |x: bool| f32::from(u8::from(x));
    v("holds".into(), Vf(r.holds.map(b)));
    v("throwing/entered/stiff/riding".into(), Vf::new(b(r.throwing), b(r.entered), b(r.stiff), b(r.riding)));
    v("off_bike/flung/settled/roll".into(), Vf::new(b(r.off_bike), b(r.flung), b(r.settled), r.roll.0));
    v("launch.force".into(), r.launch.force);
    v(
        "launch.force_time/hand_hold/target/frames".into(),
        Vf::new(r.launch.force_time.0, r.launch.hand_hold.0, r.launch.target as f32, b(r.launch.world_frame) + 2.0 * b(r.launch.other_frame)),
    );
    v("pull".into(), r.pull);
    v("mode/prev/touching/placed".into(), Vf::new(a.mode as i32 as f32, a.prev as i32 as f32, b(a.touching), b(a.placed)));
    v("repose/fade".into(), Vf::new(b(a.repose), a.fade.length.0, a.fade.left.0, 0.0));
    for (k, t) in a.hold_targets.iter().enumerate() {
        v(format!("hold_target{k}"), *t);
    }
    v("hold_velocity".into(), a.hold_velocity);
    v("box_centre".into(), a.bike_box.0);
    v("box_half".into(), a.bike_box.1);
    for (k, row) in a.bike_frame.0.iter().enumerate() {
        v(format!("bike_frame{k}"), *row);
    }
    v("pose_damping".into(), a.pose_damping);
    v("impact".into(), a.impact);
    out
}

/// The tick the S56 replay ran, rerun with the world's calls listened to; then the port's tick off
/// the RAM the original started from, with the original's answers. Every number the update writes
/// is compared, and what the tail hands the request. A sphere's hit against another collidable is
/// answered with that collidable as the original's RAM had it (S62), and the port's push-out measures
/// the bone against its velocity.
///
/// With `carried`, the port's actor is its own from the tick before (the first tick takes the
/// original's): the port runs the wreck on its own state, and only the world's answers are the
/// original's. The second number is how many of the original's sphere hits were with another
/// collidable.
fn port_tick(ee: &mut Ee, cap: &Capture, frame: u32, draws_before: u32, drew: Option<u32>, carried: Option<&mut Option<Actor>>) -> (Vec<String>, usize) {
    let update = addr("rider_body_update").unwrap();
    let dt = cap.get(frame, "dt");
    let mut r = Rng::from_state(cap.int(frame - 1, "rng_state"));
    (0..draws_before).for_each(|_| {
        r.rand();
    });
    set_rng(ee, r.state());
    let actor = actor_of(ee);
    let request = request_of(ee);
    let wrecks = wrecks_of(ee);
    let tables = tables_of(ee);
    let rests: [Vf; ragdoll::PARTS] = std::array::from_fn(|i| quad(ee, rest_at(ee, i)));
    let spheres: Vec<Sphere> = (0..9).map(|i| sphere_of(ee, collision(ee, i), actor.ragdoll.records.collisions[i])).collect();
    let terrains: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(ee, i) + coll_at::TERRAIN)).collect();
    assert!(terrains.iter().all(|&t| t == terrains[0] && t != 0), "{frame}: the collisions' terrains {terrains:x?}");
    let terrain = (quad(ee, terrains[0] + coll_at::BOX_MIN), quad(ee, terrains[0] + coll_at::BOX_MAX));

    let saved = std::mem::take(&mut ee.hooks);
    let heard = listen(ee);
    ee.call(update, &[u64::from(ACTOR)], &[dt], 2_000_000_000);
    ee.hooks = saved;
    let mut out = Vec::new();
    let d = if drew.is_some() { differ(cap, frame, &ee_bones(ee)) } else { Vec::new() };
    if !d.is_empty() {
        out.push(format!("the listened tick is not the replayed one: {:?}", &d[..d.len().min(4)]));
    }
    let drew = drew.unwrap_or_else(|| draws(r.state(), rng_at(ee), 1000).expect("the body's draws"));
    let after = Snapshot::of(ee);
    let game = actor_of(ee);
    let req = ee.u32_at(ACTOR + at::BIKE);
    let game_tail = (ee.u32_at(req + req_at::TOUCHING) != 0, bits(quad(ee, req + req_at::IMPACT)), bits(quad(ee, req + req_at::MAIN)));

    let mut carried = carried;
    let mut port = carried.as_mut().and_then(|c| c.take()).unwrap_or(actor);
    let (points, at_step) = (heard.borrow().points, heard.borrow().rests);
    let skeleton = Skeleton { points: points.map(|p| p.unwrap_or_default()), rests: at_step.unwrap_or(rests), main_rest: rests[0] };
    let mut rng = Rng::from_state(r.state());
    let tail = {
        let mut world = Replay(&heard);
        let mut scene = Replay(&heard);
        let scenery = Scenery { spheres: &spheres, world: &mut world, terrain, scene: &mut scene };
        port.tick(F(dt), &request, &skeleton, &tables, &wrecks, scenery, &mut rng, &mut Posed(&heard))
    };
    let h = heard.borrow();
    for (what, left) in [("ground queries", h.ground.len()), ("sphere tests", h.spheres.len()), ("sweeps", h.segments.len()), ("poses", h.poses.len())] {
        if left > 0 {
            out.push(format!("{left} of the original's {what} the port did not make"));
        }
    }
    let port_drew = draws(r.state(), rng.state(), 1000).unwrap_or(u32::MAX);
    if !(port_drew == drew || (port_drew == 0 && drew != 6)) {
        out.push(format!("the port drew {port_drew}, the original {drew}"));
    }
    let (want, got) = (written(&game), written(&port));
    for ((name, w), (_, g)) in want.iter().zip(&got) {
        if w != g {
            out.push(format!("{name}: {:e} want {:e}", f32::from_bits(*g), f32::from_bits(*w)));
        }
    }
    let port_tail = (tail.touching, bits(tail.impact), bits(tail.main));
    if port_tail != game_tail {
        out.push(format!("the tail {port_tail:x?} want {game_tail:x?}"));
    }
    out.extend(h.odd.iter().cloned());
    let moving = h.moving;
    drop(h);
    if let Some(c) = carried {
        *c = Some(port);
    }
    // The original's state goes on to the next tick, whatever the port made of this one.
    after.back(ee);
    (out, moving)
}


/// Exploration: what the riding request and the actor hold in the state, the request's points taken
/// into the bike's drawn frame, for the port's own riding body.
#[test]
#[ignore = "exploration"]
fn what_the_riding_request_holds() {
    let Some(ee) = state() else { return };
    let r = request_of(&ee);
    let a = actor_of(&ee);
    let m = r.frame;
    let back = |p: Vf| {
        let d = p.sub(m.0[3]);
        let dot = |k: usize| m.0[k].x() * d.x() + m.0[k].y() * d.y() + m.0[k].z() * d.z();
        [dot(0), dot(1), dot(2)]
    };
    let mut out = format!("request {r:#?}\nframe {m:?}\n");
    out += &format!("hands in the frame {:?} {:?}\nfeet in the frame {:?} {:?}\n", back(r.hands[0]), back(r.hands[1]), back(r.feet[0]), back(r.feet[1]));
    out += &format!("centre in the frame {:?}\n", back(r.centre));
    out += &format!(
        "actor: mode {:?} prev {:?} spring {:?} damping {:?} base {:?} stand {:?} slow {:?} foot_points {:?} hold_targets {:?} box {:?} gravity {:?} torques {} look {:?} clip {} look_pose {}\n",
        a.mode, a.prev, a.pose_spring, a.pose_damping, a.pose_damping_base, a.stand_speed, a.slow_speed, a.foot_points, a.hold_targets, a.bike_box, a.gravity_scale, a.torques, a.look, a.clip, a.look_pose
    );
    let d = &a.ragdoll;
    out += &format!("ragdoll: riding {} entered {} stiff {} holds {:?} bike_box {} ignores_bike {} world {:?}\n", d.riding, d.entered, d.stiff, d.holds, d.bike_box, d.ignores_bike, d.world);
    for (i, b) in d.bones.iter().enumerate() {
        out += &format!("bone {i}: mass {:?} damping {:?} ang {:?} cofg {:?} enabled {}\n", b.mass, b.body.damping, b.body.ang_damping, b.body.cofg, b.body.enabled);
    }
    for (i, p) in d.pins.iter().enumerate().take(4) {
        out += &format!("hold {i}: first {} anchors {:?} gains {:?}\n", p.first, p.anchors, p.gains);
    }
    note(&out);
}

/// The ride kit off the disc, as the shell builds it: the rider file, and the rig off the rider's
/// model and riding clip.
fn ride_kit() -> Option<game_sim::ragdoll_ride::Kit> {
    use game_render::disc::Disc;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let rider_name = names.iter().filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len())?.clone();
    let res = disc.res(&rider_name).expect("the rider archive opens");
    let rider_file = res.read_suffix("anim\\mrider.phy").expect("the rider's archive carries mrider.phy");
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
    Some(game_sim::ragdoll_ride::Kit::new(&rider_file, doll.rig))
}

/// The riding body the shell runs has the numbers the original's rider body was built with: the
/// pose spring and damper off `[Joint Forces]`, the two speeds, the feet's offsets and the bike box.
#[test]
fn the_riding_bodys_numbers_are_the_originals() {
    let Some(ee) = state() else { return };
    let Some(kit) = ride_kit() else { return };
    let game = actor_of(&ee);
    let port = game_sim::ragdoll_riding::Riding::new(&kit).actor;
    let b = |v: Vf| bits(v);
    assert_eq!(b(port.pose_spring), b(game.pose_spring), "the pose spring");
    assert_eq!(b(port.pose_damping_base), b(game.pose_damping_base), "the pose damper");
    assert_eq!(port.stand_speed.0.to_bits(), game.stand_speed.0.to_bits(), "the stand speed");
    assert_eq!(port.slow_speed.0.to_bits(), game.slow_speed.0.to_bits(), "the slow speed");
    assert_eq!(port.foot_points.map(b), game.foot_points.map(b), "the feet's offsets");
    assert_eq!((b(port.bike_box.0), b(port.bike_box.1)), (b(game.bike_box.0), b(game.bike_box.1)), "the bike box");
    assert_eq!((port.active, port.torques, port.gravity_scale.0), (game.active, game.torques, game.gravity_scale.0));
    assert_eq!(port.ragdoll.bones.len(), game.ragdoll.bones.len());
}

/// **The riding tick, bit for bit**: from each of run1's rows (the bones laid in from the row
/// before, the original's skeleton points, rests and scene answers heard off its own tick), the
/// port's actor update writes every number the original's does.
#[test]
fn the_riding_tick_is_the_originals_from_each_row() {
    let Some(cap) = Capture::load("run1") else { return };
    let Some(mut ee) = state() else { return };
    let (mut exact, mut ticks, mut worst) = (0, 0, Vec::new());
    for frame in START + 2.. {
        if !cap.has(frame) {
            break;
        }
        seed_bones(&mut ee, &cap, frame - 1);
        lay_in(&mut ee, &cap, frame);
        set_rng(&mut ee, cap.int(frame - 1, "rng_state"));
        let (d, _) = port_tick(&mut ee, &cap, frame, 0, None, None);
        ticks += 1;
        exact += usize::from(d.is_empty());
        if !d.is_empty() && worst.len() < 4 {
            worst.push(format!("{frame}: {}", d[..d.len().min(6)].join("; ")));
        }
    }
    assert!(ticks >= 1200, "only {ticks} ticks in run1");
    assert_eq!(exact, ticks, "{exact} of {ticks} riding ticks exact\n{}", worst.join("\n"));
}
