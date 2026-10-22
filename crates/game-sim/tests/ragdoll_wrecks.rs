//! S56 — the rider's ragdoll through whole wrecks of the original, tick by tick.
//!
//! Four wrecks were recorded off the human's ride on Free09 (recording2's save state, the pad driven
//! by `research/s56/*.pad`): over the bars on a landing (`front`, reason 1), a side crash at speed
//! (`side`, reason 2), a loop-out backwards (`back`, reason 11) and a tip-over (`tip`, reason 3). Each
//! capture (`watch/s56-ragdoll.toml`, one row a tick) carries every bone's body, the actor's wreck
//! fields, the holds, the skeleton's rests and the whole riding request; each has save states from the
//! tick the wreck starts, which `research/s56_state_check.py` proved equal to their rows word for word.
//!
//! The RAM images (`./kit state ram context/capture/s56/<name>-<frame>.p2s --out
//! context/capture/s56/<name>-<frame>.ee.bin`) and scratchpads (the state's `Scratchpad.bin`, as
//! `<name>-<frame>.spr.bin`) are what a replay starts from.

use game_sim::rng::Rng;
use ps2_float::Vf;
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::HashMap;
use std::path::PathBuf;

/// The rider body actor in every s56 state (the human's vehicle less 0x400).
const ACTOR: u32 = 0x017a_7700;
const PARTS: u32 = 0x2f4;

/// One wreck: its capture's name, and the frame of its state.
struct Wreck {
    name: &'static str,
    frame: u32,
}

/// A state of each wreck from its first ticks: side's is the tick before the entry (the reason set,
/// `riding` still set); the others' saves landed a tick or two into the wreck.
const WRECKS: [Wreck; 4] = [
    Wreck { name: "front", frame: 2600 },
    Wreck { name: "side", frame: 2604 },
    Wreck { name: "back", frame: 2475 },
    Wreck { name: "tip", frame: 2475 },
];

/// Where an exploration writes what it found (a passing check keeps no log).
fn note(text: &str) {
    use std::io::Write;
    let Ok(kit) = Kit::load() else { return };
    let path = kit.root.join("context/capture/s56/notes.txt");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("the notes");
    writeln!(f, "{text}").expect("write the notes");
}

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (S56's captures: research/s56_wreck.py)", path.display()));
        return None;
    }
    Some(path)
}

/// A capture: its header's columns and its rows by frame counter.
struct Capture {
    cols: HashMap<String, usize>,
    rows: HashMap<u32, Vec<String>>,
}

impl Capture {
    fn load(name: &str) -> Option<Capture> {
        Capture::load_in("s56", name)
    }

    /// A capture of `context/capture/<dir>`.
    fn load_in(dir: &str, name: &str) -> Option<Capture> {
        let text = std::fs::read_to_string(ctx(&format!("capture/{dir}/{name}.csv"))?).expect("read the capture");
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

fn state(w: &Wreck) -> Option<Ee> {
    state_in("s56", w)
}

fn state_in(dir: &str, w: &Wreck) -> Option<Ee> {
    let path = ctx(&format!("capture/{dir}/{}-{}.ee.bin", w.name, w.frame))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Some(spr) = ctx(&format!("capture/{dir}/{}-{}.spr.bin", w.name, w.frame)) {
        ee.spr = std::fs::read(spr).expect("the scratchpad");
    }
    assert_eq!(ee.u32_at(ACTOR + 0x2e0), 18, "no rider body actor at {ACTOR:#x}");
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), w.frame, "the state is not frame {}", w.frame);
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
/// capture: the riding request (and the hand dummies 0x200 before it) and the bike centre its +0x1ac
/// points at. An s62 capture carries the wreck bike's whole object round that pointer too (S62:
/// `bike_a` from 0x980 before it, `bike_b` from 0x680 after it), and the broad phase's pools.
fn lay_in(ee: &mut Ee, cap: &Capture, frame: u32) {
    let req = ee.u32_at(ACTOR + 0x488);
    set_bytes(ee, req - 0x200, &cap.bytes(frame, "req_area"));
    let far = ee.u32_at(req + 0x1ac);
    if cap.cols.contains_key("bike_a") {
        set_bytes(ee, far - 0x980, &cap.bytes(frame, "bike_a"));
        set_bytes(ee, far + 0x680, &cap.bytes(frame, "bike_b"));
    }
    set_bytes(ee, far, &cap.bytes(frame, "req_far"));
    for (col, _) in cap.cols.iter().filter(|(c, _)| c.starts_with('w') && c.len() == 8) {
        let at = u32::from_str_radix(&col[1..], 16).expect("a world block's address");
        if (0x0179_0000..0x0181_0000).contains(&at) {
            continue;
        }
        set_bytes(ee, at, &cap.bytes(frame, col));
    }
}

/// Whether the recorder saw tick `frame - 1` again under the next frame counter (a save state's pause
/// can land between the counter's step and the simulation's): no tick ran between the two rows.
fn repeated(cap: &Capture, frame: u32) -> bool {
    cap.int(frame, "rng_state") == cap.int(frame - 1, "rng_state")
        && cap.quad(frame, "bone0.pos") == cap.quad(frame - 1, "bone0.pos")
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

/// One tick of the original's own update in the oracle, with the bike's side laid in from the capture,
/// checked against every captured bone. The rest of the game draws random numbers in the same tick,
/// before the body and after it; where the body draws, the draws before it are found by trying each
/// count in turn from the previous row's state. `Ok` holds that count and the body's own draws.
fn originals_tick(ee: &mut Ee, cap: &Capture, frame: u32) -> Result<(u32, u32), Vec<String>> {
    let update = addr("rider_body_update").unwrap();
    lay_in(ee, cap, frame);
    let before = cap.int(frame - 1, "rng_state");
    let snap = Snapshot::of(ee);
    let mut first = None;
    for n in 0..64 {
        snap.back(ee);
        let mut r = Rng::from_state(before);
        (0..n).for_each(|_| {
            r.rand();
        });
        set_rng(ee, r.state());
        ee.call(update, &[u64::from(ACTOR)], &[cap.get(frame, "dt")], 2_000_000_000);
        let drew = draws(r.state(), rng_at(ee), 1000).expect("the body's draws");
        let d = differ(cap, frame, &ee_bones(ee));
        if d.is_empty() {
            return Ok((n, drew));
        }
        first.get_or_insert(d);
        if drew == 0 {
            break; // the body drew nothing: the generator is not the difference
        }
    }
    snap.back(ee);
    Err(first.unwrap())
}

/// How far each wreck's state replays exactly through the original's own update with what the capture
/// lays in: up to the tick the rider and the bike (or another moving collidable the capture does not
/// carry) first touch. The tick after each is the first the replay misses.
const EXACT_TO: [(&str, u32); 4] = [("front", 2607), ("side", 2612), ("back", 2490), ("tip", 2481)];

/// The oracle reproduces the original's ragdoll tick for tick, bit for bit, from each wreck's state:
/// the entry with its six draws (side's state is the tick before it), the steps after, the duplicate
/// rows a save state leaves skipped. What the port is to be held against.
#[test]
fn the_originals_update_replays_each_wreck() {
    for (w, (name, to)) in WRECKS.iter().zip(EXACT_TO) {
        assert_eq!(w.name, name);
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w) else { return };
        let mut entry = None;
        for frame in w.frame + 1..=to {
            if repeated(&cap, frame) {
                continue;
            }
            match originals_tick(&mut ee, &cap, frame) {
                Ok((_, 6)) => entry = Some(frame),
                Ok(_) => {}
                Err(d) => panic!("{name}: tick {frame} parts in {} lanes: {:?}", d.len(), &d[..d.len().min(4)]),
            }
        }
        if name == "side" {
            assert_eq!(entry, Some(w.frame + 1), "side: the wreck entry's six draws");
        }
    }
}

/// The original's own update through each whole capture, from the wreck's first tick and from the
/// one after.
#[test]
#[ignore = "exploration"]
fn the_originals_update_replays_the_capture() {
    for w in WRECKS.iter().flat_map(|w| [Wreck { name: w.name, frame: w.frame }, Wreck { name: w.name, frame: w.frame + 1 }]) {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(&w) else { return };
        let mut frame = w.frame + 1;
        let mut log = Vec::new();
        let mut parted = None;
        while cap.has(frame) {
            if repeated(&cap, frame) {
                log.push(format!("{frame}: the row repeats {}", frame - 1));
                frame += 1;
                continue;
            }
            match originals_tick(&mut ee, &cap, frame) {
                Ok((n, drew)) if drew > 0 => log.push(format!("{frame}: {n} draws before, {drew} by the body")),
                Ok(_) => {}
                Err(d) => {
                    parted = Some(d);
                    break;
                }
            }
            frame += 1;
        }
        for l in &log {
            note(&format!("  {} {l}", w.name));
        }
        match parted {
            Some(d) => note(&format!(
                "{} from {}: exact to {}, {frame} parts in {} lanes {:?}",
                w.name,
                w.frame,
                frame - 1,
                d.len(),
                d.iter().take(4).collect::<Vec<_>>()
            )),
            None => note(&format!("{} from {}: exact through {}", w.name, w.frame, frame - 1)),
        }
    }
}

/// Exploration: what the original's update reads, over every tick of every capture, that the game
/// changes from tick to tick (the bytes consecutive states differ in) and that is not the actor's own
/// or laid in already: the ranges a capture has to carry for a replay to see the moving world.
#[test]
#[ignore = "exploration"]
fn what_the_wrecks_read() {
    let mut changing = std::collections::BTreeSet::new();
    for (name, a, b) in [("front", 2600, 2601), ("side", 2604, 2605), ("back", 2475, 2476), ("tip", 2475, 2476)] {
        let read = |f: u32| std::fs::read(ctx(&format!("capture/s56/{name}-{f}.ee.bin")).unwrap()).unwrap();
        let (ra, rb) = (read(a), read(b));
        changing.extend((0..ra.len()).filter(|&i| ra[i] != rb[i]).map(|i| i as u32));
    }
    let mut wanted = std::collections::BTreeSet::new();
    for w in &WRECKS {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w) else { return };
        let req = ee.u32_at(ACTOR + 0x488);
        let far = ee.u32_at(req + 0x1ac);
        let mut frame = w.frame + 1;
        let mut parted = None;
        while cap.has(frame) {
            *ee.reads.borrow_mut() = Some(Default::default());
            if originals_tick(&mut ee, &cap, frame).is_err() {
                parted.get_or_insert(frame);
                set_rng(&mut ee, cap.int(frame - 1, "rng_state"));
                ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[cap.get(frame, "dt")], 2_000_000_000);
            }
            let reads = ee.reads.borrow_mut().take().unwrap();
            wanted.extend(reads.into_iter().filter(|&x| {
                x >> 28 != 7
                    && !ee.written.contains(&x)
                    && !(req - 0x200..req + 0x300).contains(&x)
                    && !(far..far + 0x100).contains(&x)
                    && !(ACTOR..ACTOR + 0x1000).contains(&x)
                    && changing.contains(&(x & 0x01ff_ffff))
            }));
            frame += 1;
        }
        note(&format!("{} from {}: parted at {parted:?}", w.name, w.frame));
    }
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for &x in &wanted {
        let x = x & 0x01ff_ffff;
        match ranges.last_mut() {
            Some(last) if last.1 + 32 >= x => last.1 = x + 1,
            _ => ranges.push((x, x + 1)),
        }
    }
    note(&format!("{} ranges, {} bytes:", ranges.len(), ranges.iter().map(|r| r.1 - r.0).sum::<u32>()));
    for (a, b) in ranges {
        note(&format!("  {a:#x} {}", b - a));
    }
}

// ---------------------------------------------------------------------------------------------------
// S61: the port's tick against the original's
// ---------------------------------------------------------------------------------------------------

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
use game_sim::ragdoll_scene::{BikeSearch, Cells};
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
        let mut ask = |ee: &mut Ee, slot: u32| {
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

/// What the ticks compared went through, so that a pass on fewer fails.
#[derive(Debug, Default)]
struct Went {
    ticks: usize,
    entries: usize,
    touching: usize,
    off_bike: usize,
    flung: usize,
    landed: usize,
    settles: usize,
    settled: usize,
}

impl Went {
    fn add(&mut self, before: &Actor, after: &Ee) {
        let a = actor_of(after);
        let r = &a.ragdoll;
        self.ticks += 1;
        self.entries += usize::from(before.mode != Mode::Wrecked && a.mode == Mode::Wrecked);
        self.touching += usize::from(a.touching);
        self.off_bike += usize::from(r.off_bike && !before.ragdoll.off_bike);
        self.flung += usize::from(r.flung);
        self.landed += usize::from(before.ragdoll.flung && !r.flung);
        self.settles += usize::from(r.settled && !before.ragdoll.settled);
        self.settled += usize::from(r.settled);
    }
}

/// **The port's tick equals the original's on every tick of the s56 captures that the oracle
/// replays** (S61): from the same RAM, the port's actor update (the mode machine, the wreck's arm,
/// the step, the normalisation and the tail) and the original's, every number of every bone, pin,
/// joint and collision and every actor field the update writes compared bit for bit. The world's
/// answers (the ground, the scene's hits, the sweeps) and the skeleton's (its pose at the wreck
/// entry, the riding pose's points) are the original's own in the same tick, listened to.
#[test]
fn the_ports_tick_is_the_originals() {
    let mut went = Went::default();
    for (w, (name, to)) in WRECKS.iter().zip(EXACT_TO) {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w) else { return };
        for frame in w.frame + 1..=to {
            if repeated(&cap, frame) {
                continue;
            }
            let before = Snapshot::of(&ee);
            let (n, drew) = originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {d:?}"));
            before.back(&mut ee);
            lay_in(&mut ee, &cap, frame);
            let was = actor_of(&ee);
            let (d, _) = port_tick(&mut ee, &cap, frame, n, Some(drew), None);
            assert!(d.is_empty(), "{name}: tick {frame} parts in {} numbers: {:#?}", d.len(), &d[..d.len().min(12)]);
            went.add(&was, &ee);
        }
    }
    eprintln!("{went:?}");
    assert!(went.ticks >= 35 && went.entries == 1 && went.touching > 0, "{went:?}");
}

/// **The port's tick equals the original's past the replay**, to the end of each capture: the
/// original runs on from the last tick the capture replays with the bike's side still laid in from
/// the capture (its bones no longer the capture's, since the bike's collider the capture does not
/// carry has touched them), and from each tick's RAM the port's tick is compared with it as above.
/// This is where the bodies come off the bike, land and come to rest.
#[test]
fn the_ports_tick_is_the_originals_to_rest() {
    let mut went = Went::default();
    let mut hits = 0;
    for (w, (name, to)) in WRECKS.iter().zip(EXACT_TO) {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w) else { return };
        let mut frame = w.frame + 1;
        while cap.has(frame) {
            if repeated(&cap, frame) {
                frame += 1;
                continue;
            }
            if frame <= to {
                originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {d:?}"));
            } else {
                lay_in(&mut ee, &cap, frame);
                let was = actor_of(&ee);
                let (d, moving) = port_tick(&mut ee, &cap, frame, 0, None, None);
                hits += usize::from(moving > 0);
                if !d.is_empty() {
                    note(&format!("{name} {frame}:\n  {}", d.join("\n  ")));
                }
                assert!(d.is_empty(), "{name}: tick {frame} parts in {} numbers: {:#?}", d.len(), &d[..d.len().min(12)]);
                went.add(&was, &ee);
            }
            frame += 1;
        }
    }
    eprintln!("{went:?}, {hits} ticks with a moving hit");
    // Every capture comes to rest: the settle test throws in all four, and the flung, landed and
    // settled arms each run.
    assert!(went.ticks >= 850 && went.settles == 4, "{went:?}");
    assert!(went.flung > 0 && went.landed > 0 && went.settled > 0 && went.off_bike > 0, "{went:?}");
    // The ticks S61 left out, a bone's hit against another collidable (S62): compared too.
    assert!(hits >= 12, "{hits} ticks with a moving hit");
}

/// S62's captures (`research/s62_wreck.py`, context/capture/s62): each wreck's first state, and the
/// last tick the original's own update holds every captured bone through with the wreck bike's
/// object and the broad phase's pools laid in (tests/ragdoll_bike.rs).
const S62: [(&str, u32, u32); 5] =
    [("front", 2602, 2627), ("side", 2607, 3146), ("back", 2477, 3179), ("tip", 2472, 3179), ("front3", 2630, 2687)];

/// **The port replays each s62 wreck through the bike to the rider at rest** (S62): from each
/// wreck's first state the port's actor runs on its own state, tick after tick, handed only the
/// world's answers the original heard (the ground, the scene's hits and what each is with, the
/// sweeps, the skeleton), and every number the update writes is compared with the original's tick,
/// bit for bit. Up to the last tick the original holds the capture through, that is the game's own
/// wreck; past it (front's last 553 ticks) the original runs on in the oracle from its own state.
#[test]
fn the_port_replays_the_wrecks_through_the_bike() {
    let mut went = Went::default();
    let (mut hits, mut exact) = (0, 0);
    for (name, start, to) in S62 {
        let w = Wreck { name, frame: start };
        let Some(cap) = Capture::load_in("s62", name) else { return };
        let Some(mut ee) = state_in("s62", &w) else { return };
        let mut port = None;
        let settles = went.settles;
        let mut frame = start + 1;
        while cap.has(frame) {
            if repeated(&cap, frame) {
                frame += 1;
                continue;
            }
            let (n, drew) = if frame <= to {
                let before = Snapshot::of(&ee);
                let (n, drew) = originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {:?}", &d[..d.len().min(4)]));
                before.back(&mut ee);
                exact += 1;
                (n, Some(drew))
            } else {
                (0, None)
            };
            lay_in(&mut ee, &cap, frame);
            let was = actor_of(&ee);
            let (d, moving) = port_tick(&mut ee, &cap, frame, n, drew, Some(&mut port));
            hits += usize::from(moving > 0);
            if !d.is_empty() {
                note(&format!("s62 {name} {frame}:\n  {}", d.join("\n  ")));
            }
            assert!(d.is_empty(), "{name}: tick {frame} parts in {} numbers: {:#?}", d.len(), &d[..d.len().min(12)]);
            went.add(&was, &ee);
            frame += 1;
        }
        eprintln!("{name}: {start}..{}, the capture's own to {to}, settles {}", frame - 1, went.settles - settles);
    }
    eprintln!("{went:?}, {exact} ticks the capture's own, {hits} with a moving hit");
    // Every wreck through to the bones at rest, each settling once (front3's state is from inside its
    // wreck, so four entries), and the hits against the wreck bike's body: told they
    // were with the still scenery instead, the port parts at front's 2611.
    assert!(went.ticks >= 2600 && exact >= 2000 && went.entries == 4 && went.settles == 5 && went.settled > 2500, "{went:?}, {exact} exact");
    assert!(hits >= 11, "{hits} ticks with a moving hit");
}

// --- S105: the bones' spheres against the wreck bike's mesh ----------------------------------------

/// `wreckbikef.col` off the disc, as the shell loads it, or `None` (skipped) without a disc.
fn wreck_bike_mesh() -> Option<game_sim::ragdoll_scene::Bike> {
    use game_render::disc::Disc;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let col = names.iter().find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbikef.col").ok()));
    Some(game_sim::ragdoll_scene::Bike::parse(&col.expect("an archive carries wreckbikef.col")).expect("the file reads as a mesh"))
}

/// A quad as it lies in RAM, nothing flushed.
fn raw_quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| f32::from_bits(ee.u32_at(a + k))))
}

/// The wreck bike's collidable (the request's `+0x1ac`) and its frame (`+0x120`, rows then origin).
fn bike_collidable(ee: &Ee) -> (u32, game_sim::contact::Node) {
    let b = ee.u32_at(ee.u32_at(ACTOR + 0x488) + 0x1ac);
    let frame = game_sim::contact::Node { rows: [0, 1, 2].map(|k| raw_quad(ee, b + 0x120 + 0x10 * k)), origin: raw_quad(ee, b + 0x150) };
    (b, frame)
}

/// **The port's mesh is the one the original tests against** (S105): `wreckbikef.col` as the port
/// reads it, every node (children, centre, half extent), every triangle (corners, distance, the
/// normal in the loader's 16-bit form) and every vertex, equal to the wreck bike collidable's
/// arrays in RAM word for word.
#[test]
fn the_wreck_bike_mesh_is_the_ones_in_ram() {
    let Some(bike) = wreck_bike_mesh() else { return };
    let Some(ee) = state_in("s62", &Wreck { name: "front", frame: 2603 }) else { return };
    let (b, _) = bike_collidable(&ee);
    assert_eq!(ee.u32_at(b + 0x68), 0, "the wreck bike's collidable is a mesh");
    let (nodes, triangles, vertices) = (ee.u32_at(b + 0x234), ee.u32_at(b + 0x238), ee.u32_at(b + 0x220));
    assert_eq!(ee.u32_at(b + 0x228), 0, "wide vertices");
    let m = &bike.mesh;
    for (i, n) in m.nodes.iter().enumerate() {
        let at = nodes + 0x24 * i as u32;
        assert_eq!([ee.u16_at(at + 8), ee.u16_at(at + 10)], n.children, "node {i}'s children");
        for k in 0..3 {
            assert_eq!(ee.u32_at(at + 0xc + 4 * k as u32), n.centre[k].0.to_bits(), "node {i}'s centre");
            assert_eq!(ee.u32_at(at + 0x18 + 4 * k as u32), n.half_extent[k].0.to_bits(), "node {i}'s half extent");
        }
    }
    for (i, (t, packed)) in m.triangles.iter().zip(&bike.normals).enumerate() {
        let at = triangles + 0x14 * i as u32;
        assert_eq!([0, 2, 4].map(|k| ee.u16_at(at + k)), t.vertices, "triangle {i}'s corners");
        assert_eq!(ee.u32_at(at + 8), t.distance.0.to_bits(), "triangle {i}'s distance");
        assert_eq!([0xc, 0xe, 0x10].map(|k| ee.u16_at(at + k) as i16), *packed, "triangle {i}'s normal");
    }
    let game_sim::track::Points::Wide(v) = &m.vertices else { panic!("the file's vertices are packed") };
    for (i, p) in v.iter().enumerate() {
        for k in 0..3 {
            assert_eq!(ee.u32_at(vertices + 12 * i as u32 + 4 * k as u32), p[k].0.to_bits(), "vertex {i}");
        }
    }
    eprintln!("{} nodes, {} triangles, {} vertices", m.nodes.len(), m.triangles.len(), v.len());
}

/// The port's answer for a sphere at `centre` of `radius` against the bike in `frame`, as the
/// record's world contact, and the original's after its pair ran: `(point, normal)` bits.
type Answer = Option<([u32; 4], [u32; 4])>;

fn ports_answer(bike: &game_sim::ragdoll_scene::Bike, frame: &game_sim::contact::Node, centre: Vf, radius: f32) -> Answer {
    bike.touch(frame, centre, ps2_float::F(radius)).map(|t| (bits(t.world.0), bits(t.world.1)))
}

/// **The sphere test answers as the original's, over generated spheres** (S105): in front's 2603
/// state, 20,000 spheres stood near the wreck bike (round a random vertex, a random way off, radii
/// from a tenth to a metre and the bones' own three), each through the original's pair on a bone's
/// sphere collider with an emptied record, and through the port: hit or not, the plane distance,
/// and the contact's point and normal in the world, bit for bit.
#[test]
fn the_sphere_test_answers_as_the_originals() {
    let Some(bike) = wreck_bike_mesh() else { return };
    let Some(mut ee) = state_in("s62", &Wreck { name: "front", frame: 2603 }) else { return };
    let (b, frame) = bike_collidable(&ee);
    let sphere = ee.u32_at(collision(&ee, 0) + coll_at::SPHERE);
    let (clear, pair) = (addr("sphere_collider_clear").unwrap(), addr("sphere_collider_pair").unwrap());
    let game_sim::track::Points::Wide(v) = &bike.mesh.vertices else { panic!("packed") };
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 40) as f32) / (1u64 << 24) as f32
    };
    let mut hits = 0;
    for i in 0..20_000 {
        let p = frame.place(Vf::new(v[i % v.len()][0].0, v[i % v.len()][1].0, v[i % v.len()][2].0, 1.0));
        let radius = match i % 5 {
            0 => 0.4f32,
            1 => 0.75,
            2 => 0.5,
            _ => 0.1 + 0.9 * next(),
        };
        let off = Vf::new(next() - 0.5, next() - 0.5, next() - 0.5, 0.0);
        let reach = 4.0 * radius * next();
        let centre = Vf::new(p.x() + off.x() * reach, p.y() + off.y() * reach, p.z() + off.z() * reach, 1.0);
        for at in [0x40, 0x120] {
            ee.write(sphere + at, 16, u128::from_le_bytes(bytemuck_quad(centre)));
        }
        ee.set_f32(sphere + 0x170, radius);
        ee.set_f32(sphere + 0x174, ps2_float::fpu::mul(radius, radius));
        ee.call(clear, &[u64::from(sphere)], &[], 1_000_000);
        ee.call(pair, &[u64::from(sphere), u64::from(b)], &[], 50_000_000);
        let rec = ee.u32_at(sphere + coll_at::RECORD);
        let hit = ee.v0() != 0;
        let original: Answer = hit.then(|| {
            assert_eq!(ee.u32_at(rec + 4), 1, "{i}: one contact");
            assert_eq!(ee.u32_at(rec + 0x90), b, "{i}: with the bike");
            (bits(raw_quad(&ee, rec + 0x60)), bits(raw_quad(&ee, rec + 0x30)))
        });
        let port = ports_answer(&bike, &frame, centre, radius);
        assert_eq!(port, original, "{i}: sphere at {centre:?} r {radius}");
        if hit {
            let t = bike.touch(&frame, centre, ps2_float::F(radius)).unwrap();
            assert_eq!(t.distance.0.to_bits(), ee.u32_at(rec + 8), "{i}: the plane distance");
            hits += 1;
        }
    }
    eprintln!("{hits} of 20000 hit");
    assert!(hits > 4000 && hits < 16000, "{hits} hits");
}

fn bytemuck_quad(v: Vf) -> [u8; 16] {
    let mut out = [0; 16];
    for k in 0..4 {
        out[4 * k..4 * k + 4].copy_from_slice(&v[k].to_bits().to_le_bytes());
    }
    out
}

/// The bones' search as the original holds it (the one the actor's `+0x628` points at): where the
/// first collision's sphere stands, the list's centre, bound and built flag, and the bike's place in the list.
fn search_in_ram(ee: &Ee, b: u32, sphere: u32) -> BikeSearch {
    let s = ee.u32_at(ACTOR + 0x628);
    let n = ee.u32_at(s + 4);
    let found = (0..n).map(|i| ee.u32_at(s) + 8 * i).find(|&at| ee.u32_at(at) == b);
    BikeSearch {
        stood: Some(raw_quad(ee, sphere + 0x40)),
        centre: raw_quad(ee, s + 0x10),
        bound: F(ee.f32_at(s + 0x20)),
        built: ee.u32_at(s + 0x30) != 0,
        listed: found.is_some(),
        distance: found.map_or(F::ZERO, |at| F(ee.f32_at(at + 4))),
    }
}

/// The track's tree as the port lays it over the one in RAM's box (the bones' first sphere's
/// space's table's static structure), its origin, extent, cell and scale held equal to RAM's.
fn cells_of_ram(ee: &Ee, sphere: u32) -> (Cells, u32) {
    let tree = ee.u32_at(ee.u32_at(ee.u32_at(sphere + 4) + 4) + 0x34);
    let origin = [F(ee.f32_at(tree + 0x30)), F(ee.f32_at(tree + 0x38))];
    let extent = [F(ee.f32_at(tree + 0x48)), F(ee.f32_at(tree + 0x4c))];
    let cells = Cells::over(origin, [origin[0] + extent[0], origin[1] + extent[1]]);
    let t = &cells.tree;
    assert_eq!((t.origin, t.extent, t.cell, t.scale), (origin, extent, ee.i32_at(tree + 0x58), F(ee.f32_at(tree + 0x5c))), "the tree");
    (cells, tree)
}

/// Whether the tree under `node` holds `o` in one of its nodes' chunks of four.
fn tree_holds(ee: &Ee, node: u32, o: u32) -> bool {
    // Laying the capture in can leave a node pointing at what is no node in the Ee.
    let ram = |a: u32| (0x10_0000..0x200_0000).contains(&a) && a % 4 == 0;
    if !ram(node) {
        return false;
    }
    let mut c = ee.u32_at(node + 4);
    while c != 0 && ram(c) {
        if (1..5).any(|k| ee.u32_at(c + 4 * k) == o) {
            return true;
        }
        c = ee.u32_at(c);
    }
    let children = ee.u32_at(node);
    ram(children) && (0..4).any(|i| ee.u32_at(children + 4 * i) != 0 && tree_holds(ee, ee.u32_at(children + 4 * i), o))
}

/// The paths (child indices from the root) of the nodes under `node` that hold `o`.
fn paths_holding(ee: &Ee, node: u32, o: u32, path: &mut Vec<u32>, out: &mut Vec<Vec<u32>>) {
    let ram = |a: u32| (0x10_0000..0x200_0000).contains(&a) && a % 4 == 0;
    if !ram(node) {
        return;
    }
    let mut c = ee.u32_at(node + 4);
    while c != 0 && ram(c) {
        if (1..5).any(|k| ee.u32_at(c + 4 * k) == o) {
            out.push(path.clone());
            break;
        }
        c = ee.u32_at(c);
    }
    let children = ee.u32_at(node);
    if ram(children) {
        for i in 0..4 {
            let n = ee.u32_at(children + 4 * i);
            if n != 0 {
                path.push(i);
                paths_holding(ee, n, o, path, out);
                path.pop();
            }
        }
    }
}

/// The key of the nodes the Ee's tree holds the bike in, read back from their paths (child `2x + z`
/// of the bit each level halves, `0x4000` first), as `broad_build::insert` reads a key: the least
/// cell, a corner bit for each node, the level. `None` when no node holds it.
fn bike_key_in_tree(ee: &Ee, b: u32, tree: u32) -> Option<u32> {
    let mut paths = Vec::new();
    paths_holding(ee, ee.u32_at(tree + 0x94), b, &mut Vec::new(), &mut paths);
    let level = paths.first()?.len() as u32 - 1;
    assert!(paths.iter().all(|p| p.len() as u32 == level + 1), "the bike's nodes are on one level: {paths:?}");
    let cell = |p: &Vec<u32>| p.iter().enumerate().fold((0, 0), |(x, z), (l, &i)| (x | (i >> 1) * (0x4000 >> l), z | (i & 1) * (0x4000 >> l)));
    let cells: Vec<(u32, u32)> = paths.iter().map(cell).collect();
    let (x, z) = (cells.iter().map(|c| c.0).min()?, cells.iter().map(|c| c.1).min()?);
    let size = 0x4000 >> level;
    let corners = cells.iter().fold(0, |k, &(cx, cz)| {
        assert!(cx - x <= size && cz - z <= size, "the bike's nodes are a block of four: {paths:?}");
        k | 0x10 << (2 * u32::from(cx != x) + u32::from(cz != z))
    });
    Some((x << 17) & 0xfff0_0000 | (z << 5) & 0xf_ff00 | corners | level)
}

/// The bike's key in the tree as RAM holds it (its placement's `+0xc`), or `None` when it is in
/// none of the tree's nodes.
fn bike_key_in_ram(ee: &Ee, b: u32, tree: u32) -> Option<u32> {
    let place = ee.u32_at(b + 4);
    (place != 0 && tree_holds(ee, ee.u32_at(tree + 0x94), b)).then(|| ee.u32_at(place + 0xc))
}

/// **The game keeps the wreck bike in the track's tree where its sphere stands** (S105): in every
/// one of the twenty s62 states where the bike is in the broad phase, its placement is in the
/// track's tree, at its sphere's centre (its mesh's root box's, through its frame), under the key the
/// port's [`Cells::key`] gives that box, and the tree holds it there. In the four late states (some 400 frames after the wreck) the bike
/// is in no structure at all.
#[test]
fn the_bike_is_in_the_tree_where_its_sphere_stands() {
    let Some(bike) = wreck_bike_mesh() else { return };
    let states = [
        ("back", [2477, 2478, 2479, 2877]),
        ("front", [2602, 2603, 2604, 3002]),
        ("front3", [2630, 2631, 2632, 2668]),
        ("side", [2607, 2608, 2609, 3007]),
        ("tip", [2472, 2473, 2474, 2872]),
    ];
    let (mut placed, mut out) = (0, 0);
    for (name, frames) in states {
        for frame in frames {
            let Some(ee) = state_in("s62", &Wreck { name, frame }) else { return };
            let (b, _) = bike_collidable(&ee);
            let sphere = ee.u32_at(collision(&ee, 0) + coll_at::SPHERE);
            let (cells, tree) = cells_of_ram(&ee, sphere);
            let place = ee.u32_at(b + 4);
            if place == 0 {
                out += 1;
                continue;
            }
            assert_eq!(ee.u32_at(place + 8), tree, "{name} {frame}: in the track's tree");
            assert_eq!(bits(raw_quad(&ee, place + 0x10)), bits(raw_quad(&ee, b + 0x40)), "{name} {frame}: at its sphere");
            let (_, at) = bike_collidable(&ee);
            let m = Mat4([at.rows[0], at.rows[1], at.rows[2], at.origin]);
            let (c, _) = game_sim::flight_search::bike_box(&bike.mesh, &m);
            assert_eq!(c.map(|v| v.0.to_bits()), bits(raw_quad(&ee, b + 0x40)), "{name} {frame}: its box's centre is its sphere's");
            let key = cells.key(&bike, &m);
            assert_eq!(ee.u32_at(place + 0xc), key, "{name} {frame}: the key");
            assert_eq!(bike_key_in_ram(&ee, b, tree), Some(key), "{name} {frame}: the tree holds it");
            assert_eq!(bike_key_in_tree(&ee, b, tree), Some(key), "{name} {frame}: its nodes read back to its key");
            placed += 1;
        }
    }
    eprintln!("{placed} states with the bike in the tree, {out} with it in none");
    assert_eq!((placed, out), (16, 4));
}

/// **Every sphere test of the five s62 wrecks, answered by the port** (S105): the original's update
/// runs each wreck as S62's replay does, and the port's search for the bike ([`BikeSearch`]) is
/// carried beside it from the wreck's first state: gathered each frame where the first sphere stood
/// (offered the bike once the bones are off it), and after the update equal to the original's search
/// (centre, bound, built, the bike listed and its distance), bit for bit. At each of the bones' sphere
/// tests the port pairs the sphere with the bike exactly when the original's search does, and tests
/// it against the mesh in the bike collidable's frame of that moment: every test the original
/// answered with a hit is the port's hit with the same world point and normal, and every miss a miss.
#[test]
fn every_sphere_test_of_the_wrecks_is_the_bike_meshs() {
    let Some(bike) = wreck_bike_mesh() else { return };
    let bike = Rc::new(bike);
    #[derive(Default)]
    struct Count {
        tests: usize,
        hits: usize,
        paired: usize,
        gathers: usize,
        rebuilt: usize,
        listed: usize,
        /// The pair calls with the bike in the sphere test running now.
        now: Vec<u32>,
        wrong: Vec<String>,
    }
    let count = Rc::new(RefCell::new(Count::default()));
    let update = addr("rider_body_update").unwrap();
    for (name, start, to) in S62 {
        let w = Wreck { name, frame: start };
        let Some(cap) = Capture::load_in("s62", name) else { return };
        let Some(mut ee) = state_in("s62", &w) else { return };
        let spheres: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(&ee, i) + coll_at::SPHERE)).collect();
        let (b, _) = bike_collidable(&ee);
        let (cells, tree) = cells_of_ram(&ee, spheres[0]);
        let model = Rc::new(RefCell::new(search_in_ram(&ee, b, spheres[0])));
        let mut frame = start + 1;
        while cap.has(frame) {
            if repeated(&cap, frame) {
                frame += 1;
                continue;
            }
            let n = if frame <= to {
                let before = Snapshot::of(&ee);
                let (n, _) = originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {:?}", &d[..d.len().min(4)]));
                before.back(&mut ee);
                n
            } else {
                0
            };
            lay_in(&mut ee, &cap, frame);
            let mut r = Rng::from_state(cap.int(frame - 1, "rng_state"));
            (0..n).for_each(|_| {
                r.rand();
            });
            set_rng(&mut ee, r.state());
            let saved = std::mem::take(&mut ee.hooks);
            let centre = Rc::new(RefCell::new(Vf::default()));
            let c = Rc::clone(&centre);
            ee.hooks.insert(
                addr("collider_set_position").unwrap(),
                Box::new(move |ee: &mut Ee| {
                    *c.borrow_mut() = raw_quad(ee, ee.r(5) as u32);
                    through(ee);
                }),
            );
            let (k, c) = (Rc::clone(&count), Rc::clone(&centre));
            let spheres_at = spheres.clone();
            ee.hooks.insert(
                addr("sphere_collider_pair").unwrap(),
                Box::new(move |ee: &mut Ee| {
                    let (s, o) = (ee.r(4) as u32, ee.r(5) as u32);
                    if spheres_at.contains(&s) && o == b {
                        let mut k = k.borrow_mut();
                        k.paired += 1;
                        k.now.push(s);
                        let rec = ee.u32_at(s + coll_at::RECORD);
                        if bits(raw_quad(ee, s + 0x40)) != bits(*c.borrow()) || ee.u32_at(rec + 4) != 0 {
                            k.wrong.push(format!("pair: the sphere's centre or a record not empty"));
                        }
                    }
                    through(ee);
                }),
            );
            let collide = |k: Rc<RefCell<Count>>, c: Rc<RefCell<Vf>>, spheres: Vec<u32>, bike: Rc<game_sim::ragdoll_scene::Bike>, model: Rc<RefCell<BikeSearch>>| {
                Box::new(move |ee: &mut Ee| {
                    let s = ee.r(4) as u32;
                    if !spheres.contains(&s) {
                        return through(ee);
                    }
                    let radius = f32::from_bits(ee.u32_at(s + 0x170));
                    let reaches = model.borrow().reaches(F(ee.f32_at(s + 0x60)));
                    let (_, at) = bike_collidable(ee);
                    let port = if reaches { ports_answer(&bike, &at, *c.borrow(), radius) } else { None };
                    k.borrow_mut().now.clear();
                    through(ee);
                    let hit = ee.u8_at(s + coll_at::FLAGS) & 0x20 != 0;
                    let rec = ee.u32_at(s + coll_at::RECORD);
                    let mut k = k.borrow_mut();
                    k.tests += 1;
                    let original: Answer = hit.then(|| {
                        if ee.u32_at(rec + 4) != 1 || ee.u32_at(rec + 0x90) != b {
                            k.wrong.push(format!("{} contacts, with {:#x}", ee.u32_at(rec + 4), ee.u32_at(rec + 0x90)));
                        }
                        (bits(raw_quad(ee, rec + 0x60)), bits(raw_quad(ee, rec + 0x30)))
                    });
                    k.hits += usize::from(hit);
                    let paired = k.now.contains(&s);
                    if paired != reaches || port != original {
                        k.wrong.push(format!(
                            "sphere {s:#x} at {:?} r {radius}: paired {paired}, the port's search {:?}; port {port:x?}, original {original:x?}",
                            *c.borrow(),
                            *model.borrow(),
                        ));
                    }
                }) as Box<dyn FnMut(&mut Ee)>
            };
            for hook in ["collide_against_search", "probe_collide_world"] {
                let f = collide(Rc::clone(&count), Rc::clone(&centre), spheres.clone(), Rc::clone(&bike), Rc::clone(&model));
                ee.hooks.insert(addr(hook).unwrap(), f);
            }
            // The port's gather where the original's runs: once a frame in the wreck mode only (a
            // body put back on the bike stops), round the first sphere where it stands, the bike
            // offered once the bones are off it, and the bike where the Ee's tree holds it. That
            // is not where the game would: the replay never runs the bike's own update, which puts
            // it in the tree where it stands every frame, and laying in the capture's bike takes
            // it out of its node once the game has moved it to another (front, from 2619).
            let (m, k, cells, s0) = (Rc::clone(&model), Rc::clone(&count), cells.clone(), spheres[0]);
            ee.hooks.insert(
                addr("collision_search_gather").unwrap(),
                Box::new(move |ee: &mut Ee| {
                    let key = bike_key_in_tree(ee, b, tree);
                    let mut m = m.borrow_mut();
                    let before = *m;
                    m.stood = Some(raw_quad(ee, s0 + 0x40));
                    let reach = F(ee.f32_at(s0 + 0x60));
                    let bike_sphere = (raw_quad(ee, b + 0x40), F(ee.f32_at(b + 0x60)));
                    let offered = ee.u32_at(ACTOR + 0x6b0) != 0 && key.is_some();
                    m.gather(Some(&cells), reach, bike_sphere, key.unwrap_or(0), offered);
                    let mut k = k.borrow_mut();
                    k.gathers += 1;
                    k.rebuilt += usize::from(bits(m.centre) != bits(before.centre) || m.bound.0.to_bits() != before.bound.0.to_bits());
                    k.listed += usize::from(m.listed);
                    through(ee);
                }),
            );
            ee.call(update, &[u64::from(ACTOR)], &[cap.get(frame, "dt")], 2_000_000_000);
            ee.hooks = saved;
            let mut theirs = search_in_ram(&ee, b, spheres[0]);
            let ours = *model.borrow();
            theirs.stood = ours.stood;
            if !theirs.listed {
                // A distance means nothing for a bike not in the list.
                theirs.distance = ours.distance;
            }
            let k = count.borrow();
            assert_eq!(format!("{ours:?}"), format!("{theirs:?}"), "{name} {frame}: the search");
            assert!(k.wrong.is_empty(), "{name} {frame}: {:#?}", &k.wrong[..k.wrong.len().min(6)]);
            drop(k);
            frame += 1;
        }
    }
    let k = count.borrow();
    eprintln!(
        "{} sphere tests, {} paired with the bike, {} hits; {} gathers, {} rebuilt, {} with the bike listed",
        k.tests, k.paired, k.hits, k.gathers, k.rebuilt, k.listed
    );
    let counts = (k.tests, k.paired, k.hits, k.gathers, k.rebuilt, k.listed);
    assert_eq!(counts, (24822, 600, 64, 367, 12, 176), "tests, paired, hits, gathers, rebuilt, listed");
}
