//! S278b — MX vs. ATV's rider mode machine ([`rider::modes`]) against SLUS-21104's own, off state 4's
//! rider (research/oracle.txt `# S278b`).
//!
//! The original's machine runs whole in the interpreter with its outside calls hooked to log and do
//! nothing (rand answers a drawn word): the riding reset, the wreck entry, the rider's pose and settle
//! virtuals, the hold parts' on/off virtual, and the wreck arm's body (a hook on its first pc that
//! resumes at the clear loop). The port runs from the same words with a [`rider::Rider`] that logs the
//! same way. Every word either side writes, outside the stack, comes out the same, and the calls are
//! the same, in the same order, with the same arguments. Skips without the image.

use game_sim::handling::node;
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
/// The wreck arm's body: its first pc after the mode test, and the clear loop's first.
const WRECK_ARM: (u32, u32) = (0x004e_fe10, 0x004f_0274);
/// The fake points the record's pointers aim at (free RAM past F39's fake world).
const FAKE: u32 = 0x01c0_a000;

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
        self.log.borrow_mut().push(c);
        match c {
            Call::Rand => self.rands.pop_front().expect("rand asked once more than the original did"),
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
    ee.hooks.insert(WRECK_ARM.0, Box::new(move |ee: &mut Ee| {
        l.borrow_mut().push(Call::WreckArm);
        ee.set(31, u64::from(WRECK_ARM.1) | 0xffff_ffff_0000_0000);
    }));
}

fn word_of(ram: &[u8], at: u32) -> u32 {
    let i = at as usize;
    u32::from_le_bytes([ram[i], ram[i + 1], ram[i + 2], ram[i + 3]])
}

/// The words written, outside the stack.
fn written_words(ee: &Ee) -> BTreeSet<u32> {
    let stack = ee.stack;
    ee.written.iter().map(|&a| a & !3).filter(|a| !(stack - 0x4_0000..stack + 0x1000).contains(a)).collect()
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
    let rec = ee.u32_at(RIDER + rider::RECORD);
    let holder = ee.u32_at(RIDER + rider::MAIN);
    let pose_slot = ee.u32_at(RIDER + 0x1fc).wrapping_add(ee.u32_at(holder + 0x40) << 6).wrapping_add(0x20);
    let follow = ee.u32_at(RIDER + 0x30c);
    let holds = [rider::HOLD_L, rider::HOLD_R, rider::FREE_L, rider::FREE_R].map(|o| ee.u32_at(RIDER + o));
    let switches: BTreeSet<u32> = holds.iter().map(|&p| ee.u32_at(ee.u32_at(p + 0x10) + 0x10)).collect();
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
        for o in [0x63c, 0x638, 0x380, 0x640] {
            let x = g.bits();
            put(&mut ee, RIDER + o, x);
        }
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
        let w1 = written_words(&ee);
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
        rider::modes(&mut port, RIDER, F(f32::from_bits(DT)));
        let w2 = written_words(&ee);
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
        let seq: String = calls
            .iter()
            .filter_map(|c| match *c {
                Call::Switch { part, on } => {
                    let i = holds.iter().position(|&h| h == part).expect("a part not among the holds");
                    Some(format!("{}{on}", ["L", "R", "l", "r"][i]))
                }
                _ => None,
            })
            .collect();
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
