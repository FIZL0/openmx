//! S278b — MX vs. ATV's rider mode machine ([`rider::modes`]) against SLUS-21104's own, off state 4's
//! rider (research/oracle.txt `# S278b`).
//!
//! The original's machine runs whole in the interpreter with its outside calls hooked to log and do
//! nothing (rand answers a drawn word): the riding reset, the wreck entry, the rider's pose and settle
//! virtuals, the hold parts' and the aim force's on/off virtuals, and the wreck arm's nearby gather.
//! The wreck arm's ground query (on Nat05's map), its release and its bodies' vt `+0x68` run as they
//! are (S285). The port runs from the same words with a [`rider::Rider`] that logs the same way and
//! serves the bodies' vt `+0x68` with [`rider::body_target`]. Every word either side writes, outside
//! the stack and the ground query's ring, comes out the same, and the calls are the same, in the same
//! order, with the same arguments. Skips without the image or the map.

use game_sim::handling::{ground, node};
use game_sim::{atv_ground, track};
use ps2_float::Vf;
use game_sim::handling::rider::{self, Call};
use ps2_float::F;
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::rc::Rc;

#[path = "mxvatv_bike_ride/gather_in_ram.rs"]
mod gather_in_ram;
use gather_in_ram::gather_in_ram;

const DT: u32 = 0x3d08_850a;
const GP: u64 = 0x003d_baf0;
const BUDGET: u64 = 50_000_000;
/// State 4's rider (the tail's third leaf) and its mode machine.
const RIDER: u32 = 0x010a_f350;
const MODES: u32 = 0x004e_f320;
/// The machine's outside calls: rand, the riding reset, the wreck entry, the rider's vt `+0xd0`
/// (pose) and `+0xcc` (settle).
const RAND: u32 = 0x0035_61a8;
const RIDING_RESET: u32 = 0x004e_ef40;
const WRECK_ENTRY: u32 = 0x004e_d700;
const POSE: u32 = 0x0050_5720;
const SETTLE: u32 = 0x0050_5710;
/// The wreck arm's nearby gather; the ground query's ring of read cells (0x70 bytes, 32 keys) and the
/// terrain's two words that keep it.
const GATHER: u32 = 0x001d_6a60;
/// S289: the on/off virtuals every part's vtable (`+0x10`) and the aim force's (`+8`) hold at `+0x10`.
const PART_SWITCH: u32 = 0x0049_bd30;
const FORCE_SWITCH: u32 = 0x0049_bd80;
const RING: (u32, u32) = (0x0054_1550, 0x0054_1550 + 0x70 * 32);
/// The rider's bodies' vt `+0x68` (vtable `0x003cd200`), run as it is.
const BODY_TARGET: u32 = 0x0049_3b00;
/// The fake points the record's pointers aim at (free RAM past F39's fake world).
const FAKE: u32 = 0x01c0_a000;
/// The wreck entry's calls beyond the machine's: the heap's reallocate (answered with [`WRECK_GROWN`]),
/// the rider's `+0x8f4` object's cue, and the bodies' vt `+0x18` (mass) and `+0x60` (inertia), run
/// as they are.
const REALLOC: u32 = 0x001b_7470;
const WRECK_GROWN: u32 = FAKE + 0x8000;
const WRECK_CUE: u32 = 0x004f_7ca0;
const BODY_MASS: u32 = 0x0049_3cc0;
const BODY_INERTIA: u32 = 0x0049_3bf0;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip("no context/mxvatv-ps2/state04 RAM image (F31's recipe: research/journal)");
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap();
    // The kernel's DIntr/EIntr, and every `addiu $3, $zero, n; syscall; jr $ra` (the answer a0).
    ee.hooks.insert(0x0034_8fa0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0034_8ff0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    Some(ee)
}

struct Gen(u64);

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.bits() as f32 / 4294967296.0)
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    /// `at` one time in five, else anywhere in `lo..hi`; a zero comes signed either way.
    fn near(&mut self, at: f32, lo: f32, hi: f32) -> f32 {
        let x = if self.one_in(5) { at } else { self.f(lo, hi) };
        if x == 0.0 && self.one_in(2) { -0.0 } else { x }
    }
    fn pick(&mut self, xs: &[u32]) -> u32 {
        xs[(self.bits() % xs.len() as u32) as usize]
    }
}

type Log = Rc<RefCell<Vec<Call>>>;

/// The port's side: the interpreter's RAM, the calls logged, rand's answers handed back in turn.
struct Port<'a> {
    ee: &'a mut Ee,
    log: Log,
    rands: VecDeque<u32>,
}

impl node::Ram for Port<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w)
    }
}

impl rider::Rider for Port<'_> {
    fn call(&mut self, c: Call, dt: F) -> u32 {
        assert_eq!(dt.0.to_bits(), DT, "{c:?}: dt");
        // The bodies' vt +0x68 (and the wreck entry's +0x18 and +0x60) run as they are in the
        // original, unlogged; their writes are compared.
        if !matches!(c, Call::BodyTarget { .. } | Call::BodyMass { .. } | Call::BodyInertia { .. }) {
            self.log.borrow_mut().push(c);
        }
        match c {
            Call::Rand => self.rands.pop_front().expect("rand asked once more than the original did"),
            Call::BodyTarget { body, from } => {
                rider::body_target(self, body, from);
                0
            }
            Call::BodyMass { body, mass } => {
                rider::body_mass(self, body, mass);
                0
            }
            Call::BodyInertia { body, from } => {
                rider::body_inertia(self, body, from);
                0
            }
            Call::Grow { .. } => WRECK_GROWN,
            Call::Switch { part, on } => {
                rider::switch_on(self, RIDER, part, on);
                0
            }
            Call::Gather { list, probe, kind } if GATHERS.get() => {
                gather_in_ram(self.ee, list, probe, kind);
                0
            }
            _ => 0,
        }
    }
}

/// The original's calls hooked: each checks the rider is `a0` (and `dt` in `f12`, the second argument,
/// where it is handed on) and logs; rand answers from `answers`.
fn hook_calls(ee: &mut Ee, log: &Log, answers: &Rc<RefCell<VecDeque<u32>>>, switches: &BTreeSet<u32>) {
    let rider_in = |ee: &Ee, what: &str| assert_eq!(ee.r(4) as u32, RIDER, "{what}: a0");
    let dt_in = |ee: &Ee, what: &str| assert_eq!(ee.f(12).to_bits(), DT, "{what}: f12");
    let l = log.clone();
    let a = answers.clone();
    ee.hooks.insert(RAND, Box::new(move |ee: &mut Ee| {
        l.borrow_mut().push(Call::Rand);
        let v = a.borrow_mut().pop_front().unwrap();
        ee.set(2, u64::from(v));
    }));
    let l = log.clone();
    ee.hooks.insert(RIDING_RESET, Box::new(move |ee: &mut Ee| {
        rider_in(ee, "riding reset");
        dt_in(ee, "riding reset");
        l.borrow_mut().push(Call::RidingReset);
    }));
    let l = log.clone();
    ee.hooks.insert(WRECK_ENTRY, Box::new(move |ee: &mut Ee| {
        rider_in(ee, "wreck entry");
        dt_in(ee, "wreck entry");
        l.borrow_mut().push(Call::WreckEntry(ee.r(5) as u32));
    }));
    let l = log.clone();
    ee.hooks.insert(POSE, Box::new(move |ee: &mut Ee| {
        rider_in(ee, "pose");
        dt_in(ee, "pose");
        assert_eq!(ee.r(5), 0, "pose: a1");
        l.borrow_mut().push(Call::Pose);
    }));
    let l = log.clone();
    ee.hooks.insert(SETTLE, Box::new(move |ee: &mut Ee| {
        rider_in(ee, "settle");
        l.borrow_mut().push(Call::Settle);
    }));
    // S289: the switches and the gather logged, then run as they are.
    for &at in switches {
        assert!([PART_SWITCH, FORCE_SWITCH].contains(&at), "an on/off virtual at {at:#x}: not the parts' or the force's");
        let l = log.clone();
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            l.borrow_mut().push(Call::Switch { part: ee.r(4) as u32, on: ee.r(5) as u32 });
            run_as_is(ee, at);
        }));
    }
    let l = log.clone();
    ee.hooks.insert(GATHER, Box::new(move |ee: &mut Ee| {
        l.borrow_mut().push(Call::Gather { list: ee.r(4) as u32, probe: ee.r(5) as u32, kind: ee.r(6) as u32 });
        if GATHERS.get() {
            run_as_is(ee, GATHER);
        }
    }));
}

thread_local! {
    /// S289: whether the gather runs, on both sides (as it is in the original, [`gather_in_ram`] in
    /// the port's), or on neither: a gather reads state 4's whole broad phase out of RAM in the
    /// port's, ~9 ms, so the mode machine's test runs it in one case in eight.
    static GATHERS: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
}

/// S289: inside its own hook, the hooked function run in the interpreter after all (a nested call
/// with the hook out of the way, on the stack below the caller's): its arguments as they are, the
/// caller's `sp`/`ra`, the written set and the hooked `pc` put back so the hook fires again.
fn run_as_is(ee: &mut Ee, at: u32) {
    let (sp, ra, stack) = (ee.r(29), ee.r(31), ee.stack);
    let args: Vec<u64> = (4..8).map(|i| ee.r(i)).collect();
    let floats: Vec<f32> = (12..14).map(|i| ee.f(i)).collect();
    let written = std::mem::take(&mut ee.written);
    ee.stack = (sp as u32 - 0x400) & !0xf;
    ee.call(at, &args, &floats, BUDGET);
    ee.written.extend(written);
    ee.set(29, sp);
    ee.set(31, ra);
    (ee.stack, ee.pc) = (stack, at);
}

/// Nat05's terrain map off its `.res` (state 4 rides Nat05), for the port's ground query.
fn nat05_trn() -> Option<Vec<u8>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let res = kit.ctx("mxvatv-ps2/nat05").join("nat05.res");
    if !res.exists() {
        skip(&format!("{} is not there: Nat05's nat05.res off MX vs. ATV's disc (Env/Nat05)", res.display()));
        return None;
    }
    ground::terrain_file(&std::fs::read(&res).unwrap(), "Nat05")
}

fn word_of(ram: &[u8], at: u32) -> u32 {
    let i = at as usize;
    u32::from_le_bytes([ram[i], ram[i + 1], ram[i + 2], ram[i + 3]])
}

/// The words written, outside the stack and the ground query's ring (with the terrain's two words
/// that keep it): the port keeps no ring.
fn written_words(ee: &Ee, terrain: u32) -> BTreeSet<u32> {
    let stack = ee.stack;
    ee.written
        .iter()
        .map(|&a| a & !3)
        .filter(|a| !(stack - 0x4_0000..stack + 0x1000).contains(a))
        .filter(|a| !(RING.0..RING.1).contains(a) && *a != terrain + 0x9e0 && *a != terrain + 0x9e4)
        .collect()
}

/// **The rider's mode machine is the original's**: 60 000 frames of state 4's rider, each with the
/// words the machine reads drawn at random — the record's respawn ask, wreck kind (13 among them),
/// hold ask and ridden word; the rider's acted mode, last kind, respawn count, `+0x3a4`, `+0x86d` /
/// `+0x86e` bytes, holds' fades, the follow's words; the record's orientation, points and pairs (none,
/// one or both pointers), its air and level words and the lean inputs on both sides of each limit,
/// signed zeros among them. Every word written comes out the same on both sides, and the same calls
/// are made in the same order. S289: the parts' and the aim force's on/off virtuals run on both sides
/// (as they are, and [`rider::switch_on`]), and so does the wreck arm's nearby gather in one case in
/// eight (as it is, and [`gather_in_ram`]), the probe's ignore list sometimes naming one it finds.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_riders_mode_machine_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let Some(trn) = nat05_trn() else { return };
    let map = track::Terrain::parse(&trn).unwrap();
    let rec = ee.u32_at(RIDER + rider::RECORD);
    let holder = ee.u32_at(RIDER + rider::MAIN);
    let pose_slot = ee.u32_at(RIDER + 0x1fc).wrapping_add(ee.u32_at(holder + 0x40) << 6).wrapping_add(0x20);
    let follow = ee.u32_at(RIDER + 0x30c);
    let holds = [rider::HOLD_L, rider::HOLD_R, rider::FREE_L, rider::FREE_R, rider::LET_GO[0], rider::LET_GO[1]]
        .map(|o| ee.u32_at(RIDER + o));
    let mut switches: BTreeSet<u32> = holds.iter().map(|&p| ee.u32_at(ee.u32_at(p + 0x10) + 0x10)).collect();
    // The wreck arm's aim force: its vtable at +8.
    let force = ee.u32_at(RIDER + rider::AIM_FORCE);
    switches.insert(ee.u32_at(ee.u32_at(force + 8) + 0x10));
    // The wreck arm's words.
    let terrain = ee.u32_at(RIDER + rider::TERRAIN);
    let body = ee.u32_at(holder + 0xa4);
    let bodies = ee.u32_at(RIDER + rider::BODIES);
    let poses = ee.u32_at(RIDER + rider::POSES);
    let parts: Vec<u32> = (0..ee.u32_at(RIDER + rider::PARTS_COUNT))
        .map(|i| ee.u32_at(ee.u32_at(RIDER + rider::PARTS) + 4 * i))
        .collect();
    for i in 0..bodies {
        let b = ee.u32_at(ee.u32_at(RIDER + rider::MAIN) + 0xb0 * i + 0xa4);
        assert_eq!(ee.u32_at(ee.u32_at(b + 0x110) + 0x68), BODY_TARGET, "body {i}'s vt +0x68");
    }
    let home = [0, 4, 8].map(|k| f32::from_bits(ee.u32_at(body + 0x70 + k)));
    let ground_at = |x: f32, z: f32| {
        let t = ground::read_terrain(&|a: u32| ee.u32_at(a), terrain, Some(&map));
        let mut p = Vf::new(x, 0.0, z, 1.0);
        atv_ground::query(&t, &mut p, None, None);
        p.y()
    };
    let mut grounds = Vec::new();
    {
        let mut g = Gen(0x0285_0001);
        for _ in 0..64 {
            let (x, z) = (home[0] + g.f(-40.0, 40.0), home[2] + g.f(-40.0, 40.0));
            grounds.push((x, z, ground_at(x, z)));
        }
    }
    eprintln!("terrain {terrain:#x}, main body {body:#x}, {bodies} bodies, pose list {poses:#x}, {} parts", parts.len());
    let real_parts = ee.u32_at(RIDER + 0x3ac);
    eprintln!("record {rec:#x}, holder {holder:#x}, switch fns {switches:#x?}, pose list {real_parts}");
    // The gather's list and probe: the first part's sphere.
    let nearby = ee.u32_at(RIDER + rider::NEARBY);
    let probe_sphere = ee.u32_at(parts[0] + 0x88);
    // What it finds on the image, run once as it is: the collidables (entry - 0x18) the probe's
    // ignore list is sometimes given, so the pair filter leaves one out.
    let found: Vec<u32> = {
        let mut e = image().unwrap();
        e.set(28, GP);
        e.call(GATHER, &[u64::from(nearby), u64::from(probe_sphere), 3], &[], BUDGET);
        let (arr, n) = (e.u32_at(nearby), e.u32_at(nearby + 4));
        (0..n).map(|i| e.u32_at(arr + 8 * i) - 0x18).collect()
    };
    assert!(!found.is_empty(), "the gather finds nothing round the rider on the image");
    eprintln!("gather list {nearby:#x}, probe {probe_sphere:#x}: {} found", found.len());
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let answers = Rc::new(RefCell::new(VecDeque::new()));
    hook_calls(&mut ee, &log, &answers, &switches);
    let snapshot = ee.ram.clone();

    let mut g = Gen(0x0278_b001);
    let mut t: BTreeMap<String, u32> = BTreeMap::new();
    let mut tally = |path: String| *t.entry(path).or_insert(0) += 1;
    for k in 0..60_000 {
        // The inputs, poked over the image.
        let mut given: Vec<(u32, u32)> = Vec::new();
        let mut put = |ee: &mut Ee, at: u32, v: u32| {
            ee.set_u32(at, v);
            given.push((at, v));
        };
        let f = |x: f32| x.to_bits();
        let respawn_ask = if g.one_in(6) { g.bits() | 1 } else { 0 };
        put(&mut ee, rec + rider::R_RESPAWN, respawn_ask);
        let kind = if g.one_in(2) { 0 } else { g.pick(&[13, 13, 1, 2, 5, 0x40]) };
        put(&mut ee, rec + rider::R_KIND, kind);
        put(&mut ee, rec + rider::R_HOLD, if g.one_in(3) { 1 } else { 0 });
        put(&mut ee, rec + rider::R_RIDDEN, if g.one_in(2) { 1 } else { 0 });
        put(&mut ee, RIDER + rider::ACTED, g.bits() % 5);
        put(&mut ee, RIDER + rider::KIND_KEPT, g.pick(&[0, 13, 2]));
        put(&mut ee, RIDER + rider::RESPAWN, g.pick(&[0, 0, 0, 0, 1, 2, 3, 0xffff_ffff]));
        put(&mut ee, RIDER + 0x3a4, g.pick(&[0, 1, 2, 0xffff_ffff]));
        let w86c = ee.u32_at(RIDER + 0x86c);
        let b86d = g.pick(&[45, 46, 0, 0xff, 0x60]);
        let b86e = g.pick(&[0x38, 0x38, 0x37, 0]);
        put(&mut ee, RIDER + 0x86c, (w86c & 0xff00_00ff) | (b86d << 8) | (b86e << 16));
        put(&mut ee, RIDER + 0x8ac, if g.one_in(3) { g.bits() | 1 } else { 0 });
        put(&mut ee, RIDER + 0x874, g.pick(&[0xffff_ffff, 0xffff_ffff, 0xffff_ffff, 0, 3]));
        let over = g.near(0.0, 0.5, 20.0);
        put(&mut ee, RIDER + 0x894, f(over));
        let low = g.near(0.0, 0.5, 20.0);
        put(&mut ee, RIDER + 0x8a0, f(low));
        for o in [rider::FADE_L, rider::FADE_R] {
            let v = if g.one_in(3) { g.pick(&[0, 0x3f00_0000, 0x4000_0000]) } else { f(g.near(0.0, -0.5, 2.5)) };
            put(&mut ee, RIDER + o, v);
        }
        for o in [0x258, 0x38c, 0x384, 0x388] {
            put(&mut ee, RIDER + o, if g.one_in(2) { 1 } else { 0 });
        }
        put(&mut ee, follow + 0x28, g.pick(&[2, 2, 1, 0]));
        for o in [0x600, 0x410, 0x400] {
            for i in 0..4 {
                let x = g.near(0.0, -3.0, 3.0);
                put(&mut ee, RIDER + o + 4 * i, f(x));
            }
        }
        // The wreck arm's: the aim and its force, the let-go, the touched parts, the bike's
        // collidable and the spheres' ignore lists, the main body over the ground, the clauses'
        // limits and latches, the pose list and the fade.
        let clock = |g: &mut Gen| match g.bits() % 4 {
            0 => g.pick(&[0, 0, 0x8000_0000, DT]),
            _ => f(g.near(0.0, -0.1, 0.2)),
        };
        let aim_t = clock(&mut g);
        put(&mut ee, RIDER + rider::AIM_TIME, aim_t);
        let let_t = clock(&mut g);
        put(&mut ee, RIDER + rider::LET_GO_TIME, let_t);
        put(&mut ee, RIDER + rider::AIM_AS_IS, if g.one_in(3) { 1 } else { 0 });
        put(&mut ee, RIDER + rider::AIM_BY_BODY, if g.one_in(2) { 1 } else { 0 });
        for i in 0..4 {
            let x = g.near(0.0, -2.0, 2.0);
            put(&mut ee, RIDER + rider::AIM_FROM + 4 * i, f(x));
        }
        put(&mut ee, force, if g.one_in(2) { 1 } else { 0 });
        for o in rider::LET_GO {
            let p = ee.u32_at(RIDER + o);
            put(&mut ee, p + 4, if g.one_in(2) { 1 } else { 0 });
        }
        let few = g.one_in(2);
        for &p in &parts {
            put(&mut ee, p + rider::TOUCHED, if few && g.one_in(6) { 1 } else { 0 });
        }
        let old_t = if g.one_in(3) { 0.0 } else { g.near(0.75, 0.0, 1.5) };
        put(&mut ee, RIDER + rider::TOUCHING_TIME, f(old_t));
        let (x, z, gy) = grounds[g.bits() as usize % grounds.len()];
        let high_at = g.near(3.0, 0.5, 5.0);
        put(&mut ee, RIDER + rider::HIGH_AT, f(high_at));
        let y = if g.one_in(5) { gy + high_at } else { gy + g.f(-1.0, 8.0) };
        for (i, v) in [x, y, z, 1.0].into_iter().enumerate() {
            put(&mut ee, body + 0x70 + 4 * i as u32, f(v));
        }
        let under = g.near(0.0, 0.0, 3.0);
        put(&mut ee, RIDER + rider::SLOW_UNDER, f(under));
        put(&mut ee, RIDER + rider::SLOW_AFTER, f(g.near(0.75, 0.0, 1.5)));
        let sp = g.f(0.0, 4.0);
        for i in 0..4 {
            put(&mut ee, body + 0x80 + 4 * i, f(sp * g.f(-1.0, 1.0)));
        }
        put(&mut ee, RIDER + rider::HIGH, if g.one_in(2) { 1 } else { 0 });
        put(&mut ee, RIDER + rider::SLOW, if g.one_in(2) { 1 } else { 0 });
        put(&mut ee, RIDER + rider::RELEASED, if g.one_in(2) { 1 } else { 0 });
        let collidable = FAKE + 0x1000;
        let c = if g.one_in(4) { 0 } else { collidable };
        put(&mut ee, rec + rider::R_COLLIDABLE, c);
        let radius = g.f(0.5, 3.0);
        put(&mut ee, collidable + 0x70, f(radius));
        let reach = if g.one_in(5) { radius + 1.0 } else { g.f(0.0, 2.0 * (radius + 1.0)) };
        let (dx, dy) = (g.f(-1.0, 1.0), g.f(-1.0, 1.0));
        let dz = (1.0 - dx * dx - dy * dy).max(0.0).sqrt();
        for (i, v) in [x - dx * reach, y - dy * reach, z - dz * reach, 1.0].into_iter().enumerate() {
            put(&mut ee, collidable + 0x50 + 4 * i as u32, f(v));
        }
        for (k, &p) in parts.iter().enumerate() {
            let sphere = ee.u32_at(p + 0x88);
            let list = FAKE + 0x2000 + 0x10 * k as u32;
            put(&mut ee, sphere + 0xe4, list);
            put(&mut ee, sphere + 0xe8, g.pick(&[0, 1, 2, 3, 4, 0xffff_ffff]));
            for i in 0..4 {
                let w = if k == 0 && g.one_in(3) { found[g.bits() as usize % found.len()] } else { g.pick(&[collidable, collidable, 0, FAKE + 0x1100]) };
                put(&mut ee, list + 4 * i, w);
            }
        }
        for i in 0..bodies {
            for o in [0x40, 0x50] {
                for k in 0..4 {
                    let x = g.near(0.0, -9.0, 9.0);
                    put(&mut ee, poses + 0x60 * i + o + 4 * k, f(x));
                }
            }
        }
        for k in 0..4 {
            let x = g.near(0.0, -9.0, 9.0);
            put(&mut ee, RIDER + rider::TARGET + 4 * k, f(x));
        }
        put(&mut ee, RIDER + rider::FADE, f(g.near(0.0, -0.1, 0.5)));
        put(&mut ee, RIDER + rider::FADE_FROM, f(g.near(0.0, 0.0, 2.0)));
        put(&mut ee, RIDER + rider::TARGETED, if g.one_in(2) { 1 } else { 0 });
        // The record's orientation (and the reference pose's), its points and pairs.
        for at in [rec + 0x1c0, pose_slot] {
            for i in 0..4 {
                let x = g.near(0.0, -1.0, 1.0);
                put(&mut ee, at + 4 * i, f(x));
            }
        }
        for o in [0x1b0, 0xa0, 0xd0, 0xc0, 0xf0, 0xe0, 0x60, 0x70, 0x80, 0x90] {
            for i in 0..4 {
                let x = g.near(0.0, -50.0, 50.0);
                put(&mut ee, rec + o + 4 * i, f(x));
            }
        }
        for p in 0..10 {
            let at = FAKE + 0x10 * p;
            for i in 0..4 {
                let x = g.near(0.0, -9.0, 9.0);
                put(&mut ee, at + 4 * i, f(x));
            }
            // +0x100 is read unchecked by the respawn's turned point: never none.
            let none = p != 0 && g.one_in(4);
            put(&mut ee, rec + 0x100 + 4 * p, if none { 0 } else { at + if g.one_in(4) { 4 } else { 0 } });
        }
        put(&mut ee, rec + 0x130, if g.one_in(3) { 1 } else { 0 });
        let level = if g.one_in(2) { 0 } else { f(g.near(0.0, -1.0, 1.0)) };
        put(&mut ee, rec + 0x19c, level);
        let a8 = if g.one_in(4) { [over, low][g.bits() as usize % 2] } else { g.near(0.0, -2.0, 20.0) };
        put(&mut ee, rec + 0x1a8, f(a8));
        let b4 = if g.one_in(5) { [1.5, -1.5][g.bits() as usize % 2] } else { g.near(0.0, -3.0, 3.0) };
        put(&mut ee, rec + 0xb4, f(b4));
        let a0 = if g.one_in(4) { [0.7, 0.3][g.bits() as usize % 2] } else { g.near(0.5, -1.0, 1.0) };
        put(&mut ee, rec + 0x1a0, f(a0));
        put(&mut ee, rec + 0x12c, if g.one_in(2) { 1 } else { 0 });
        put(&mut ee, RIDER + 0x3ac, g.pick(&[real_parts, real_parts, real_parts, 0, 0xffff_ffff]));
        let rands: Vec<u32> = (0..4).map(|_| g.bits()).collect();
        let gathers = g.one_in(8);
        GATHERS.set(gathers);

        // The original.
        log.borrow_mut().clear();
        *answers.borrow_mut() = rands.iter().copied().collect();
        ee.set(28, GP);
        ee.call(MODES, &[u64::from(RIDER)], &[f32::from_bits(DT)], BUDGET);
        let w1 = written_words(&ee, terrain);
        let want: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
        let calls = log.borrow().clone();
        let mode = ee.u32_at(RIDER + rider::MODE);

        // Back to the inputs, then the port.
        for &a in &w1 {
            ee.set_u32(a, word_of(&snapshot, a));
        }
        for &(a, v) in &given {
            ee.set_u32(a, v);
        }
        let pre: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
        log.borrow_mut().clear();
        ee.written.clear();
        let mut port = Port { ee: &mut ee, log: log.clone(), rands: rands.iter().copied().collect() };
        rider::modes(&mut port, RIDER, F(f32::from_bits(DT)), Some(&map));
        let w2 = written_words(&ee, terrain);
        let got_calls = log.borrow().clone();
        assert_eq!(got_calls, calls, "case {k}: the calls");
        for &a in w1.union(&w2) {
            let w = want.get(&a).copied().unwrap_or_else(|| pre.get(&a).copied().unwrap_or_else(|| ee.u32_at(a)));
            let got = ee.u32_at(a);
            let before = pre.get(&a).copied().unwrap_or(word_of(&snapshot, a));
            assert_eq!(got, w, "case {k} (mode {mode}, calls {calls:?}): the word at {a:#x} (was {before:#x}, {})", if w2.contains(&a) { "the port wrote it" } else { "only the original wrote it" });
        }
        for &a in w1.union(&w2).chain(given.iter().map(|(a, _)| a)) {
            ee.set_u32(a, word_of(&snapshot, a));
        }

        let acted = given.iter().find(|(a, _)| *a == RIDER + rider::ACTED).unwrap().1;
        let count = given.iter().find(|(a, _)| *a == RIDER + rider::RESPAWN).unwrap().1 as i32;
        tally(format!("mode {mode}"));
        tally(match () {
            _ if respawn_ask != 0 || count > 0 => "switch: the respawn".into(),
            _ if calls.contains(&Call::Rand) => "switch: into 4".into(),
            _ if calls.iter().any(|c| matches!(c, Call::WreckEntry(_))) => {
                if acted == 2 { "switch: the wreck entered again".into() } else { "switch: into the wreck".into() }
            }
            _ if mode != 2 && acted == 2 => "switch: out of the wreck".into(),
            _ if mode == 3 && acted != 3 => "switch: into 3".into(),
            _ if calls.contains(&Call::RidingReset) => "switch: into 1".into(),
            _ => "switch: none".to_string(),
        });
        // Which holds arm: the parts switched, in order (L/R hold, l/r free), on or off, and whether a
        // fade ran.
        let switched: Vec<String> = calls
            .iter()
            .filter_map(|c| match *c {
                Call::Switch { part, .. } if part == force => Some("F".to_string()),
                Call::Switch { part, on } => {
                    let i = holds.iter().position(|&h| h == part).expect("a part not among the holds");
                    Some(format!("{}{on}", ["L", "R", "l", "r", "A", "B"][i]))
                }
                _ => None,
            })
            .collect();
        let seq: String = switched.iter().filter(|s| "LRlr".contains(&s[..1])).cloned().collect();
        if mode == 2 {
            let arm: String = switched.iter().filter(|s| "FAB".contains(&s[..1])).cloned().collect();
            tally(format!("arm: switched {arm}"));
            let gv = |a: u32| given.iter().rev().find(|(at, _)| *at == a).unwrap().1;
            let now = |a: u32| want.get(&a).copied().unwrap_or_else(|| gv(a));
            if gathers {
                // S289: the gather run on both sides, what it found, and whether the probe's own
                // ignore list (drawn for the release) was in play.
                let n = want.get(&(nearby + 4)).copied().unwrap_or_else(|| ee.u32_at(nearby + 4));
                tally(format!("arm: gathered {}", if n == 0 { "nothing" } else { "some" }));
                let ignores = gv(probe_sphere + 0xe8) as i32;
                let list = gv(probe_sphere + 0xe4);
                let left_out = (0..ignores.max(0) as u32).any(|i| found.contains(&gv(list + 4 * i)));
                tally(format!("arm: gathered, the probe ignoring {}", if left_out { "one it finds" } else if ignores > 0 { "others" } else { "none" }));
            }
            if f32::from_bits(gv(RIDER + rider::AIM_TIME)) > 0.0 {
                let how = if gv(RIDER + rider::AIM_AS_IS) != 0 { "as is" } else if gv(RIDER + rider::AIM_BY_BODY) != 0 { "by body" } else { "by record" };
                tally(format!("arm: aim {how}"));
            }
            if gv(RIDER + rider::RELEASED) == 0 && now(RIDER + rider::RELEASED) == 1 {
                tally("arm: released".into());
                if (0..parts.len() as u32).any(|k| (0..4).any(|i| want.contains_key(&(FAKE + 0x2000 + 0x10 * k + 4 * i)))) {
                    tally("arm: released off a list".into());
                }
            }
            let latch = |a: u32| (gv(RIDER + a), now(RIDER + a));
            match (latch(rider::HIGH), latch(rider::SLOW)) {
                ((0, 1), _) => tally("arm: high set".into()),
                ((1, 0), _) => tally("arm: high cleared".into()),
                (_, (0, 1)) => {
                    let late = f32::from_bits(now(RIDER + rider::TOUCHING_TIME)) > f32::from_bits(gv(RIDER + rider::SLOW_AFTER));
                    tally(if late { "arm: slow set, late".into() } else { "arm: slow set, slow".into() });
                }
                _ => tally("arm: no clause".into()),
            }
            if gv(RIDER + rider::TARGETED) == 0 && now(RIDER + rider::TARGETED) == 1 && gv(rec + rider::R_KIND) == 13 {
                tally("arm: targets set, kind 13".into());
            }
            if f32::from_bits(gv(RIDER + rider::FADE)) > 0.0 && want.contains_key(&(RIDER + rider::FADE)) {
                tally("arm: fade ran".into());
            }
        }
        let fades: Vec<u32> = [rider::FADE_R, rider::FADE_L].iter().map(|o| RIDER + o).collect();
        let faded = fades.iter().any(|a| want.get(a).is_some_and(|&v| v != pre[a]));
        tally(format!("holds: {seq}"));
        if faded {
            tally("holds: a fade ran".into());
        }
        let posed = calls.iter().filter(|c| **c == Call::Pose).count();
        if posed > usize::from(respawn_ask != 0 || count > 0 || (mode != 2 && acted == 2)) {
            tally("follow: posed again".into());
        }
    }
    eprintln!("{t:#?}");
    for path in ["arm: gathered, the probe ignoring one it finds", "arm: gathered, the probe ignoring others", "arm: gathered, the probe ignoring none"] {
        assert!(t.contains_key(path), "path {path} never taken");
    }
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
}


/// The rider's bodies' setters the pose calls (vt `+0x10` position, `+0x14` velocity, `+0x58`
/// orientation and rates) and the commit the settle calls (`+0x3c`): the port writes them as they are.
const BODY_SLOTS: [(u32, u32); 4] = [(0x10, 0x0049_2b90), (0x14, 0x0049_2b70), (0x58, 0x0049_2b30), (0x3c, 0x0049_3070)];

/// A random rotation's three rows (`x`, `y`, `z` axes), host floats: inputs only.
#[allow(clippy::float_arithmetic)]
fn rotation(g: &mut Gen) -> [[f32; 3]; 3] {
    let mut q = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
    let n = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
    q.iter_mut().for_each(|x| *x /= n);
    let [x, y, z, w] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

/// **The rider's pose and settle virtuals are the original's** (S286): 12 000 cases of state 4's
/// rider, each running vt `+0xd0` (0x00505720, `still` and `dt` drawn: the step, 0, −0, negative, a
/// tiny one) and vt `+0xcc` (0x00505710) whole in the interpreter, against [`rider::pose`] and
/// [`rider::settle`] from the same words. Drawn per body holder: its node's world frame (current) or
/// own frame (stale, rebuilt from its parents), with rotations, skewed rows and signed zeros; the
/// drawn frame random, or exactly the node's axes' basis (no turn); the body's centre-of-gravity
/// offset; for the settle every body's staged words. The holder and body counts are drawn too (the
/// real one, 0, −1). Every word either side writes, off the stack, comes out the same.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_riders_pose_and_settle_are_the_originals() {
    use game_sim::math::{Mat4, basis, mat4_to_quat};
    let Some(mut ee) = image() else { return };
    let acos = game_sim::air::AcosTable::build();
    let curve = game_sim::camera::CurveTable::build();
    let bodies = ee.u32_at(RIDER + rider::BODIES);
    let world = ee.u32_at(RIDER + rider::WORLD);
    let world_n = ee.u32_at(world + rider::W_BODIES_COUNT);
    let holder_at = |ee: &Ee, i: u32| ee.u32_at(RIDER + rider::MAIN) + 0xb0 * i;
    let mut all_bodies: BTreeSet<u32> = (0..bodies).map(|i| ee.u32_at(holder_at(&ee, i) + rider::H_BODY)).collect();
    for i in 0..world_n {
        all_bodies.insert(ee.u32_at(ee.u32_at(world + rider::W_BODIES) + 4 * i));
    }
    for &b in &all_bodies {
        for (slot, code) in BODY_SLOTS {
            assert_eq!(ee.u32_at(ee.u32_at(b + 0x110) + slot), code, "body {b:#x}'s vt {slot:#x}");
        }
    }
    eprintln!("{bodies} holders, world {world:#x} with {world_n} bodies, {} bodies in all", all_bodies.len());
    let snapshot = ee.ram.clone();
    let mut g = Gen(0x0286_0001);
    let mut t: BTreeMap<String, u32> = BTreeMap::new();
    let mut tally = |path: String| *t.entry(path).or_insert(0) += 1;
    for k in 0..12_000 {
        let mut given: Vec<(u32, u32)> = Vec::new();
        let mut put = |ee: &mut Ee, at: u32, v: u32| {
            ee.set_u32(at, v);
            given.push((at, v));
        };
        let f = |x: f32| x.to_bits();
        let mut no_turn = Vec::new();
        let mut touched: BTreeSet<u32> = BTreeSet::new();
        let mut flipped = 0;
        let mut prev_flipped = 0;
        for i in 0..bodies {
            let h = holder_at(&ee, i);
            let n = ee.u32_at(h + rider::H_NODE);
            let b = ee.u32_at(h + rider::H_BODY);
            let current = !g.one_in(4);
            let r = rotation(&mut g);
            let mut rows = [[0f32; 4]; 4];
            for a in 0..3 {
                let skew = if g.one_in(8) { g.f(0.5, 2.0) } else { 1.0 };
                for c in 0..3 {
                    let x = r[a][c] * skew + if g.one_in(10) { g.f(-0.2, 0.2) } else { 0.0 };
                    rows[a][c] = if g.one_in(40) { if g.one_in(2) { -0.0 } else { 0.0 } } else { x };
                }
                rows[a][3] = g.pick(&[0, 0, 0x3f80_0000]) as f32;
            }
            rows[3] = [g.near(0.0, -60.0, 60.0), g.near(0.0, -60.0, 60.0), g.near(0.0, -60.0, 60.0), 1.0];
            let frame = if current { n + 0x70 } else { n + 0x30 };
            for (a, row) in rows.iter().enumerate() {
                for (c, &x) in row.iter().enumerate() {
                    put(&mut ee, frame + 0x10 * a as u32 + 4 * c as u32, f(x));
                }
            }
            let flags = ee.u32_at(n + 0x100);
            put(&mut ee, n + 0x100, if current { flags | 1 } else { flags & !1 });
            // The drawn frame: the node's own axes' basis (no turn, when the world is known here), or
            // another rotation.
            let same = current && g.one_in(4);
            let z = Vf::new(rows[2][0], rows[2][1], rows[2][2], 0.0);
            let y = Vf::new(rows[1][0], rows[1][1], rows[1][2], 0.0);
            let drawn = if same {
                no_turn.push(b);
                basis(Vf::new(0.0, 0.0, 0.0, 1.0), z, y, false, true)
            } else {
                let d = rotation(&mut g);
                Mat4([0, 1, 2].map(|a| Vf::new(d[a][0], d[a][1], d[a][2], 0.0)).into_iter().chain([Vf::ZERO]).collect::<Vec<_>>().try_into().unwrap())
            };
            if !same && mat4_to_quat(&drawn).w() < 0.0 {
                prev_flipped += 1;
            }
            if current && mat4_to_quat(&basis(Vf::new(0.0, 0.0, 0.0, 1.0), z, y, false, true)).w() < 0.0 {
                flipped += 1;
            }
            for a in 0..3 {
                for c in 0..4 {
                    put(&mut ee, h + rider::H_DRAWN + 0x10 * a + 4 * c, drawn.0[a as usize].0[c as usize].to_bits());
                }
            }
            for c in 0..4 {
                let x = if c == 3 { g.pick(&[0x3f80_0000, 0]) } else { f(g.near(0.0, -60.0, 60.0)) };
                put(&mut ee, h + rider::H_DRAWN + 0x30 + 4 * c, x);
            }
            for c in 0..4 {
                let x = g.near(0.0, -0.5, 0.5);
                put(&mut ee, b + 0x1a0 + 4 * c, f(x));
            }
        }
        for &b in &all_bodies {
            for o in [0x40, 0x50, 0x160, 0x180, 0x1a0] {
                for c in 0..4 {
                    let x = g.near(0.0, -9.0, 9.0);
                    put(&mut ee, b + o + 4 * c, f(x));
                }
            }
        }
        put(&mut ee, RIDER + rider::BODIES, g.pick(&[bodies, bodies, bodies, bodies, bodies, 0, 0xffff_ffff]));
        put(&mut ee, world + rider::W_BODIES_COUNT, g.pick(&[world_n, world_n, world_n, world_n, world_n, 0, 0xffff_ffff]));
        let dt = match g.bits() % 8 {
            0 => 0.0,
            1 => -0.0,
            2 => -f32::from_bits(DT),
            3 => 1e-6,
            4 => g.f(0.001, 0.1),
            _ => f32::from_bits(DT),
        };
        let still = g.one_in(5);

        for which in ["pose", "settle"] {
            let call = || match which {
                "pose" => (POSE, vec![u64::from(RIDER), u64::from(still)]),
                _ => (SETTLE, vec![u64::from(RIDER)]),
            };
            let pre: HashMap<u32, u32> = given.iter().map(|&(a, _)| (a, ee.u32_at(a))).collect();
            ee.written.clear();
            let (pc, ints) = call();
            ee.set(28, GP);
            ee.call(pc, &ints, &[dt], BUDGET);
            let w1 = written_words(&ee, 0);
            let want: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
            for &a in &w1 {
                ee.set_u32(a, pre.get(&a).copied().unwrap_or_else(|| word_of(&snapshot, a)));
            }
            ee.written.clear();
            let mut port = Port { ee: &mut ee, log: Rc::new(RefCell::new(Vec::new())), rands: VecDeque::new() };
            match which {
                "pose" => rider::pose(&mut port, RIDER, F(dt), still, &acos, &curve),
                _ => rider::settle(&mut port, RIDER),
            }
            let w2 = written_words(&ee, 0);
            touched.extend(w1.iter().chain(&w2));
            for &a in w1.union(&w2) {
                let w = want.get(&a).copied().unwrap_or_else(|| pre.get(&a).copied().unwrap_or_else(|| word_of(&snapshot, a)));
                assert_eq!(ee.u32_at(a), w, "case {k}, {which} (dt {dt}, still {still}): the word at {a:#x} ({})", if w2.contains(&a) { "the port wrote it" } else { "only the original wrote it" });
            }
            if which == "pose" {
                tally(format!("pose: {} words", if w1.is_empty() { "no" } else { "some" }));
                // No turn: the arc cosine's zero, its axis (1, 0, 0) times a rate of 0.
                let zero = |b: u32| (0..3).all(|c| want.get(&(b + 0x160 + 4 * c)) == Some(&0));
                if !still && dt > 0.0 && no_turn.iter().any(|&b| zero(b)) {
                    tally("pose: zero angle".into());
                }
            } else {
                tally(format!("settle: {} words", if w1.is_empty() { "no" } else { "some" }));
            }
            // The pose's writes stay for the settle, as the respawn makes them.
        }
        let moving = !still && dt > 0.0;
        let n_now = ee.u32_at(RIDER + rider::BODIES) as i32;
        if n_now > 0 {
            tally(if moving { "pose: moving".into() } else if still { "pose: still".into() } else { "pose: dt <= 0".into() });
            if moving && !no_turn.is_empty() {
                tally("pose: no turn".into());
            }
            if flipped > 0 {
                tally("pose: q flipped".into());
            }
            if moving && prev_flipped > 0 {
                tally("pose: drawn flipped".into());
            }
        }
        for a in given.iter().map(|(a, _)| *a).chain(touched) {
            ee.set_u32(a, word_of(&snapshot, a));
        }
    }
    eprintln!("{t:#?}");
    for path in ["pose: moving", "pose: still", "pose: dt <= 0", "pose: no turn", "pose: zero angle", "pose: q flipped", "pose: drawn flipped",
        "pose: no words", "settle: some words", "settle: no words"] {
        assert!(t.contains_key(path), "path {path} never taken");
    }
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
}

/// The riding reset's bodies' virtuals (vt `+0x18` mass, `+0x60` inertia, `+0x68` target, `+0x20`
/// position), run as they are; the heap's resize, hooked on both sides to the same fake.
const RESET_BODY_SLOTS: [(u32, u32); 4] = [(0x18, 0x0049_3cc0), (0x60, 0x0049_3bf0), (0x68, BODY_TARGET), (0x20, 0x0049_3b20)];
const HEAP_RESIZE: u32 = 0x001b_7470;
/// Where the fake resize puts the `n`th grown list (`0x40` apart, past the lists the cases draw).
const GROWN: u32 = FAKE + 0x4000;

/// The fake resize: the `n`th call's list at `GROWN + 0x40 n`, the old list's first `bytes − 4`
/// bytes copied in.
fn grow(ee: &mut Ee, n: u32, list: u32, bytes: u32) -> u32 {
    let to = GROWN + 0x40 * n;
    for k in (0..bytes.saturating_sub(4).min(0x3c)).step_by(4) {
        let w = ee.u32_at(list + k);
        ee.set_u32(to + k, w);
    }
    to
}

/// The port's side of the riding reset: the bodies' virtuals served by the ports, unlogged; the heap's
/// resize by [`grow`]; every other call logged.
struct ResetPort<'a> {
    ee: &'a mut Ee,
    log: Log,
    grown: u32,
}

impl node::Ram for ResetPort<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w)
    }
}

impl rider::Rider for ResetPort<'_> {
    fn call(&mut self, c: Call, dt: F) -> u32 {
        assert_eq!(dt.0.to_bits(), DT, "{c:?}: dt");
        match c {
            Call::BodyMass { body, mass } => rider::body_mass(self, body, mass),
            Call::BodyInertia { body, from } => rider::body_inertia(self, body, from),
            Call::BodyTarget { body, from } => rider::body_target(self, body, from),
            Call::BodyAt { body, from } => rider::body_at(self, body, from),
            Call::Switch { part, on } => {
                self.log.borrow_mut().push(c);
                rider::switch_on(self, RIDER, part, on);
            }
            Call::Grow { list, bytes } => {
                self.log.borrow_mut().push(c);
                let n = self.grown;
                self.grown += 1;
                return grow(self.ee, n, list, bytes);
            }
            _ => self.log.borrow_mut().push(c),
        }
        0
    }
}

/// **The riding reset is the original's** (S287): 20 000 cases of state 4's rider, each running
/// 0x004eef40 whole in the interpreter (the pose and settle virtuals and every part's on/off virtual
/// hooked to log, the heap's resize to [`grow`]; the bodies' four virtuals as they are) against
/// [`rider::riding_reset`] from the same words. Drawn: `+0x25c`, the world's riding iterations and
/// rate, `+0x4f0`; the part, pose-list and body counts (the real one, 0, −1); the parts' touched
/// flags; the record (none) and its collidable (none, one already listed, one not); each part's
/// sphere's ignore list (counts 0..4 and −1, entries the collidable, 0 or another); the pose list's
/// strengths, damping ratios and `+0xc`, and the bodies' masses and inertias (signed zeros,
/// negatives, denormals). Every word either side writes, off the stack, comes out the same, and the
/// same calls are made in the same order.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_riding_reset_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let rec = ee.u32_at(RIDER + rider::RECORD);
    let holder = ee.u32_at(RIDER + rider::MAIN);
    let bodies = ee.u32_at(RIDER + rider::BODIES);
    let poses = ee.u32_at(RIDER + rider::POSES);
    let joints_n = ee.u32_at(RIDER + 0x3ac);
    let joints_at = ee.u32_at(RIDER + 0x3b0);
    let parts: Vec<u32> = (0..ee.u32_at(RIDER + rider::PARTS_COUNT))
        .map(|i| ee.u32_at(ee.u32_at(RIDER + rider::PARTS) + 4 * i))
        .collect();
    for i in 0..bodies {
        let b = ee.u32_at(holder + 0xb0 * i + 0xa4);
        for (slot, code) in RESET_BODY_SLOTS {
            assert_eq!(ee.u32_at(ee.u32_at(b + 0x110) + slot), code, "body {i}'s vt {slot:#x}");
        }
    }
    let force = ee.u32_at(RIDER + rider::AIM_FORCE);
    let mut switched: Vec<u32> = rider::RIDE_ON.iter().chain(&rider::RIDE_EXTRA_PARTS).map(|&o| ee.u32_at(RIDER + o)).collect();
    switched.extend(&parts);
    let mut switches: BTreeSet<u32> = switched.iter().map(|&p| ee.u32_at(ee.u32_at(p + 0x10) + 0x10)).collect();
    switches.insert(ee.u32_at(ee.u32_at(force + 8) + 0x10));
    eprintln!(
        "record {rec:#x}, {} parts, {joints_n} joints at {joints_at:#x}, {bodies} bodies, switch fns {switches:#x?}, zero quad {:#x?}, \
         world {:#x}: iterations {:#x}, rate {:#x}, riding {:#x} {:#x}, +0x25c {}, +0x4f0 {}",
        parts.len(),
        [0, 4, 8, 12].map(|k| ee.u32_at(rider::ONES + k)),
        ee.u32_at(RIDER + rider::WORLD),
        ee.u32_at(ee.u32_at(RIDER + rider::WORLD)),
        ee.u32_at(ee.u32_at(RIDER + rider::WORLD) + 4),
        ee.u32_at(RIDER + rider::RIDE_ITERATIONS),
        ee.u32_at(RIDER + rider::RIDE_RATE),
        ee.u32_at(RIDER + rider::LEFT_AS_IS),
        ee.u32_at(RIDER + rider::RIDE_EXTRA),
    );
    let joints: Vec<u32> = (0..joints_n).map(|i| ee.u32_at(joints_at + 0x50 * i + 8)).collect();
    let others: BTreeSet<u32> = joints.iter().map(|&j| ee.u32_at(j + 0x7c)).collect();
    let body_list: Vec<u32> = (0..bodies).map(|i| ee.u32_at(holder + 0xb0 * i + 0xa4)).collect();
    // A written word named by what holds it, for the failure message.
    let name = |a: u32| -> String {
        let near = |xs: &[u32], what: &str| xs.iter().position(|&b| (b..b + 0x200).contains(&a)).map(|i| format!("{what} {i} +{:#x}", a - xs[i]));
        near(&joints, "joint").or_else(|| near(&body_list, "body")).or_else(|| near(&[RIDER], "rider")).unwrap_or_default()
    };
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    hook_calls(&mut ee, &log, &Rc::new(RefCell::new(VecDeque::new())), &switches);
    ee.hooks.remove(&RIDING_RESET);
    let grown = Rc::new(RefCell::new(0u32));
    {
        let (l, n) = (log.clone(), grown.clone());
        ee.hooks.insert(HEAP_RESIZE, Box::new(move |ee: &mut Ee| {
            assert_eq!((ee.r(6), ee.r(7)), (0x0039_683e, 0x4cf), "the resize's file and line");
            let (list, bytes) = (ee.r(4) as u32, ee.r(5) as u32);
            l.borrow_mut().push(Call::Grow { list, bytes });
            let k = *n.borrow();
            *n.borrow_mut() += 1;
            let to = grow(ee, k, list, bytes);
            ee.set(2, u64::from(to));
        }));
    }
    let snapshot = ee.ram.clone();
    let mut g = Gen(0x0287_0001);
    let mut t: BTreeMap<String, u32> = BTreeMap::new();
    let mut tally = |path: String| *t.entry(path).or_insert(0) += 1;
    for k in 0..20_000 {
        let mut given: Vec<(u32, u32)> = Vec::new();
        let mut put = |ee: &mut Ee, at: u32, v: u32| {
            ee.set_u32(at, v);
            given.push((at, v));
        };
        let f = |x: f32| x.to_bits();
        // A float drawn for the strengths, ratios, masses and inertias: signed zeros, negatives and
        // denormals among them.
        let any = |g: &mut Gen| match g.bits() % 10 {
            0 => g.pick(&[0, 0x8000_0000, 0x0000_0001, 0x8040_0000, 0x7f80_0000]),
            1 => f(g.f(-50.0, 0.0)),
            _ => f(g.f(0.0, 80.0)),
        };
        put(&mut ee, RIDER + rider::LEFT_AS_IS, if g.one_in(2) { 0 } else { 1 });
        put(&mut ee, RIDER + rider::RIDE_ITERATIONS, g.bits() % 8);
        put(&mut ee, RIDER + rider::RIDE_RATE, any(&mut g));
        put(&mut ee, RIDER + rider::RIDE_EXTRA, if g.one_in(2) { 0 } else { g.bits() | 1 });
        let n_parts = parts.len() as u32;
        put(&mut ee, RIDER + rider::PARTS_COUNT, g.pick(&[n_parts, n_parts, n_parts, 0, 0xffff_ffff]));
        put(&mut ee, RIDER + 0x3ac, g.pick(&[joints_n, joints_n, joints_n, 0, 0xffff_ffff]));
        put(&mut ee, RIDER + rider::BODIES, g.pick(&[bodies, bodies, bodies, 0, 0xffff_ffff]));
        let collidable = FAKE + 0x1000;
        let has_rec = !g.one_in(5);
        put(&mut ee, RIDER + rider::RECORD, if has_rec { rec } else { 0 });
        let c = if g.one_in(5) { 0 } else { collidable };
        put(&mut ee, rec + rider::R_COLLIDABLE, c);
        for &p in &parts {
            put(&mut ee, p + rider::TOUCHED, if g.one_in(2) { 1 } else { 0 });
        }
        for (k, &p) in parts.iter().enumerate() {
            let sphere = ee.u32_at(p + 0x88);
            let list = FAKE + 0x2000 + 0x10 * k as u32;
            put(&mut ee, sphere + 0xe4, list);
            put(&mut ee, sphere + 0xe8, g.pick(&[0, 1, 2, 3, 4, 0xffff_ffff]));
            for i in 0..4 {
                let w = g.pick(&[collidable, 0, FAKE + 0x1100, FAKE + 0x1100, FAKE + 0x1200]);
                put(&mut ee, list + 4 * i, w);
            }
        }
        for i in 0..joints_n {
            let e = joints_at + 0x50 * i;
            put(&mut ee, e + 0xc, any(&mut g));
            for o in [0x10, 0x20] {
                for c in 0..4 {
                    let v = any(&mut g);
                    put(&mut ee, e + o + 4 * c, v);
                }
            }
        }
        for &o in &others {
            for c in 0..3 {
                let v = any(&mut g);
                put(&mut ee, o + 0x130 + 4 * c, v);
            }
        }
        for i in 0..bodies {
            let e = poses + 0x60 * i;
            for o in [0x0, 0x10, 0x14, 0x18, 0x1c, 0x30, 0x34, 0x38, 0x3c] {
                let v = any(&mut g);
                put(&mut ee, e + o, v);
            }
        }

        // The original.
        log.borrow_mut().clear();
        *grown.borrow_mut() = 0;
        ee.written.clear();
        ee.set(28, GP);
        ee.call(RIDING_RESET, &[u64::from(RIDER)], &[f32::from_bits(DT)], BUDGET);
        let w1 = written_words(&ee, 0);
        let want: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
        let calls = log.borrow().clone();

        for &a in &w1 {
            ee.set_u32(a, word_of(&snapshot, a));
        }
        for &(a, v) in &given {
            ee.set_u32(a, v);
        }
        let pre: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
        log.borrow_mut().clear();
        ee.written.clear();
        let mut port = ResetPort { ee: &mut ee, log: log.clone(), grown: 0 };
        rider::riding_reset(&mut port, RIDER, F(f32::from_bits(DT)));
        let w2 = written_words(&ee, 0);
        let got_calls = log.borrow().clone();
        assert_eq!(got_calls, calls, "case {k}: the calls");
        for &a in w1.union(&w2) {
            let w = want.get(&a).copied().unwrap_or_else(|| pre.get(&a).copied().unwrap_or_else(|| ee.u32_at(a)));
            let before = pre.get(&a).copied().unwrap_or(word_of(&snapshot, a));
            assert_eq!(ee.u32_at(a), w, "case {k}: the word at {a:#x} ({}, was {before:#x}, {})", name(a), if w2.contains(&a) { "the port wrote it" } else { "only the original wrote it" });
        }

        let gv = |a: u32| given.iter().rev().find(|(at, _)| *at == a).unwrap().1;
        tally(format!("posed: {}", calls.contains(&Call::Pose)));
        tally(format!("extra parts on: {}", gv(RIDER + rider::RIDE_EXTRA) != 0));
        let grows = calls.iter().filter(|c| matches!(c, Call::Grow { .. })).count();
        if gv(RIDER + rider::PARTS_COUNT) as i32 > 0 {
            if !has_rec {
                tally("ignore: no record".into());
            } else if c == 0 {
                tally("ignore: no collidable".into());
            } else {
                let mut listed = 0;
                let mut filled = 0;
                for (k, &p) in parts.iter().enumerate() {
                    let sphere = word_of(&snapshot, p + 0x88);
                    let n = gv(sphere + 0xe8) as i32;
                    let list = FAKE + 0x2000 + 0x10 * k as u32;
                    let entries: Vec<u32> = (0..n.max(0) as u32).map(|i| gv(list + 4 * i)).collect();
                    if entries.contains(&c) {
                        listed += 1;
                    } else if entries.contains(&0) {
                        filled += 1;
                    }
                }
                if listed > 0 {
                    tally("ignore: listed already".into());
                }
                if filled > 0 {
                    tally("ignore: an empty entry".into());
                }
                if grows > 0 {
                    tally("ignore: grown".into());
                }
            }
        }
        let jn = gv(RIDER + 0x3ac) as i32;
        tally(format!("joints: {}", if jn > 0 { "some" } else { "none" }));
        if jn > 0 && joints.iter().any(|&j| (0..3).any(|c| want.get(&(j + 0x100 + 4 * c)).is_some_and(|&w| w == 0 || w == 0x8000_0000))) {
            tally("joints: a zero damping".into());
        }
        let bn = gv(RIDER + rider::BODIES) as i32;
        tally(format!("bodies: {}", if bn > 0 { "some" } else { "none" }));
        if bn > 0 {
            for i in 0..bodies {
                let e = poses + 0x60 * i;
                for c in 0..3 {
                    let x = fpu_operand(gv(e + 0x10 + 4 * c));
                    tally(if x <= 0.0 { "bodies: an inertia not above 0".into() } else { "bodies: an inertia above 0".into() });
                }
            }
        }
        for a in w1.union(&w2).copied().chain(given.iter().map(|(a, _)| *a)) {
            ee.set_u32(a, word_of(&snapshot, a));
        }
        *grown.borrow_mut() = 0;
    }
    eprintln!("{t:#?}");
    for path in ["posed: true", "posed: false", "extra parts on: true", "extra parts on: false", "ignore: no record", "ignore: no collidable",
        "ignore: listed already", "ignore: an empty entry", "ignore: grown", "joints: some", "joints: none", "joints: a zero damping",
        "bodies: some", "bodies: none", "bodies: an inertia not above 0", "bodies: an inertia above 0"] {
        assert!(t.contains_key(path), "path {path} never taken");
    }
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
}

/// A float word as the FPU's compares read it.
fn fpu_operand(w: u32) -> f32 {
    ps2_float::fpu::operand(w)
}

/// **The rider's wreck entry is the original's** (S288): 20 000 cases of state 4's rider, each
/// running 0x004ed700 whole in the interpreter (its pose, settle, switches, rand, reallocate and cue
/// hooked as the machine's test hooks them; the bodies' vt `+0x18`, `+0x60` and `+0x68` run as they
/// are) against [`rider::wreck_entry`] from the same words. Drawn: the kind (every one of 0..=16, and
/// two past the table), the three counts (the real one, 0, −1), the record's collidable (or none)
/// and the spheres' ignore lists (it listed, a free slot, full so the list grows), the pose list's
/// entries and their bodies' `+0x7c` lanes (negatives under the square root among them), the
/// masses, inertias (≤ 0 and −0 lanes) and targets, the record's velocities, the bodies' positions
/// and orientations, every kind's block of aim and clock words, and the world's two settings. Every
/// word either side writes, off the stack, comes out the same, and so do the calls.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_riders_wreck_entry_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let rec = ee.u32_at(RIDER + rider::RECORD);
    let bodies = ee.u32_at(RIDER + rider::BODIES);
    let poses = ee.u32_at(RIDER + rider::POSES);
    let entries = ee.u32_at(RIDER + 0x3ac);
    let entry_list = ee.u32_at(RIDER + 0x3b0);
    let parts: Vec<u32> = (0..ee.u32_at(RIDER + rider::PARTS_COUNT))
        .map(|i| ee.u32_at(ee.u32_at(RIDER + rider::PARTS) + 4 * i))
        .collect();
    let named: Vec<u32> = rider::OFF_FIRST.iter().chain(&rider::LET_GO).map(|&o| ee.u32_at(RIDER + o)).collect();
    let mut switches: BTreeSet<u32> = named.iter().chain(&parts).map(|&p| ee.u32_at(ee.u32_at(p + 0x10) + 0x10)).collect();
    let force = ee.u32_at(RIDER + rider::AIM_FORCE);
    switches.insert(ee.u32_at(ee.u32_at(force + 8) + 0x10));
    let holder_body = |ee: &Ee, i: u32| ee.u32_at(ee.u32_at(RIDER + rider::MAIN) + 0xb0 * i + 0xa4);
    for i in 0..bodies {
        let b = holder_body(&ee, i);
        for (slot, code) in [(0x18, BODY_MASS), (0x60, BODY_INERTIA), (0x68, BODY_TARGET)] {
            assert_eq!(ee.u32_at(ee.u32_at(b + 0x110) + slot), code, "body {i}'s vt {slot:#x}");
        }
    }
    let entry_bodies: Vec<u32> = (0..entries).map(|i| ee.u32_at(entry_list + 0x50 * i + 8)).collect();
    let inertia_of: BTreeSet<u32> = entry_bodies.iter().map(|&b| ee.u32_at(b + 0x7c)).collect();
    eprintln!("{bodies} bodies, {entries} pose-list entries ({} inertia objects), {} parts, switch fns {switches:#x?}", inertia_of.len(), parts.len());
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let answers = Rc::new(RefCell::new(VecDeque::new()));
    hook_calls(&mut ee, &log, &answers, &switches);
    // The wreck entry itself runs.
    ee.hooks.remove(&WRECK_ENTRY);
    let l = log.clone();
    ee.hooks.insert(REALLOC, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(6), ee.r(7)), (0x0039_683e, 0x4cf), "reallocate: its file and line");
        l.borrow_mut().push(Call::Grow { list: ee.r(4) as u32, bytes: ee.r(5) as u32 });
        ee.set(2, u64::from(WRECK_GROWN));
    }));
    let l = log.clone();
    ee.hooks.insert(WRECK_CUE, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(5), 0, "cue: a1");
        l.borrow_mut().push(Call::WreckCue(ee.r(4) as u32));
    }));
    let snapshot = ee.ram.clone();

    let mut g = Gen(0x0288_0001);
    let mut t: BTreeMap<String, u32> = BTreeMap::new();
    let mut tally = |path: String| *t.entry(path).or_insert(0) += 1;
    let f = |x: f32| x.to_bits();
    for k in 0..20_000 {
        let mut given: Vec<(u32, u32)> = Vec::new();
        let mut put = |ee: &mut Ee, at: u32, v: u32| {
            ee.set_u32(at, v);
            given.push((at, v));
        };
        let kind = if g.one_in(20) { g.pick(&[0x40, 0xffff_ffff]) } else { g.bits() % 17 };
        let count = |g: &mut Gen, real: u32| g.pick(&[real, real, real, real, real, real, 0, 0xffff_ffff]);
        put(&mut ee, RIDER + rider::PARTS_COUNT, count(&mut g, parts.len() as u32));
        put(&mut ee, RIDER + 0x3ac, count(&mut g, entries));
        put(&mut ee, RIDER + rider::BODIES, count(&mut g, bodies));
        for o in rider::WORLD_SETTINGS {
            put(&mut ee, RIDER + o, g.bits());
        }
        // The collidable and the spheres' ignore lists.
        let collidable = FAKE + 0x1000;
        let c = if g.one_in(5) { 0 } else { collidable };
        put(&mut ee, rec + rider::R_COLLIDABLE, c);
        for (n, &p) in parts.iter().enumerate() {
            let sphere = ee.u32_at(p + 0x88);
            let list = FAKE + 0x2000 + 0x20 * n as u32;
            put(&mut ee, sphere + 0xe4, list);
            put(&mut ee, sphere + 0xe8, g.pick(&[0, 1, 2, 3, 4, 4, 0xffff_ffff]));
            let full = g.one_in(2);
            for i in 0..4 {
                let w = if full { g.pick(&[FAKE + 0x1100, FAKE + 0x1200]) } else { g.pick(&[collidable, 0, FAKE + 0x1100, FAKE + 0x1100]) };
                put(&mut ee, list + 4 * i, w);
            }
        }
        // The pose list's entries and their bodies' lanes.
        for i in 0..entries {
            let e = entry_list + 0x50 * i;
            put(&mut ee, e + 0xc, f(g.near(0.0, 0.0, 4.0)));
            for o in [0x30, 0x40] {
                for c in 0..4 {
                    let x = if o == 0x30 && g.one_in(10) { g.f(-2.0, 0.0) } else { g.near(0.0, 0.0, 9.0) };
                    put(&mut ee, e + o + 4 * c, f(x));
                }
            }
        }
        for &o in &inertia_of {
            for c in 0..3 {
                let x = if g.one_in(10) { g.f(-2.0, 0.0) } else { g.near(0.0, 0.0, 9.0) };
                put(&mut ee, o + 0x130 + 4 * c, f(x));
            }
        }
        // The masses, inertias and targets.
        for i in 0..bodies {
            let p = poses + 0x60 * i;
            let mass = match g.bits() % 8 {
                0 => g.pick(&[0, 0x8000_0000]),
                1 => f(g.f(-3.0, 0.0)),
                _ => f(g.f(0.1, 20.0)),
            };
            put(&mut ee, p + 4, mass);
            for c in 0..4 {
                let x = match g.bits() % 6 {
                    0 => g.pick(&[0, 0x8000_0000, 1]),
                    1 => f(g.f(-2.0, 0.0)),
                    _ => f(g.f(0.01, 5.0)),
                };
                put(&mut ee, p + 0x20 + 4 * c, x);
            }
            for c in 0..4 {
                put(&mut ee, p + 0x50 + 4 * c, f(g.near(0.0, -9.0, 9.0)));
            }
        }
        // The record's velocities, the bodies' positions and orientations.
        for o in [0x180, 0xa0, 0xb0] {
            for c in 0..4 {
                put(&mut ee, rec + o + 4 * c, f(g.near(0.0, -30.0, 30.0)));
            }
        }
        for i in 0..bodies {
            let b = holder_body(&ee, i);
            for c in 0..4 {
                put(&mut ee, b + 0x70 + 4 * c, f(g.near(0.0, -60.0, 60.0)));
                put(&mut ee, b + 0x190 + 4 * c, f(g.near(0.0, -1.0, 1.0)));
            }
        }
        // Every kind's block: base times, aim, spreads.
        for at in (0x680..0x840).step_by(4) {
            put(&mut ee, RIDER + at, f(g.near(0.0, -3.0, 3.0)));
        }
        let rands: Vec<u32> = (0..8).map(|_| g.bits() >> 1).collect();

        // The original.
        log.borrow_mut().clear();
        *answers.borrow_mut() = rands.iter().copied().collect();
        ee.written.clear();
        ee.set(28, GP);
        ee.call(WRECK_ENTRY, &[u64::from(RIDER), u64::from(kind)], &[f32::from_bits(DT)], BUDGET);
        let w1 = written_words(&ee, 0);
        let want: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
        let calls = log.borrow().clone();

        // Back to the inputs, then the port.
        for &a in &w1 {
            ee.set_u32(a, word_of(&snapshot, a));
        }
        for &(a, v) in &given {
            ee.set_u32(a, v);
        }
        let pre: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
        log.borrow_mut().clear();
        ee.written.clear();
        let mut port = Port { ee: &mut ee, log: log.clone(), rands: rands.iter().copied().collect() };
        rider::wreck_entry(&mut port, RIDER, kind, F(f32::from_bits(DT)));
        let w2 = written_words(&ee, 0);
        assert_eq!(*log.borrow(), calls, "case {k} (kind {kind}): the calls");
        for &a in w1.union(&w2) {
            let w = want.get(&a).copied().unwrap_or_else(|| pre.get(&a).copied().unwrap_or_else(|| ee.u32_at(a)));
            assert_eq!(ee.u32_at(a), w, "case {k} (kind {kind}): the word at {a:#x} ({})", if w2.contains(&a) { "the port wrote it" } else { "only the original wrote it" });
        }
        for &a in w1.union(&w2).chain(given.iter().map(|(a, _)| a)) {
            ee.set_u32(a, word_of(&snapshot, a));
        }

        tally(format!("kind {}", if kind < 17 { kind.to_string() } else { "past".into() }));
        let gv = |a: u32| given.iter().rev().find(|(at, _)| *at == a).map(|g| g.1);
        if calls.iter().any(|c| matches!(c, Call::Grow { .. })) {
            tally("keep: grown".into());
        }
        if (0..parts.len() as u32).any(|n| (0..4).any(|i| want.contains_key(&(FAKE + 0x2000 + 0x20 * n + 4 * i)))) {
            tally("keep: a free slot".into());
        }
        if gv(RIDER + rider::BODIES) == Some(bodies) {
            let inv0 = (0..bodies).any(|i| (0..3).any(|c| want.get(&(holder_body(&ee, i) + 0x140 + 4 * c)) == Some(&0)));
            if inv0 {
                tally("inertia: a lane at 0".into());
            }
        }
        if kind == 5 {
            let holds: String = calls
                .iter()
                .filter_map(|c| match *c {
                    Call::Switch { part, on } if part == named[7] => Some(format!("l{on}")),
                    Call::Switch { part, on } if part == named[8] => Some(format!("r{on}")),
                    _ => None,
                })
                .collect();
            tally(format!("kind 5: {holds}"));
        }
    }
    eprintln!("{t:#?}");
    for kind in [1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 13, 15, 0, 9, 10, 14, 16] {
        assert!(t.contains_key(&format!("kind {kind}")), "kind {kind} never drawn");
    }
    for path in ["keep: grown", "keep: a free slot", "inertia: a lane at 0", "kind 5: l1r1", "kind 5: l0r0", "kind 5: l1r0", "kind 5: l0r1", "kind past"] {
        assert!(t.contains_key(path), "path {path} never taken");
    }
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
}

// F55: the rider's listener step's own code — the frame around the mode machine, the copy after it
// and the extra step — each against the original.

const FRAME: u32 = 0x004f_1790;
const COPY_AFTER: u32 = 0x004f_2970;
const EXTRA_STEP: u32 = 0x004f_2d50;
const WORLD_STEP: u32 = 0x0050_5d50;
const NODE_POSITION: u32 = 0x0019_4c20;

/// What both sides' node position writes: a quadword that names the call.
fn position_marker(node: u32, out: u32) -> [u32; 4] {
    [node, out, 0x3f80_0000, node ^ out]
}

/// The port's side of the listener step's calls: logged, the world step answering `answer`, the
/// node position writing [`position_marker`].
struct StepPort<'a> {
    ee: &'a mut Ee,
    log: Log,
    answer: u32,
}

impl node::Ram for StepPort<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w)
    }
}

impl rider::Rider for StepPort<'_> {
    fn call(&mut self, c: Call, _dt: F) -> u32 {
        self.log.borrow_mut().push(c);
        match c {
            Call::WorldStep => self.answer,
            Call::AfterStep => 0,
            Call::NodePosition { node, out } => {
                for (k, w) in position_marker(node, out).into_iter().enumerate() {
                    self.ee.set_u32(out + 4 * k as u32, w);
                }
                0
            }
            _ => panic!("{c:?}: not a call of the listener step's own code"),
        }
    }
}

/// **The rider's listener step's own code is the original's** (F55), on state 4's rider, 20 000
/// cases each of: the rider's frame with its record not ridden (the node's alpha off the rider's
/// kind 0..4 and −1, the kind-3 switch, the record's ask 0..3 and its fade and start — above, at and
/// below 0, signed zeros, denormals, saturated words; the two clocks, their targets, flags and rates
/// on both sides; no node, no record; the world step's answer drawn); the copy after it (record or
/// none, its wreck kind, the mask against each copy's bits, `+0xa70` 0..2); and the extra step (its
/// switch, the table index 0..3, each entry's time, step and down word, the record's speed about 5
/// and 10, the value, clock and ceiling, `dt` 0 among them). The world step and the rider's vt
/// `+0x148` are hooked and logged, the node position writes a marker. Every word either side writes,
/// off the stack, comes out the same, the same calls are made in the same order, and the answers
/// agree. The frame's ridden arm (the mode machine, proven by S278b's oracle) is proven by C15's
/// replays (`tests/mxvatv_bike_ride.rs`, `*_with_the_rider_leaf_on_the_ports`).
#[test]
#[allow(clippy::float_arithmetic)]
fn the_riders_listener_step_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let rec = ee.u32_at(RIDER + rider::RECORD);
    let node = ee.u32_at(RIDER + rider::NODE);
    assert!(rec != 0 && node != 0, "state 4's rider has a record and a node");
    let after_vt = ee.u32_at(ee.u32_at(RIDER + 4) + 0x148);
    eprintln!("record {rec:#x}, node {node:#x}, vt +0x148 {after_vt:#x}");
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let answer = Rc::new(RefCell::new(0u32));
    {
        let (l, a) = (log.clone(), answer.clone());
        ee.hooks.insert(WORLD_STEP, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, RIDER, "the world step's rider");
            l.borrow_mut().push(Call::WorldStep);
            ee.set(2, u64::from(*a.borrow()));
        }));
        let l = log.clone();
        ee.hooks.insert(after_vt, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, RIDER, "vt +0x148's rider");
            l.borrow_mut().push(Call::AfterStep);
        }));
        let l = log.clone();
        ee.hooks.insert(NODE_POSITION, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(5), 0, "the node position's a1");
            let (node, out) = (ee.r(4) as u32, ee.r(6) as u32);
            l.borrow_mut().push(Call::NodePosition { node, out });
            for (k, w) in position_marker(node, out).into_iter().enumerate() {
                ee.set_u32(out + 4 * k as u32, w);
            }
        }));
    }
    let snapshot = ee.ram.clone();
    let mut t: BTreeMap<String, u32> = BTreeMap::new();
    let mut g = Gen(0x0f55_0001);
    let f = |x: f32| x.to_bits();
    // A float word: mostly near `at` in `lo..hi`, sometimes a signed zero, a denormal or saturated.
    let word = |g: &mut Gen, at: f32, lo: f32, hi: f32| match g.bits() % 12 {
        0 => g.pick(&[0, 0x8000_0000]),
        1 => g.pick(&[0x0000_0001, 0x8040_0000, 0x7f80_0000, 0xff80_0001]),
        _ => g.near(at, lo, hi).to_bits(),
    };
    for (what, fun) in [("frame", FRAME), ("copy", COPY_AFTER), ("extra", EXTRA_STEP)] {
        for k in 0..20_000 {
            let mut given: Vec<(u32, u32)> = Vec::new();
            let mut put = |ee: &mut Ee, at: u32, v: u32| {
                ee.set_u32(at, v);
                given.push((at, v));
            };
            let some = f(g.f(0.0, 0.1));
            let dt = g.pick(&[DT, DT, DT, 0, some]);
            let mut path = Vec::new();
            match what {
                "frame" => {
                    put(&mut ee, rec + rider::R_RIDDEN, 0);
                    let kind = g.pick(&[0, 1, 2, 3, 3, 4, 0xffff_ffff]);
                    put(&mut ee, RIDER + rider::KIND, kind);
                    let switch = g.pick(&[0, 1]);
                    put(&mut ee, RIDER + rider::KIND3_FADE, switch);
                    let ask = g.pick(&[0, 1, 2, 3]);
                    put(&mut ee, rec + rider::R_ALPHA, ask);
                    let fade = word(&mut g, 0.0, -0.5, 2.0);
                    put(&mut ee, rec + rider::R_FADE, fade);
                    let from = if g.one_in(4) { g.pick(&[0, 0x8000_0000, 0x0000_0001]) } else { word(&mut g, 1.0, -1.0, 3.0) };
                    put(&mut ee, rec + rider::R_FADE_FROM, from);
                    for (clock, target, flag, set, clear) in rider::CLOCKS {
                        put(&mut ee, RIDER + flag, g.pick(&[0, 1]));
                        for o in [set, clear] {
                            let r = word(&mut g, 1.0, -0.5, 5.0);
                            put(&mut ee, RIDER + o, r);
                        }
                        let c = word(&mut g, 0.0, -1.0, 1.0);
                        put(&mut ee, RIDER + clock, c);
                        let at = if g.one_in(6) { c } else { word(&mut g, 0.0, -1.0, 1.0) };
                        put(&mut ee, RIDER + target, at);
                    }
                    put(&mut ee, RIDER + rider::UNRIDDEN_CLEAR, g.bits());
                    let (no_node, no_rec) = (g.one_in(20), g.one_in(20));
                    if no_node {
                        put(&mut ee, RIDER + rider::NODE, 0);
                    }
                    if no_rec {
                        put(&mut ee, RIDER + rider::RECORD, 0);
                    }
                    *answer.borrow_mut() = g.bits();
                    path.push(match () {
                        _ if no_node || no_rec => "none".to_string(),
                        _ if kind == 2 => "kind 2".into(),
                        _ if kind == 3 => format!("kind 3, switch {switch}"),
                        _ => {
                            let fo = fpu_operand(fade);
                            let tail = if fo <= 0.0 { "no fade" } else if fpu_operand(from) == 0.0 { "fade from 0" } else { "fade" };
                            format!("ask {}, {tail}", ask.min(3))
                        }
                    });
                }
                "copy" => {
                    let has_rec = !g.one_in(4);
                    if !has_rec {
                        put(&mut ee, RIDER + rider::RECORD, 0);
                    }
                    let kind = g.pick(&[0, 0, 1, 13]);
                    put(&mut ee, rec + rider::R_KIND, kind);
                    let mask = g.pick(&[0, 1, 2, 3, 0xffff_ffff]);
                    put(&mut ee, RIDER + rider::COPY_MASK, mask);
                    for o in [0x9d4, 0x904, 0xa94] {
                        let b = g.pick(&[0, 1, 2, 3]);
                        put(&mut ee, RIDER + o, b);
                    }
                    let a70 = g.pick(&[0, 1, 1, 2]);
                    put(&mut ee, RIDER + 0xa70, a70);
                    for o in [0x214, 0xa0, 0xa4, 0xa8, 0xac] {
                        put(&mut ee, rec + o, g.bits());
                    }
                    for i in 0..12 {
                        put(&mut ee, rec + 0x220 + 4 * i, g.bits());
                    }
                    put(&mut ee, RIDER + 0xa74, g.bits());
                    put(&mut ee, RIDER + rider::SPOT_NODE, g.bits() & !3);
                    let on = |o: u32| mask & given.iter().rev().find(|(a, _)| *a == RIDER + o).unwrap().1 != 0;
                    let lit = |b: bool| if b { "on" } else { "off" };
                    path.push(if has_rec { "record".to_string() } else { format!("no record, {} a70 {a70}", lit(on(0x9d4))) });
                    for o in [0x904, 0xa94] {
                        path.push(format!("{o:#x} {}{}", lit(on(o)), if on(o) { if has_rec && kind != 0 { " wrecked" } else { " node" } } else { "" }));
                    }
                }
                _ => {
                    let on = g.pick(&[0, 1, 1, 1, 0x100]);
                    put(&mut ee, RIDER + rider::EXTRA_ON, on);
                    let i = g.pick(&[0, 1, 2, 3]);
                    put(&mut ee, rec + 0x1e8, i);
                    for e in 0..4 {
                        let at = RIDER + rider::EXTRA_TABLE + 32 * e;
                        let time = word(&mut g, 0.0, -0.1, 0.2);
                        put(&mut ee, at, time);
                        let step = word(&mut g, 0.5, -2.0, 2.0);
                        put(&mut ee, at + 4, step);
                        let down = g.pick(&[0, 1]);
                        put(&mut ee, at + 8, down);
                    }
                    let edge = g.pick(&[5.0f32.to_bits(), 10.0f32.to_bits()]);
                    let speed = if g.one_in(5) { edge } else { word(&mut g, 7.5, 0.0, 15.0) };
                    put(&mut ee, rec + 0x1a8, speed);
                    for (o, at, lo, hi) in [(rider::EXTRA_VALUE, 0.5, -1.0, 3.0), (rider::EXTRA_CLOCK, 0.0, -0.1, 0.3), (rider::EXTRA_MAX, 1.0, -1.0, 3.0)] {
                        let v = word(&mut g, at, lo, hi);
                        put(&mut ee, RIDER + o, v);
                    }
                    let gv = |a: u32| given.iter().rev().find(|(x, _)| *x == a).unwrap().1;
                    let e = RIDER + rider::EXTRA_TABLE + 32 * i;
                    let down = gv(e + 8) != 0;
                    let fast = fpu_operand(speed) > if down { 5.0 } else { 10.0 };
                    path.push(match () {
                        _ if on == 0 => "off".to_string(),
                        _ if down && fpu_operand(gv(e)) <= 0.0 => "down, no time".into(),
                        _ if !fast => format!("{}, slow", if down { "down" } else { "up" }),
                        _ => format!("{}, fast", if down { "down" } else { "up" }),
                    });
                }
            }

            // The original.
            log.borrow_mut().clear();
            ee.set(28, GP);
            ee.call(fun, &[u64::from(RIDER)], &[f32::from_bits(dt)], BUDGET);
            let v1 = ee.r(2) as u32;
            let w1 = written_words(&ee, 0);
            let want: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
            let calls = log.borrow().clone();

            // Back to the inputs, then the port.
            for &a in &w1 {
                ee.set_u32(a, word_of(&snapshot, a));
            }
            for &(a, v) in &given {
                ee.set_u32(a, v);
            }
            let pre: HashMap<u32, u32> = w1.iter().map(|&a| (a, ee.u32_at(a))).collect();
            log.borrow_mut().clear();
            ee.written.clear();
            let dt = F(f32::from_bits(dt));
            let v2 = {
                let mut port = StepPort { ee: &mut ee, log: log.clone(), answer: *answer.borrow() };
                match what {
                    "frame" => rider::frame(&mut port, RIDER, dt, None),
                    "copy" => {
                        rider::copy_after(&mut port, RIDER);
                        v1
                    }
                    _ => rider::extra_step(&mut port, RIDER, dt),
                }
            };
            let w2 = written_words(&ee, 0);
            assert_eq!(*log.borrow(), calls, "{what} case {k} ({path:?}): the calls");
            assert_eq!(v2, v1, "{what} case {k} ({path:?}): the answer");
            for &a in w1.union(&w2) {
                let w = want.get(&a).copied().unwrap_or_else(|| pre.get(&a).copied().unwrap_or_else(|| ee.u32_at(a)));
                let got = ee.u32_at(a);
                assert_eq!(got, w, "{what} case {k} ({path:?}): the word at {a:#x} ({})", if w2.contains(&a) { "the port wrote it" } else { "only the original wrote it" });
            }
            for &a in w1.union(&w2).chain(given.iter().map(|(a, _)| a)) {
                ee.set_u32(a, word_of(&snapshot, a));
            }
            for p in path {
                *t.entry(format!("{what}: {p}")).or_insert(0) += 1;
            }
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
    for path in ["frame: kind 2", "frame: kind 3, switch 0", "frame: kind 3, switch 1", "frame: ask 1, fade", "frame: ask 2, fade from 0", "frame: ask 0, no fade", "frame: none", "extra: down, no time", "extra: down, fast", "extra: up, fast", "extra: up, slow", "copy: no record, on a70 1", "copy: 0xa94 on wrecked", "copy: 0x904 on node"] {
        assert!(t.contains_key(path), "path {path} never taken");
    }
}
