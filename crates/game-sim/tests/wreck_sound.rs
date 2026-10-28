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

// --- past the entry (S173) ---------------------------------------------------------------------

/// S173's capture of S109's reason-11 fall (journal 2026-10-28-s173-wreck-sound-draws, playback of
/// animandpump.p2m2, the frame counter's numbering): both emitters' gates, last picks and clip lengths.
struct Voices {
    rows: Vec<std::collections::HashMap<String, String>>,
}

impl Voices {
    fn load() -> Option<Voices> {
        let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
        let path = kit.ctx("capture").join("s173").join("voices.csv");
        let Ok(text) = std::fs::read_to_string(&path) else {
            skip(&format!("no {}: research/journal/2026-10-28-s173-wreck-sound-draws/capture/play.sh", path.display()));
            return None;
        };
        let mut lines = text.lines();
        let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
        let rows = lines.map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect()).collect();
        Some(Voices { rows })
    }

    fn f(row: &std::collections::HashMap<String, String>, name: &str) -> ps2_float::F {
        ps2_float::F(row[name].parse::<f32>().unwrap_or_else(|_| panic!("{name}: {}", row[name])))
    }

    fn i(row: &std::collections::HashMap<String, String>, name: &str) -> i64 {
        row[name].parse().unwrap_or_else(|_| panic!("{name}: {}", row[name]))
    }

    fn u(row: &std::collections::HashMap<String, String>, name: &str) -> u64 {
        row[name].parse().unwrap_or_else(|_| panic!("{name}: {}", row[name]))
    }

    fn v(row: &std::collections::HashMap<String, String>, name: &str) -> ps2_float::Vf {
        let [x, y, z] = ["x", "y", "z"].map(|c| Voices::f(row, &format!("{name}.{c}")).to_f32());
        ps2_float::Vf::new(x, y, z, 0.0)
    }

    /// The emitters as the original loaded them: its own clip lengths and thresholds.
    fn kit(&self) -> game_sim::wreck_sound::Kit {
        let r = &self.rows[0];
        let list = |name: &str, n: i64| (0..n).map(|k| Voices::f(r, &format!("{name}{k}.len"))).collect::<Vec<_>>();
        game_sim::wreck_sound::Kit {
            fall: list("r_fall", Voices::i(r, "r_fall_n")),
            crash: list("r_imp", Voices::i(r, "r_imp_n")),
            voice_min: Voices::f(r, "r_min"),
            clank: list("b_snd", Voices::i(r, "b_n")),
            clank_min: Voices::f(r, "b_min"),
        }
    }
}

/// The draw in `from..to`'s steps that a pick made with `pick` lands on `want`, if any.
fn drawn_by(from: u64, to: u64, want: i32, pick: impl Fn(&mut Rng) -> i32) -> Option<usize> {
    let mut s = Rng::from_state(from);
    for k in 0..10_000 {
        if s.state() == to {
            return None;
        }
        if pick(&mut s.clone()) == want {
            return Some(k);
        }
        s.rand();
    }
    None
}

/// **S109's reason-11 wreck draws for its sounds as many times as the original** (S173): the port's
/// two emitters, driven tick by tick off the original's own gates over the whole wreck — the bike's
/// entry and its wreck body's hits, the body's entry and its touches — pick on the same ticks the
/// original's last picks moved, and nowhere else: the `[Fall]` grunt at the body's entry, then at the
/// thrown body's first touchdown the clank and the `[IMPACT]` cry, three draws in all. The touches
/// and hits after it that pass every gate but the emitter's own idle stay quiet on the clip clocks.
/// Each pick's index is one a draw inside its tick gives.
#[test]
fn the_s109_wreck_draws_as_the_original() {
    use game_sim::wreck_sound::{Clocks, magnitude};
    let Some(cap) = Voices::load() else { return };
    let kit = cap.kit();
    assert_eq!((kit.fall.len(), kit.crash.len(), kit.clank.len()), (5, 5, 4), "the retail lists");
    let (mut e, mut c) = (Emitter::retail(), Clocks::default());
    let mut prev = &cap.rows[0];
    let (mut ours, mut theirs, mut quiet) = (Vec::new(), Vec::new(), 0);
    for row in &cap.rows[1..] {
        let fc = Voices::i(row, "frame_counter");
        let (from, to) = (Voices::u(prev, "rng_state"), Voices::u(row, "rng_state"));
        let mut rng = Rng::from_state(from);
        c.age(Voices::f(row, "dt"));
        // The player's entry on the reason's rise, the body actor's on its record's.
        let wreck = Voices::i(row, "wreck_reason") as i32;
        let entered = Voices::i(prev, "wreck_reason") == 0 && wreck != 0;
        let clank_at = c.clank_last;
        let mut picks = Vec::new();
        if entered {
            if let Some(i) = c.clank_entry(&kit, magnitude(Voices::v(row, "b_imp")), &mut rng) {
                picks.push(("clank", i));
            }
        }
        let body = Voices::i(row, "rec_reason") as i32;
        let body_entered = Voices::i(prev, "rec_reason") == 0 && body != 0;
        let (fall_at, crash_at) = (e.fall, e.crash);
        if body_entered {
            if let Some((_, i)) = c.voice_entry(&mut e, &kit, body, &mut rng) {
                picks.push(("fall", i));
            }
        } else {
            let gated = Voices::i(row, "b_gate_a") != 0;
            let impulse = magnitude(Voices::v(row, "b_imp"));
            if let Some(i) = c.clank_frame(&kit, gated, Voices::i(row, "b_gate_b") != 0, impulse, &mut rng) {
                picks.push(("clank", i));
            } else if gated && Voices::i(row, "b_gate_b") != 0 && impulse >= kit.clank_min {
                quiet += 1;
            }
            let (down, touching) = (body != 0 && body != 13, Voices::i(row, "rec_touch") != 0);
            let speed = magnitude(Voices::v(row, "rec_vel"));
            if let Some(i) = c.voice_frame(&mut e, &kit, down, touching, speed, &mut rng) {
                picks.push(("impact", i));
            } else if down && touching && kit.voice_min < speed {
                quiet += 1;
            }
        }
        // Where in the tick each pick draws is not known (the tick's other draws are not the
        // emitters'), so each list takes the original's pick, which must be one of the tick's draws
        // through the port's pick from the list as it was.
        for &(what, _) in &picks {
            let (want, ok) = match what {
                "fall" => {
                    let want = Voices::i(row, "r_fall_last") as i32;
                    e.fall.last = want;
                    (want, drawn_by(from, to, want, |r| { let mut l = fall_at; l.pick(r) }))
                }
                "impact" => {
                    let want = Voices::i(row, "r_imp_last") as i32;
                    e.crash.last = want;
                    (want, drawn_by(from, to, want, |r| { let mut l = crash_at; l.pick(r) }))
                }
                _ => {
                    let want = Voices::i(row, "b_last") as i32;
                    c.clank_last = want;
                    let n = kit.clank.len() as i32;
                    (want, drawn_by(from, to, want, |r| {
                        let k = r.index(n);
                        if k == clank_at { (k + 1) % n } else { k }
                    }))
                }
            };
            assert!(ok.is_some(), "{fc}: the original's {what} {want} is no draw of the tick");
            ours.push((fc, what));
        }
        // The original's picks: each moves its list's last, two or more sounds long.
        for (what, col) in [("clank", "b_last"), ("fall", "r_fall_last"), ("impact", "r_imp_last")] {
            if Voices::i(row, col) != Voices::i(prev, col) {
                theirs.push((fc, what));
            }
        }
        prev = row;
    }
    ours.sort();
    theirs.sort();
    assert_eq!(ours, theirs, "the two emitters' picks");
    assert_eq!(theirs.len(), 3, "{theirs:?}");
    assert!(quiet >= 7, "only {quiet} gated touches found the emitters busy");
}

/// The clank's pick draws even out of a list of one, where the voice's lists draw nothing; and the
/// clocks hold each emitter busy for its clip and the tail.
#[test]
fn the_clank_always_draws_and_the_clocks_hold() {
    use game_sim::wreck_sound::{Clocks, Kit as Sounds, TAIL};
    use ps2_float::F;
    let one = Sounds { clank: vec![F(1.0)], ..Sounds::default() };
    let mut c = Clocks::default();
    let mut rng = Rng::from_state(99);
    assert_eq!(c.clank_frame(&one, true, true, F(18.8), &mut rng), Some(0));
    let mut once = Rng::from_state(99);
    once.rand();
    assert_eq!(rng.state(), once.state(), "a list of one draws");
    assert_eq!(c.clank, F(1.0) + TAIL);
    // Busy until the clip and the tail have gone by, then idle.
    let dt = F(1.0 / 60.0);
    let mut ticks = 0;
    while c.clank_busy() {
        assert_eq!(c.clank_frame(&one, true, true, F(40.0), &mut rng), None);
        c.age(dt);
        ticks += 1;
    }
    assert!((83..=85).contains(&ticks), "{ticks} ticks");
    // Under MinImpulse, or untouched, or not wrecked: nothing, and no draw.
    let at = rng.state();
    assert_eq!(c.clank_frame(&one, true, true, F(18.7), &mut rng), None);
    assert_eq!(c.clank_frame(&one, true, false, F(30.0), &mut rng), None);
    assert_eq!(c.clank_frame(&one, false, true, F(30.0), &mut rng), None);
    assert_eq!(c.clank_entry(&one, F(18.7), &mut rng), None);
    assert_eq!(rng.state(), at);
    // The voice: over MinImpulse, strictly; down and touching; one busy clock for both lists.
    let kit = Sounds { fall: vec![F(0.5); 5], crash: vec![F(0.9); 5], ..Sounds::default() };
    let mut e = Emitter::retail();
    let mut c = Clocks::default();
    assert_eq!(c.voice_frame(&mut e, &kit, true, true, F(12.8), &mut rng), None);
    assert_eq!(c.voice_frame(&mut e, &kit, true, false, F(30.0), &mut rng), None);
    c.was = F(0.0);
    assert_eq!(c.voice_frame(&mut e, &kit, false, true, F(30.0), &mut rng), None);
    assert_eq!(rng.state(), at);
    assert!(c.voice_entry(&mut e, &kit, 11, &mut rng).is_some());
    assert_eq!(c.voice, F(0.5) + TAIL);
    assert_eq!(c.voice_entry(&mut e, &kit, 1, &mut rng), None, "busy with the fall");
    assert_eq!(c.voice_frame(&mut e, &kit, true, true, F(30.0), &mut rng), None, "busy with the fall");
}

/// The bike's `[Wreck]` emitter in the first recording's save state: the game manager's list to the
/// human's bike, whose emitter's frame is the clank's.
fn clank_emitter(ee: &Ee) -> Option<u32> {
    let frame = addr("bike_wreck_clank").unwrap();
    [&[0x34, 0x34, 0x8, 0x0][..], &[0x34, 0x34, 0x8, 0x8, 0x0], &[0x34, 0x34, 0x8, 0x8, 0x8, 0x0]].into_iter().find_map(|hops| {
        let mut a = ee.u32_at(addr("game_manager").unwrap());
        for &h in hops {
            a = ee.u32_at(a + h);
        }
        let e = ee.u32_at(a + 0x1498);
        (ee.u32_at(ee.u32_at(e + at::VTABLE) + 0x38) == frame).then_some(e)
    })
}

/// A speed near a threshold: on it, a bit either side, or anywhere up to three times it.
fn near(pick: &mut Pick, at: f32) -> f32 {
    match pick.upto(4) {
        0 => at,
        1 => f32::from_bits(at.to_bits() + 1),
        2 => f32::from_bits(at.to_bits() - 1),
        _ => (pick.upto(1 << 20) as f32 / (1 << 20) as f32) * 3.0 * at,
    }
}

/// A velocity of length about `len`, split over the three axes at random, signs too.
fn split(pick: &mut Pick, len: f32) -> [f32; 3] {
    let (a, b) = ((pick.upto(1000) as f32 / 1000.0) * std::f32::consts::TAU, (pick.upto(1000) as f32 / 1000.0) * 3.1);
    let v = [len * b.sin() * a.cos(), len * b.cos(), len * b.sin() * a.sin()];
    match pick.upto(4) {
        0 => [len, 0.0, 0.0],
        1 => [0.0, -len, 0.0],
        _ => v,
    }
}

/// **The voice's frame** (S173, `rider_voice_frame`) against the original over generated bodies: the
/// reason, the touching word, the velocity about `MinImpulse`, the emitter busy or not and the
/// `[IMPACT]` list generated. Its frame gate (`+0x23c`) the port has no use for is held open.
#[test]
fn the_voice_frame_matches_the_original() {
    use game_sim::wreck_sound::{Clocks, Kit as Sounds};
    use ps2_float::F;
    let Some(mut ee) = state() else { return };
    let e = emitter(&ee);
    let vt = ee.u32_at(e + at::VTABLE);
    let busy = Rc::new(Cell::new(0u64));
    let b = Rc::clone(&busy);
    ee.hooks.insert(ee.u32_at(vt + 0x90), Box::new(move |ee: &mut Ee| ee.set(2, b.get())));
    let played = Rc::new(Cell::new(0usize));
    let p = Rc::clone(&played);
    ee.hooks.insert(addr("rider_voice_play").unwrap(), Box::new(move |_| p.set(p.get() + 1)));
    ee.hooks.insert(addr("emitter_update").unwrap(), Box::new(|_| {}));
    let frame = addr("rider_voice_frame").unwrap();
    let record = ee.u32_at(e + at::EMITTER_REQUEST);
    let min = F(ee.f32_at(e + 0x238));
    ee.set_u32(e + 0x23c, 0);
    let mut pick = Pick(0x5eed_0173_0000_7015);
    let (mut cried, mut quiet) = (0, 0);
    for case in 0..CASES {
        let reason = pick.upto(18) as i32 - 1;
        let touching = [0u32, 1, 0x100][pick.upto(3) as usize];
        let status = [0, 0, 1, 2][pick.upto(4) as usize];
        busy.set(status);
        let len = near(&mut pick, min.to_f32());
        let v = split(&mut pick, len);
        let count = pick.count();
        let mut e2 = Emitter::retail();
        e2.crash = List { count, last: pick.last(count) };
        let seed = pick.next();
        ee.write(rand_state(), 8, u128::from(seed));
        ee.set_u32(record + at::REASON, reason as u32);
        ee.set_u32(record + 0x1bc, touching);
        for (k, x) in v.iter().enumerate() {
            ee.set_u32(record + 0x130 + 4 * k as u32, x.to_bits());
        }
        ee.set_u32(e + at::CRASH_COUNT, count as u32);
        ee.set_u32(e + at::CRASH_LAST, e2.crash.last as u32);

        let kit = Sounds { voice_min: min, ..Sounds::default() };
        let mut c = Clocks { voice: if status != 0 { F(1.0) } else { F(0.0) }, ..Clocks::default() };
        let mut rng = Rng::from_state(seed);
        let speed = game_sim::wreck_sound::magnitude(ps2_float::Vf::new(v[0], v[1], v[2], 0.0));
        let before = played.get();
        let got = c.voice_frame(&mut e2, &kit, reason != 0 && reason != 13, touching != 0, speed, &mut rng);
        ee.call(frame, &[u64::from(e)], &[], BUDGET);

        let ctx = format!("case {case}: reason {reason}, touching {touching}, status {status}, v {v:?}, list {count}");
        assert_eq!(got.is_some(), played.get() > before, "{ctx}: cried out");
        assert_eq!(e2.crash.last, ee.u32_at(e + at::CRASH_LAST) as i32, "{ctx}: the last pick");
        assert_eq!(rng.state(), ee.read(rand_state(), 8) as u64, "{ctx}: the generator");
        if got.is_some() { cried += 1 } else { quiet += 1 }
    }
    assert!(cried > 500 && quiet > 5_000, "cried {cried}, quiet {quiet}");
}

/// **The clank**, the bike emitter's frame (`bike_wreck_clank`) and the player's wreck entry's call
/// (`bike_wreck_entry_clank`), against the original over generated gates, impulses about
/// `MinImpulse`, last picks and lists of one to the retail four (the emitter's own sounds).
#[test]
fn the_clank_matches_the_original() {
    use game_sim::wreck_sound::{Clocks, Kit as Sounds};
    use ps2_float::F;
    let Some(mut ee) = state() else { return };
    let Some(e) = clank_emitter(&ee) else { panic!("no bike emitter whose frame is the clank's") };
    let vt = ee.u32_at(e + at::VTABLE);
    let busy = Rc::new(Cell::new(0u64));
    let b = Rc::clone(&busy);
    ee.hooks.insert(ee.u32_at(vt + 0x90), Box::new(move |ee: &mut Ee| ee.set(2, b.get())));
    ee.hooks.insert(ee.u32_at(vt + 0x20), Box::new(|_| {}));
    let played = Rc::new(Cell::new(0usize));
    let p = Rc::clone(&played);
    ee.hooks.insert(addr("wreck_clank_play").unwrap(), Box::new(move |_| p.set(p.get() + 1)));
    ee.hooks.insert(addr("emitter_update").unwrap(), Box::new(|_| {}));
    let (frame, entry) = (addr("bike_wreck_clank").unwrap(), addr("bike_wreck_entry_clank").unwrap());
    let (wrecked, touched, impulse) = (ee.u32_at(e + 0x200), ee.u32_at(e + 0x204), ee.u32_at(e + 0x224));
    let min = F(ee.f32_at(e + 0x228));
    let full = ee.u32_at(e + 0x208) as i32;
    assert_eq!(full, 4, "the 125's four clanks");
    let mut pick = Pick(0x5eed_0173_c1a4_0000);
    let mut seen = [0usize; 4];
    for case in 0..CASES {
        let at_entry = pick.upto(3) == 0;
        let (w, t) = ([0u32, 1, 1, 11][pick.upto(4) as usize], [0u32, 1, 1][pick.upto(3) as usize]);
        let status = [0, 0, 1, 2][pick.upto(4) as usize];
        busy.set(status);
        let len = near(&mut pick, min.to_f32());
        let v = split(&mut pick, len);
        let n = pick.upto(full as u64) as i32 + 1;
        let last = match pick.upto(3) {
            0 => -1,
            _ => pick.upto(n as u64 + 1) as i32,
        };
        let seed = pick.next();
        ee.write(rand_state(), 8, u128::from(seed));
        ee.set_u32(wrecked, w);
        ee.set_u32(touched, t);
        for (k, x) in v.iter().enumerate() {
            ee.set_u32(impulse + 4 * k as u32, x.to_bits());
        }
        ee.set_u32(e + 0x208, n as u32);
        ee.set_u32(e + 0x220, last as u32);

        let kit = Sounds { clank: vec![F(0.0); n as usize], clank_min: min, ..Sounds::default() };
        let mut c = Clocks { clank: if status != 0 { F(1.0) } else { F(0.0) }, clank_last: last, ..Clocks::default() };
        let mut rng = Rng::from_state(seed);
        let peak = game_sim::wreck_sound::magnitude(ps2_float::Vf::new(v[0], v[1], v[2], 0.0));
        let before = played.get();
        let got = if at_entry {
            ee.call(entry, &[u64::from(e), 0], &[], BUDGET);
            c.clank_entry(&kit, peak, &mut rng)
        } else {
            ee.call(frame, &[u64::from(e)], &[], BUDGET);
            c.clank_frame(&kit, w != 0, t != 0, peak, &mut rng)
        };

        let ctx = format!("case {case}: entry {at_entry}, gates {w}/{t}, status {status}, v {v:?}, list {n}, last {last}");
        assert_eq!(got.is_some(), played.get() > before, "{ctx}: clanked");
        assert_eq!(c.clank_last, ee.u32_at(e + 0x220) as i32, "{ctx}: the last pick");
        assert_eq!(rng.state(), ee.read(rand_state(), 8) as u64, "{ctx}: the generator");
        seen[usize::from(at_entry) * 2 + usize::from(got.is_some())] += 1;
    }
    assert!(seen.iter().all(|&s| s > 500), "{seen:?}");
}
