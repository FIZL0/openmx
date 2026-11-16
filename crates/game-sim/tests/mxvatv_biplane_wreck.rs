//! MD101c — SLUS-21104's biplane wreck/recover machine against the port's ([`Crash::step_mxvatv`]).
//!
//! The plane's crash test (its vtable's slot after the wreck check's caller, research/oracle.txt
//! `# MD101c`) runs once a frame: MXVehicle's wreck check (the edge throw, reason 13) unless the plane
//! is already thrown; then, flying, a contact faster than `MinWreckSpeed` on a surface the plane's mask
//! does not spare, or upside down on any, is reason 1; wrecked, the clock at `+0xc84` runs, a thrown
//! plane latches (`+0xc8c`) once it has been out 2.5 s and is down, and past `MaxWreckTime` (`+0xc88`)
//! the plane recovers (a thrown one keeping 0.3 of its velocity). The wreck entry starts the clock at
//! zero.
//!
//! Three proofs:
//!
//! 1. **the oracle**: ten thousand generated cases of the crash test on MD80a's biplane3 image
//!    (`context/mxvatv-ps2/md80a/biplane3.ee.bin`), with the real wreck check, terrain edge test and
//!    wreck entry, `rand` handing out the port's generator's numbers and the unwreck recorded (it
//!    puts the plane back, which is not the clock's);
//! 2. **the crash capture** (`context/capture/md80a/biplane3crash.csv`, `research/md80a/fly.sh` with
//!    `biplane3crash.pad`: off the runway, then the stick held into a dive until it strikes the
//!    desert): the interpreter flies it from the same image as tests/mxvatv_biplane_replay.rs does,
//!    every row's physics blocks and wreck words word for word, and the crash test runs the original
//!    with a port [`Crash`] beside it, fed each frame from the plane's words; the port's reason, clock
//!    and latch must equal the original's after every frame, the port's own clock running from the
//!    wreck to the recovery.
//! 3. **the crash mask** (MD113): the plane's mask and a struck object's class are categories by name,
//!    VEHICLE in each game's own bit; the original's crash test, struck by each class on the image,
//!    spares exactly what the port's [`game_sim::flight_search::VEHICLE`] rule spares.
//!
//! Skips without the image, the capture or its tape.

use game_sim::biplane::{Area, Crash, CrashContact, THROWN};
use game_sim::input::{PlaneCommand, PlaneInput};
use game_sim::rng::Rng;
use ps2_float::{Dest, F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const CASES: usize = 10_000;
const BUDGET: u64 = 400_000_000;
const CALL_BUDGET: u64 = 2_000_000;

/// The player's plane in biplane3's image, its vtable, and SLUS-21104's `$gp`.
const PLANE: u32 = 0x01b1_3790;
const BIPLANE_VTABLE: u32 = 0x003c_e7b0;
const GP: u64 = 0x003d_baf0;
/// The functions (research/oracle.txt `# MD101c`, `# MD80b`, `# MD101`).
const CRASH_TEST: u32 = 0x004c_4110;
const UNWRECK: u32 = 0x004c_4350;
const TERRAIN_EDGE: u32 = 0x0031_d410;
const RAND: u32 = 0x0035_61a8;
const POST_EVENT: u32 = 0x0027_2300;
const RUMBLE_ON: u32 = 0x0051_a250;
/// The sim frame and its tick (tests/mxvatv_biplane_replay.rs).
const FRAME: u32 = 0x0020_4040;
const DT: u32 = 0x3d08_850a;
const SIM_VTABLE: u32 = 0x003b_9320;
const VSYNC: u32 = 0x003f_a6d8;

/// The plane's words the crash test reads and writes.
mod at {
    pub const BODY: u32 = 0xb8;
    pub const RECORD: u32 = 0xc8;
    pub const GRAVITY: u32 = 0xe8;
    pub const GATE: u32 = 0x308;
    pub const MASK: u32 = 0x30c;
    pub const GROUNDED: u32 = 0x324;
    pub const SPEED: u32 = 0x32c;
    pub const UP_Y: u32 = 0xbf4;
    pub const REASON: u32 = 0xc80;
    pub const CLOCK: u32 = 0xc84;
    pub const MAX_WRECK_TIME: u32 = 0xc88;
    pub const LATCH: u32 = 0xc8c;
    pub const MIN_WRECK_SPEED: u32 = 0x1160;
    pub const TERRAIN: u32 = 0x3b0;
    /// The collision record's: touching, and the object it struck (its `+0x94` the class).
    pub const TOUCHING: u32 = 0x34;
    pub const STRUCK: u32 = 0x20c;
    pub const CLASS: u32 = 0x94;
}
const BODY_POS: u32 = 0x70;
const BODY_VEL: u32 = 0x80;
const BODY_RATES: u32 = 0x1d0;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md80a").join(format!("biplane3.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("no {} (python3 -I research/md80a/ram.py biplane3; the .p2s in kept-context/md80a)", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, GP);
    assert_eq!(ee.u32_at(PLANE + 4), BIPLANE_VTABLE, "no biplane at {PLANE:#x}");
    Some(ee)
}

fn vector(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn put_vector(ee: &mut Ee, a: u32, v: [f32; 4]) {
    for (k, x) in v.into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}

fn bits(v: Vf) -> [u32; 4] {
    [v.x(), v.y(), v.z(), v.w()].map(f32::to_bits)
}

/// The terrain's extents, from the edge test on a point out on every side (the edge test hands them
/// back only when it says yes; tests/mxvatv_biplane.rs proves [`Area::outside_mxvatv`] on it).
fn area_of(ee: &mut Ee) -> Area {
    let terrain = ee.u32_at(PLANE + at::TERRAIN);
    let (x_out, z_out, point) = (0x01f8_0000u32, 0x01f8_0010u32, 0x01f8_0020u32);
    put_vector(ee, point, [-1.0, 0.0, -1.0, 1.0]);
    ee.call(TERRAIN_EDGE, &[u64::from(terrain), u64::from(point), u64::from(x_out), u64::from(z_out), 0], &[150.0], CALL_BUDGET);
    assert_eq!(ee.r(2), 1, "a point at (-1, -1) is not by the edge");
    Area { width: F(ee.f32_at(x_out)), depth: F(ee.f32_at(z_out)) }
}

/// What the crash test reads off the plane, as the port's crash test takes it.
struct Seen {
    contact: CrashContact,
    crash: Crash,
    position: Vf,
    velocity: Vf,
    rates: Vf,
    scale: F,
    dt: F,
}

fn seen(ee: &Ee, dt: f32) -> Seen {
    let record = ee.u32_at(PLANE + at::RECORD);
    let struck = ee.u32_at(record + at::STRUCK);
    let mask = ee.u32_at(PLANE + at::MASK);
    let body = ee.u32_at(PLANE + at::BODY);
    let contact = CrashContact {
        touching: ee.u32_at(record + at::TOUCHING) != 0,
        ignored: struck != 0 && ee.u32_at(struck + at::CLASS) & mask != 0,
        grounded: ee.u32_at(PLANE + at::GROUNDED) != 0,
        speed: F(ee.f32_at(PLANE + at::SPEED)),
        up_y: F(ee.f32_at(PLANE + at::UP_Y)),
        holed: false,
        bare: false,
    };
    let crash = Crash {
        reason: ee.u32_at(PLANE + at::REASON),
        landed: ee.u32_at(PLANE + at::LATCH) != 0,
        min_wreck_speed: F(ee.f32_at(PLANE + at::MIN_WRECK_SPEED)),
        wreck_time: F(ee.f32_at(PLANE + at::MAX_WRECK_TIME)),
        clock: F(ee.f32_at(PLANE + at::CLOCK)),
    };
    Seen {
        contact,
        crash,
        position: vector(ee, body + BODY_POS),
        velocity: vector(ee, body + BODY_VEL),
        rates: vector(ee, body + BODY_RATES),
        scale: F(ee.f32_at(ee.u32_at(PLANE + at::GRAVITY) + 4)),
        dt: F(dt),
    }
}

/// The original's words after the crash test: reason, clock, latch.
fn words(ee: &Ee) -> (u32, u32, u32) {
    (ee.u32_at(PLANE + at::REASON), ee.u32_at(PLANE + at::CLOCK), ee.u32_at(PLANE + at::LATCH))
}

/// A deterministic generator for the cases (tests/biplane.rs's).
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
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    /// A value on, a step either side of, or about `x`.
    fn about(&mut self, x: f32) -> f32 {
        // never a denormal or a NaN (a step off a zero): the PS2 has neither, and the port's F flushes them
        let v = match self.next() % 8 {
            0 => x,
            1 => f32::from_bits(x.to_bits() + 1),
            2 => f32::from_bits(x.to_bits().wrapping_sub(1)),
            3 => -0.0,
            _ => x * self.range(0.0, 2.0),
        };
        if v.is_subnormal() || v.is_nan() { 0.0 } else { v }
    }
}

// ---------------------------------------------------------------------------------------------------
// 1 — the oracle
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_mxvatv_crash_test_matches_the_original() {
    let Some(mut ee) = image() else { return };
    let area = area_of(&mut ee);
    let rng = Rc::new(RefCell::new(Rng::from_seed(0x0d10_1c0c)));
    let r = rng.clone();
    ee.hooks.insert(RAND, Box::new(move |ee| {
        let v = r.borrow_mut().rand();
        ee.set(2, v as u32 as u64);
    }));
    // The events (a3: 11 the wreck entry, 12 the thrown plane's latch) and the unwreck, recorded.
    let posts = Rc::new(RefCell::new(Vec::new()));
    let p = posts.clone();
    ee.hooks.insert(POST_EVENT, Box::new(move |ee| p.borrow_mut().push(ee.r(7) as u32)));
    let unwrecks = Rc::new(RefCell::new(0usize));
    let u = unwrecks.clone();
    ee.hooks.insert(UNWRECK, Box::new(move |_| *u.borrow_mut() += 1));
    ee.hooks.insert(RUMBLE_ON, Box::new(|ee| ee.set(2, 0)));
    let record = ee.u32_at(PLANE + at::RECORD);
    let body = ee.u32_at(PLANE + at::BODY);
    let gravity = ee.u32_at(PLANE + at::GRAVITY) + 4;
    let thing = 0x01f8_0100u32;
    let mut pick = Pick(0x6d10_1c5e_ed0f_c4a5);
    let (mut wrecks, mut recoveries, mut latches, mut thrown) = (0, 0, 0, 0);
    for case in 0..CASES {
        let reason = match pick.next() % 10 {
            0..=3 => 0,
            4..=6 => 1,
            7 | 8 => THROWN,
            _ => [5, 7, 11, 14][pick.next() as usize % 4],
        };
        let max = match pick.next() % 4 {
            0 | 1 => 5.0,
            2 => pick.about(2.5),
            _ => pick.range(0.0, 8.0),
        };
        let clock = match pick.next() % 6 {
            0 => 0.0,
            1 => pick.about(2.5),
            2 | 3 => pick.about(max),
            4 if pick.one_in(16) => f32::from_bits(0x7f7f_ffff),
            _ => pick.range(-1.0, 9.0),
        };
        let dt = match pick.next() % 4 {
            0 | 1 => f32::from_bits(DT),
            2 => 0.0,
            _ => pick.range(0.0, 0.5),
        };
        let min = if pick.one_in(2) { 73.333 } else { pick.range(0.0, 150.0) };
        let speed = match pick.next() % 4 {
            0 => min,
            1 => pick.about(min),
            _ => pick.range(0.0, 200.0),
        };
        let up_y = match pick.next() % 4 {
            0 => 0.0,
            1 => -0.0,
            _ => pick.range(-1.0, 1.0),
        };
        // the point: inside, by an edge (thrown unless already), or about the margin
        let side = |pick: &mut Pick, extent: f32| match pick.next() % 8 {
            0 => 150.0,
            1 => extent - 150.0,
            2 => pick.range(-200.0, 150.0),
            _ => pick.range(200.0, extent - 200.0),
        };
        let position = [side(&mut pick, area.width.to_f32()), pick.range(0.0, 900.0), side(&mut pick, area.depth.to_f32()), 1.0];
        let velocity = [0; 4].map(|_| pick.range(-200.0, 200.0));
        let rates = [0; 4].map(|_| pick.range(-3.0, 3.0));
        let scale = pick.range(0.0, 3.0);
        let mask = if pick.one_in(2) { 4 } else { pick.next() as u32 };
        let struck = if pick.one_in(3) { 0 } else { thing };
        let class = [0, 4, 0x2000, u32::MAX, pick.next() as u32][pick.next() as usize % 5];

        ee.set_u32(PLANE + at::REASON, reason);
        ee.set_f32(PLANE + at::CLOCK, clock);
        ee.set_u32(PLANE + at::LATCH, u32::from(pick.one_in(2)));
        ee.set_f32(PLANE + at::MAX_WRECK_TIME, max);
        ee.set_f32(PLANE + at::MIN_WRECK_SPEED, min);
        ee.set_f32(PLANE + at::SPEED, speed);
        ee.set_f32(PLANE + at::UP_Y, up_y);
        ee.set_u32(PLANE + at::GROUNDED, u32::from(pick.one_in(2)));
        ee.set_u32(PLANE + at::GATE, 1);
        ee.set_u32(PLANE + at::MASK, mask);
        ee.set_u32(record + at::TOUCHING, u32::from(pick.one_in(2)));
        ee.set_u32(record + at::STRUCK, struck);
        ee.set_u32(thing + at::CLASS, class);
        put_vector(&mut ee, body + BODY_POS, position);
        put_vector(&mut ee, body + BODY_VEL, velocity);
        put_vector(&mut ee, body + BODY_RATES, rates);
        ee.set_f32(gravity, scale);

        let s = seen(&ee, dt);
        let mut port_rng = rng.borrow().clone();
        let mut crash = s.crash;
        let (mut v, mut spin, mut g) = (s.velocity, s.rates, s.scale);
        let out = crash.step_mxvatv(&s.contact, Some(&area), s.position, &mut v, &mut spin, &mut g, &mut port_rng, s.dt);
        posts.borrow_mut().clear();
        *unwrecks.borrow_mut() = 0;
        ee.call(CRASH_TEST, &[u64::from(PLANE)], &[dt], CALL_BUDGET);

        let what = || format!("case {case}: reason {reason} clock {clock} max {max} dt {dt} {:?}", s.contact);
        assert_eq!(words(&ee), (crash.reason, crash.clock.to_bits(), u32::from(crash.landed)), "{}: reason, clock, latch", what());
        assert_eq!(*unwrecks.borrow() == 1, out.recover, "{}: the recovery", what());
        let entered = posts.borrow().iter().filter(|&&e| e == 11).count();
        assert_eq!(entered, usize::from(out.wreck.is_some()), "{}: the wreck entry", what());
        assert_eq!(posts.borrow().iter().filter(|&&e| e == 12).count(), usize::from(crash.landed && !s.crash.landed), "{}: the latch", what());
        assert_eq!(bits(vector(&ee, body + BODY_VEL)), bits(v), "{}: the velocity", what());
        // the wreck entry spins the body up by four (the port's Flight::wreck)
        let spin = if out.wreck.is_some() { spin.merge(Dest::XYZ, spin.mul(Vf::splat(4.0))) } else { spin };
        assert_eq!(bits(vector(&ee, body + BODY_RATES)), bits(spin), "{}: the rates", what());
        assert_eq!(ee.f32_at(gravity).to_bits(), g.to_bits(), "{}: the gravity scale", what());
        assert_eq!(port_rng.state(), rng.borrow().state(), "{}: the draws", what());
        wrecks += usize::from(out.wreck.is_some());
        thrown += usize::from(out.wreck == Some(THROWN));
        recoveries += usize::from(out.recover);
        latches += usize::from(crash.landed && !s.crash.landed);
    }
    println!("the crash test: {CASES} cases, {wrecks} wrecks ({thrown} thrown), {latches} latches, {recoveries} recoveries");
    assert!(wrecks > 500 && thrown > 200 && latches > 50 && recoveries > 500, "the cases miss a branch");
}

// ---------------------------------------------------------------------------------------------------
// 2 — the crash capture
// ---------------------------------------------------------------------------------------------------

type Row = HashMap<String, String>;

fn table(path: std::path::PathBuf, why: &str) -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} ({why})", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(String::from).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(String::from)).collect()).collect())
}

fn hex_words(s: &str) -> Vec<u32> {
    s.as_bytes()
        .chunks(8)
        .map(|c| {
            let b: Vec<u8> = (0..4).map(|i| u8::from_str_radix(std::str::from_utf8(&c[2 * i..2 * i + 2]).unwrap(), 16).unwrap()).collect();
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        })
        .collect()
}

/// tests/mxvatv_biplane_replay.rs's hooks: the kernel's DIntr/EIntr, the network send, the thread
/// delay, the sound driver's IOP call and every syscall stub.
fn hooked(ee: &mut Ee) {
    ee.hooks.insert(0x0034_8fa0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0034_8ff0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x004e_7e70, Box::new(|_: &mut Ee| {}));
    ee.hooks.insert(0x0033_ff58, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0018_2c90, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
}

fn sim(ee: &Ee) -> u32 {
    let hits: Vec<u32> = (0x0010_0000..0x01ff_fff0u32).step_by(4).filter(|&a| ee.u32_at(a + 4) == SIM_VTABLE).collect();
    assert_eq!(hits.len(), 1, "sim objects {hits:x?}");
    hits[0]
}

/// The tape's buttons (MX vs. ATV's shipped bindings, tests/mxvatv_biplane_replay.rs).
fn button(c: PlaneCommand) -> &'static str {
    match c {
        PlaneCommand::ThrottleUp => "cross",
        PlaneCommand::ThrottleDown => "square",
        PlaneCommand::RudderLeft => "l1",
        PlaneCommand::RudderRight => "r1",
        PlaneCommand::BankLeft => "left",
        PlaneCommand::BankRight => "right",
        PlaneCommand::Dive => "up",
        PlaneCommand::Climb => "down",
    }
}

/// The input step's words for frame `j`: tape tick `j - 2`'s (tests/mxvatv_biplane_replay.rs's feed).
fn feed(ee: &mut Ee, ticks: &[Row], j: usize) {
    let Some(t) = j.checked_sub(2).and_then(|t| ticks.get(t)) else { return };
    let held: Vec<&str> = t["buttons"].split(' ').collect();
    let stick = |k: &str| F(t[k].parse::<f32>().unwrap());
    let i = PlaneInput::from_sticks((stick("lx"), stick("ly")), |c| if held.contains(&button(c)) { F::ONE } else { F::ZERO });
    for (o, v) in [(0x310, i.throttle_down), (0x314, i.throttle_up), (0x10f4, i.elevator), (0x10f8, i.aileron), (0x10fc, i.rudder)] {
        ee.set_u32(PLANE + o, v.to_bits());
    }
}

/// A row's words: the body, engine, aero and tires, and the three wreck words (+0xc80..+0xc8c, in the
/// row's `obj1` block from +0xc00).
fn row_words(row: &Row) -> Vec<u32> {
    let mut w: Vec<u32> = ["body", "engine", "aero", "tire0.raw", "tire1.raw", "tire2.raw"].iter().flat_map(|n| hex_words(&row[*n])).collect();
    let obj1 = hex_words(&row["obj1"]);
    w.extend_from_slice(&obj1[0x20..0x24]);
    w
}

fn ram_words(ee: &Ee) -> Vec<u32> {
    let p = |o: u32| ee.u32_at(PLANE + o);
    let blocks = [(p(0xb8), 640), (p(0x1164), 512), (p(0x1168), 512), (p(0x116c), 512), (p(0x1170), 512), (p(0x1174), 512), (PLANE + 0xc80, 16)];
    blocks.iter().flat_map(|&(at, len)| (0..len / 4).map(move |i| at + 4 * i)).map(|a| ee.u32_at(a)).collect()
}

/// The port beside the original: its crash state, the last frame's, and every disagreement.
struct Beside {
    crash: Option<Crash>,
    area: Area,
    rng: Rng,
    frame: usize,
    wrecked: Vec<(usize, u32)>,
    recovered: Vec<usize>,
    clocked: usize,
    errors: Vec<String>,
}

#[test]
fn the_mxvatv_wreck_clock_matches_the_crash_capture() {
    let Some(mut ee) = image() else { return };
    let kit = Kit::load().unwrap();
    let Some(rows) = table(kit.ctx("capture/md80a").join(format!("biplane3crash{}", ".csv")), "research/md80a/fly.sh with biplane3crash.pad") else { return };
    let Some(ticks) = table(kit.ctx("recordings/md80a").join(format!("biplane3crash.p2m2.ticks{}", ".csv")), "research/md80a/fly.sh") else { return };
    let area = {
        let mut other = image().unwrap();
        area_of(&mut other)
    };
    hooked(&mut ee);
    let beside = Rc::new(RefCell::new(Beside { crash: None, area, rng: Rng::from_seed(1), frame: 0, wrecked: Vec::new(), recovered: Vec::new(), clocked: 0, errors: Vec::new() }));
    let b = beside.clone();
    // The crash test, run as the original inside the frame (this hook is out of the map while it runs,
    // so the nested call is the game's own) on a stack below the caller's, with the port beside it.
    ee.hooks.insert(CRASH_TEST, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, PLANE, "the crash test of another vehicle");
        let dt = ee.f(12);
        let s = seen(ee, dt);
        let (ra, sp, stack, steps) = (ee.r(31), ee.r(29), ee.stack, ee.steps);
        ee.stack = (sp as u32 - 0x100) & !0xf;
        ee.call(CRASH_TEST, &[u64::from(PLANE)], &[dt], CALL_BUDGET);
        ee.stack = stack;
        ee.steps += steps;
        ee.set(29, sp);
        ee.set(31, ra);
        // the hook goes back in under the pc it leaves (Ee::call), and the nested call left DONE there
        ee.pc = CRASH_TEST;
        let got = words(ee);
        let mut b = b.borrow_mut();
        let Beside { crash, area, rng, frame, .. } = &mut *b;
        let frame = *frame;
        // the port's state is its own from the first frame on: only the first is seeded off the plane
        let mut c = crash.unwrap_or(s.crash);
        let (mut v, mut r, mut g) = (s.velocity, s.rates, s.scale);
        let out = c.step_mxvatv(&s.contact, Some(&*area), s.position, &mut v, &mut r, &mut g, rng, s.dt);
        if out.recover {
            // Flight::unwreck's
            (c.reason, c.clock, c.landed) = (0, F::ZERO, false);
        }
        *crash = Some(c);
        // the unwreck keeps the latch; it is read only while thrown
        let latch = if got.0 == THROWN { u32::from(c.landed) } else { got.2 };
        if got != (c.reason, c.clock.to_bits(), latch) {
            b.errors.push(format!("frame {frame}: the original's (reason, clock, latch) {got:?}, the port's ({}, {:#x} = {}, {})", c.reason, c.clock.to_bits(), c.clock, c.landed));
        }
        if let Some(w) = out.wreck {
            b.wrecked.push((frame, w));
        }
        if out.recover {
            b.recovered.push(frame);
        }
        b.clocked += usize::from(c.reason != 0);
    }));
    let s = sim(&ee);
    let v0 = ee.u32_at(VSYNC) as usize;
    let mut j = 0;
    let (mut agreed, mut late) = (0, Vec::new());
    let mut before = ram_words(&ee);
    let mut step = |ee: &mut Ee, j: &mut usize| {
        *j += 1;
        beside.borrow_mut().frame = *j;
        feed(ee, &ticks, *j);
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(s)], &[f32::from_bits(DT)], BUDGET);
    };
    for (r, row) in rows.iter().enumerate() {
        let at = (row["vsync"].parse::<usize>().unwrap() - v0) / 2;
        while j < at {
            before = ram_words(&ee);
            step(&mut ee, &mut j);
        }
        let want = row_words(row);
        if j > at && want == before {
            agreed += 1;
            continue;
        }
        if want == ram_words(&ee) {
            agreed += 1;
            continue;
        }
        // a row read late shows the frame after its own
        let mine = ram_words(&ee);
        before = mine.clone();
        step(&mut ee, &mut j);
        let differ = mine.iter().zip(&want).filter(|(a, b)| a != b).count();
        assert_eq!(want, ram_words(&ee), "row {r} (frame {at}): {differ} words differ from its frame, and it is not the next");
        late.push(r);
    }
    let b = beside.borrow();
    println!(
        "biplane3crash: {agreed} rows agree, late {late:?}; the port's wrecks {:?}, recoveries {:?}, {} frames wrecked",
        b.wrecked, b.recovered, b.clocked
    );
    assert!(b.errors.is_empty(), "{} frames differ:\n  {}", b.errors.len(), b.errors.iter().take(20).cloned().collect::<Vec<_>>().join("\n  "));
    assert!(late.len() <= rows.len() / 5, "{} late rows of {}: re-record it", late.len(), rows.len());
    // the capture's one crash, and its recovery past MaxWreckTime
    assert_eq!(b.wrecked.len(), 1, "the port's wrecks {:?}", b.wrecked);
    assert_eq!(b.wrecked[0].1, 1, "the capture's wreck is a crash");
    assert_eq!(b.recovered.len(), 1, "the port's recoveries {:?}", b.recovered);
    assert!(b.recovered[0] > b.wrecked[0].0 + 140, "recovered {} frames after the wreck", b.recovered[0] - b.wrecked[0].0);
}

// ---------------------------------------------------------------------------------------------------
// 3 — the crash mask (MD113)
// ---------------------------------------------------------------------------------------------------

/// The pointers to the collision categories' name tables: SLUS-21104's (the port's
/// [`game_sim::handling::update::CATEGORY_NAMES`]) and MX Unleashed's (read by its plane's loader).
const MXU_CATEGORY_NAMES: u32 = 0x0041_db50;
/// The collidables' vtables in biplane3's image (at `+0xc`, their second at `+0x1c`, `+0x94` the class;
/// a moving one's `+0x20` the world it is in, 0 when out: the static scenery's is 0 too).
const COLLIDABLE_VTABLES: std::ops::Range<u32> = 0x003b_0000..0x003d_0000;

/// A name table's names in bit order: bit `i` is `1 << i` (game_sim::handling::update::category).
fn category_names(ee: &Ee, pointer: u32) -> Vec<String> {
    let table = ee.u32_at(pointer);
    (0..ee.u32_at(table))
        .map(|i| {
            let at = table + 4 + 32 * i;
            (0..32).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(char::from).collect()
        })
        .collect()
}

/// A class word of one game's table in another's, by name.
fn translate(class: u32, from: &[String], to: &[String]) -> u32 {
    (0..32).filter(|i| class & 1 << i != 0).fold(0, |w, i| {
        let name = from.get(i as usize).unwrap_or_else(|| panic!("class bit {i} of {class:#x} has no name"));
        let j = to.iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} is not in the other table"));
        w | 1 << j
    })
}

/// The plane's crash mask (`+0x30c`) is MXVehicle's category("VEHICLE"), and the struck object's class
/// (`+0x94`) is a collidable's category word. Categories are bits by a name's place in a table built as
/// the game asks for them, so the same name has another bit in each game: SLUS-21104's VEHICLE is 0x4,
/// MX Unleashed's (the port's [`flight_search::VEHICLE`]) 0x2000. Every collidable in biplane3's world
/// struck by the plane, through the original crash test, wrecks it exactly when its class in MX
/// Unleashed's names misses the port's VEHICLE; and the port's own categories are those classes by name.
#[test]
fn the_mxvatv_crash_mask_is_the_ports_vehicle_category() {
    use game_sim::flight_search;
    let Some(mut ee) = image() else { return };
    let kit = Kit::load().unwrap();
    let path = kit.ctx("states").join(format!("biplane.p2m2_SaveState.eeMemory{}", ".bin"));
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return;
    }
    let mxu = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    let atv_names = category_names(&ee, game_sim::handling::update::CATEGORY_NAMES);
    let mxu_names = category_names(&mxu, MXU_CATEGORY_NAMES);
    let bit = |names: &[String], name: &str| 1u32 << names.iter().position(|n| n == name).unwrap_or_else(|| panic!("no {name}"));
    let to_port = |class: u32| translate(class, &atv_names, &mxu_names);

    // the mask: VEHICLE by name in both games, the port's VEHICLE in MX Unleashed's bits
    assert_eq!(ee.u32_at(PLANE + at::MASK), bit(&atv_names, "VEHICLE"), "the plane's mask");
    assert_eq!(bit(&mxu_names, "VEHICLE"), flight_search::VEHICLE, "MX Unleashed's VEHICLE");
    assert_eq!(to_port(ee.u32_at(PLANE + at::MASK)), flight_search::VEHICLE);

    // the image's collidables
    let mut world: Vec<(u32, u32)> = Vec::new();
    for a in (0x0010_0000..0x01ff_ff00u32).step_by(4) {
        let vt = ee.u32_at(a + 0xc);
        if COLLIDABLE_VTABLES.contains(&vt) && ee.u32_at(a + 0x1c) == vt + 0x10 {
            world.push((a, ee.u32_at(a + at::CLASS)));
        }
    }
    let plane_mesh = ee.u32_at(PLANE + 0xd0);
    let classes: std::collections::BTreeSet<u32> = world.iter().map(|&(_, c)| c).collect();
    println!("biplane3's image: {} collidables, classes {:x?}", world.len(), classes);
    let class_of = |a: u32| world.iter().find(|&&(o, _)| o == a).map(|&(_, c)| c);
    // the port's categories are the same names in MX Unleashed's bits: the plane's collidable, the
    // scenery's meshes (STATICOBJECT | CAMERACOLLISION) and the foliage (SOLIDFOLIAGE: the port's trunks)
    assert_eq!(class_of(plane_mesh).map(to_port), Some(game_sim::flight::COLLIDABLE), "the plane's collidable");
    assert!(classes.contains(&(bit(&atv_names, "STATICOBJECT") | bit(&atv_names, "CAMERACOLLISION"))));
    assert_eq!(to_port(bit(&atv_names, "STATICOBJECT") | bit(&atv_names, "CAMERACOLLISION")), flight_search::SCENERY);
    assert_eq!(to_port(bit(&atv_names, "SOLIDFOLIAGE")), flight_search::TREE);
    // the only VEHICLE in the world is the plane itself: the second plane's is out of it
    let in_world = |a: u32| ee.u32_at(a + 0x20) != 0;
    let vehicles: Vec<(u32, bool)> = world.iter().filter(|&&(_, c)| c & ee.u32_at(PLANE + at::MASK) != 0).map(|&(a, _)| (a, in_world(a))).collect();
    assert_eq!(vehicles.iter().filter(|v| v.1).map(|v| v.0).collect::<Vec<_>>(), [plane_mesh], "VEHICLE collidables in biplane3's world: {vehicles:x?}");

    // each class struck, flying fast and level: the original wrecks unless the port's rule spares it
    ee.hooks.insert(RAND, Box::new(|ee| ee.set(2, 0)));
    ee.hooks.insert(POST_EVENT, Box::new(|_| {}));
    ee.hooks.insert(UNWRECK, Box::new(|_| {}));
    ee.hooks.insert(RUMBLE_ON, Box::new(|ee| ee.set(2, 0)));
    let record = ee.u32_at(PLANE + at::RECORD);
    // a struck class of every collidable, and every single bit of either table
    let struck: Vec<u32> = classes.iter().copied().chain((0..atv_names.len()).map(|i| 1 << i)).collect();
    // (each case sets every word the test reads; the wreck entry's writes are not among them)
    let (mut spared, mut wrecked) = (0, 0);
    for class in struck {
        let thing = world.iter().find(|&&(_, c)| c == class).map_or(0x01f8_0100, |&(a, _)| a);
        ee.set_u32(thing + at::CLASS, class);
        ee.set_u32(PLANE + at::REASON, 0);
        ee.set_u32(record + at::TOUCHING, 1);
        ee.set_u32(record + at::STRUCK, thing);
        ee.set_f32(PLANE + at::SPEED, 150.0);
        ee.set_f32(PLANE + at::UP_Y, 1.0);
        ee.call(CRASH_TEST, &[u64::from(PLANE)], &[f32::from_bits(DT)], CALL_BUDGET);
        let port_spares = to_port(class) & flight_search::VEHICLE != 0;
        let wrecks = ee.u32_at(PLANE + at::REASON) == 1;
        assert_eq!(wrecks, !port_spares, "struck class {class:#x} (the port's {:#x})", to_port(class));
        spared += usize::from(port_spares);
        wrecked += usize::from(wrecks);
    }
    println!("the crash mask: {spared} classes spared, {wrecked} wreck");
    assert!(spared >= 2 && wrecked >= 10, "the classes miss a side");
}
