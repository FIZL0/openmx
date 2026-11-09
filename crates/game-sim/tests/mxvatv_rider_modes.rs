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
const RING: (u32, u32) = (0x0054_1550, 0x0054_1550 + 0x70 * 32);
/// The rider's bodies' vt `+0x68` (vtable `0x003cd200`), run as it is.
const BODY_TARGET: u32 = 0x0049_3b00;
/// The fake points the record's pointers aim at (free RAM past F39's fake world).
const FAKE: u32 = 0x01c0_a000;
/// The wreck entry's calls beyond the machine's: the heap's reallocate (answered with [`GROWN`]),
/// the rider's `+0x8f4` object's cue, and the bodies' vt `+0x18` (mass) and `+0x60` (inertia), run
/// as they are.
const REALLOC: u32 = 0x001b_7470;
const GROWN: u32 = FAKE + 0x8000;
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
            Call::Realloc { .. } => GROWN,
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
    for &at in switches {
        let l = log.clone();
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            l.borrow_mut().push(Call::Switch { part: ee.r(4) as u32, on: ee.r(5) as u32 });
        }));
    }
    let l = log.clone();
    ee.hooks.insert(GATHER, Box::new(move |ee: &mut Ee| {
        l.borrow_mut().push(Call::Gather { list: ee.r(4) as u32, probe: ee.r(5) as u32, kind: ee.r(6) as u32 });
    }));
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
/// are made in the same order.
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
                let w = g.pick(&[collidable, collidable, 0, FAKE + 0x1100]);
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
        l.borrow_mut().push(Call::Realloc { block: ee.r(4) as u32, bytes: ee.r(5) as u32 });
        ee.set(2, u64::from(GROWN));
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
        if calls.iter().any(|c| matches!(c, Call::Realloc { .. })) {
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
