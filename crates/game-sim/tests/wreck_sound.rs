//! S53 — the wreck sound's draw against the original.
//!
//! Both proofs run on the rider's live sound emitter in the first recording's save state:
//!
//! 1. **the pick**, the engine's no-repeat pick, called directly over 10 000 generated lists and
//!    generator states;
//! 2. **the wreck sound**, the wreck entry's last call, over every reason from −1 to 16, the
//!    emitter busy or not, both lists generated.
//!
//! The emitter's own "still playing" virtual is hooked to answer what the case says. Playing the sound
//! is hooked out by the emitter's play gate, which every case clears: that part is audio and draws
//! nothing.

use game_sim::rng::Rng;
use game_sim::wreck_sound::{Emitter, List, RETAIL_SOUNDS, Which};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::Cell;
use std::rc::Rc;

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
/// The rider's body actor in that state.
const ACTOR: u32 = 0x0159_1c10;
const CASES: usize = 10_000;
const BUDGET: u64 = 100_000;

/// The fields of the actor and its emitter the call reads (research/oracle.txt, `wreck_sound`).
mod at {
    pub const EMITTER: u32 = 0xa74;
    pub const REQUEST: u32 = 0x488;
    pub const REASON: u32 = 0x104;
    pub const VTABLE: u32 = 0x8;
    pub const FLAGS: u32 = 0x29;
    pub const PLAY_GATE: u32 = 0x1f0;
    pub const FALL_COUNT: u32 = 0x200;
    pub const FALL_LAST: u32 = 0x218;
    pub const CRASH_COUNT: u32 = 0x21c;
    pub const CRASH_LAST: u32 = 0x234;
    pub const EMITTER_REQUEST: u32 = 0x240;
}

/// A scratch word the pick's `last` lives in when the pick is called on its own.
const SCRATCH: u32 = 0x0010_0000 - 0x40;

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of first_recording.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn upto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    /// A list size: mostly small, the retail five often, now and then large or negative.
    fn count(&mut self) -> i32 {
        match self.upto(8) {
            0 => -(self.upto(4) as i32),
            1 => RETAIL_SOUNDS,
            2 => self.upto(1 << 20) as i32 + 2,
            _ => self.upto(8) as i32,
        }
    }
    /// A last pick: never picked, inside the list, or just past it.
    fn last(&mut self, count: i32) -> i32 {
        match self.upto(4) {
            0 => -1,
            1 => self.next() as i32,
            _ => self.upto(count.max(1) as u64 + 1) as i32,
        }
    }
}

fn rand_state() -> u32 {
    addr("rand_state").unwrap()
}

fn emitter(ee: &Ee) -> u32 {
    ee.u32_at(ACTOR + at::EMITTER)
}

fn emitter_of(ee: &Ee) -> Emitter {
    let e = emitter(ee);
    let list = |count, last| List { count: ee.u32_at(e + count) as i32, last: ee.u32_at(e + last) as i32 };
    Emitter { crash: list(at::CRASH_COUNT, at::CRASH_LAST), fall: list(at::FALL_COUNT, at::FALL_LAST) }
}

/// Every retail rider's emitter: two lists of five, nothing picked, and the request it reads the
/// reason off is the body actor's own, the one the wreck entry is handed.
#[test]
fn the_emitter_is_the_retail_one() {
    let Some(ee) = state() else { return };
    let e = emitter(&ee);
    assert_eq!(emitter_of(&ee), Emitter::retail());
    assert_eq!(ee.u32_at(e + at::EMITTER_REQUEST), ee.u32_at(ACTOR + at::REQUEST));
    let vt = ee.u32_at(e + at::VTABLE);
    assert_eq!(ee.u32_at(vt + 0x90), addr("emitter_busy").unwrap());
}

#[test]
fn the_pick_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let pick_fn = addr("sound_pick").unwrap();
    let mut pick = Pick(0x5eed_0053_50a7_d000);
    let (mut drew, mut moved_on, mut short) = (0, 0, 0);
    for case in 0..CASES {
        let count = pick.count();
        let last = pick.last(count);
        let seed = pick.next();
        ee.write(rand_state(), 8, u128::from(seed));
        ee.set_u32(SCRATCH, last as u32);

        let mut rng = Rng::from_state(seed);
        let mut list = List { count, last };
        let got = list.pick(&mut rng);
        ee.call(pick_fn, &[count as i64 as u64, u64::from(SCRATCH)], &[], BUDGET);

        let ctx = format!("case {case}: count {count}, last {last}");
        assert_eq!(got, ee.v0() as i32, "{ctx}: the pick");
        assert_eq!(list.last, ee.u32_at(SCRATCH) as i32, "{ctx}: the last pick");
        assert_eq!(rng.state(), ee.read(rand_state(), 8) as u64, "{ctx}: the generator");
        if count < 2 {
            short += 1;
        } else {
            drew += 1;
            let mut plain = Rng::from_state(seed);
            moved_on += usize::from(plain.index(count) != got);
        }
    }
    assert!(short > 1_000 && drew > 5_000 && moved_on > 300, "short {short}, drew {drew}, moved on {moved_on}");
}

#[test]
fn the_wreck_sound_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let e = emitter(&ee);
    let vt = ee.u32_at(e + at::VTABLE);
    // The emitter's status, as the case says.
    let busy = Rc::new(Cell::new(0u64));
    let b = Rc::clone(&busy);
    ee.hooks.insert(ee.u32_at(vt + 0x90), Box::new(move |ee: &mut Ee| ee.set(2, b.get())));
    // Audio the play path calls before its gate: logged, and never asked to do anything.
    let stopped = Rc::new(Cell::new(0usize));
    let s = Rc::clone(&stopped);
    ee.hooks.insert(ee.u32_at(vt + 0x20), Box::new(move |_| s.set(s.get() + 1)));
    let request = ee.u32_at(ACTOR + at::REQUEST);
    let entry = addr("wreck_sound").unwrap();
    let mut pick = Pick(0x5eed_0053_0000_50d0);
    let mut seen = [[0usize; 3]; 18];
    for case in 0..CASES {
        let reason = pick.upto(18) as i32 - 1;
        let status = [0, 0, 1, 2][pick.upto(4) as usize];
        busy.set(status);
        let crash = pick.count();
        let fall = pick.count();
        let mut port =
            Emitter { crash: List { count: crash, last: pick.last(crash) }, fall: List { count: fall, last: pick.last(fall) } };
        let seed = pick.next();
        ee.write(rand_state(), 8, u128::from(seed));
        ee.set_u32(request + at::REASON, reason as u32);
        ee.set_u32(e + at::CRASH_COUNT, crash as u32);
        ee.set_u32(e + at::CRASH_LAST, port.crash.last as u32);
        ee.set_u32(e + at::FALL_COUNT, fall as u32);
        ee.set_u32(e + at::FALL_LAST, port.fall.last as u32);
        let flags = ee.read(e + at::FLAGS, 1) as u8;
        ee.write(e + at::FLAGS, 1, u128::from(flags & 0xfe | pick.upto(2) as u8));
        let gate = ee.read(e + at::PLAY_GATE, 1) as u8;
        ee.write(e + at::PLAY_GATE, 1, u128::from(gate & !2));

        let mut rng = Rng::from_state(seed);
        let got = port.wreck(reason, status != 0, &mut rng);
        ee.call(entry, &[u64::from(e), 0], &[], BUDGET);

        let ctx = format!("case {case}: reason {reason}, status {status}, lists {crash}/{fall}");
        assert_eq!(port, emitter_of(&ee), "{ctx}: the lists");
        assert_eq!(rng.state(), ee.read(rand_state(), 8) as u64, "{ctx}: the generator");
        let k = match got {
            None => 0,
            Some((Which::Crash, _)) => 1,
            Some((Which::Fall, _)) => 2,
        };
        seen[(reason + 1) as usize][k] += 1;
    }
    // Every reason seen idle and picking from its own list, or idle and picking nothing.
    for (i, s) in seen.iter().enumerate() {
        let reason = i as i32 - 1;
        let want = match reason {
            1..=7 | 12 => 1,
            8..=11 | 14 => 2,
            _ => 0,
        };
        assert!(s[want] > 100, "reason {reason}: {s:?}");
    }
    assert!(stopped.get() > 1_000, "the stop call ran {} times", stopped.get());
}

/// A wreck the retail emitter knows draws once, so the next number out of the generator is the
/// second one, not the first.
#[test]
fn a_retail_wreck_draws_once() {
    for reason in [1, 5, 6, 8, 12, 14] {
        let mut rng = Rng::from_state(0x1234_5678_9abc_def0);
        let mut e = Emitter::retail();
        assert!(e.wreck(reason, false, &mut rng).is_some());
        let mut one = Rng::from_state(0x1234_5678_9abc_def0);
        one.rand();
        assert_eq!(rng.state(), one.state(), "reason {reason}");
    }
    for reason in [0, 13, 15] {
        let mut rng = Rng::from_state(7);
        assert_eq!(Emitter::retail().wreck(reason, false, &mut rng), None);
        assert_eq!(rng.state(), Rng::from_state(7).state());
    }
}
