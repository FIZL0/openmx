//! The race's gates (S13a), proved against the original in the interpreter.
//!
//! 1. **The crossing test**: the original's own function, called on 20,000 generated gates and steps,
//!    against [`Gate::crossed`]. The inputs are aimed at each of its branches — the live flag, the
//!    reach, a step that does not move, the plane's fraction at and past its ends, the plane-only gate,
//!    a gate with no width, the one-sided half, the side and height edges and the way the step goes —
//!    with stray words (denormals, the FPU's Inf/NaN patterns, signed zeros) mixed into every field.
//! 2. **The start-gate watch**: the race manager's whole tick, run on a race object built in free RAM
//!    with 1–3 score cards of 1–2 entities each, its entity messages hooked and the score manager's tick
//!    after it stubbed; 10,000 ticks against [`Gate::watch`], compared on every card's two positions,
//!    its penalty, every push message and the generator's state.
//! 3. **The activity's reset**: the supercross activity's own reset slot on 10,000 generated activities
//!    (clip counts, the last picks, drop times, the intro seen or not, the generator anywhere), its
//!    presentation calls landing on hooks, against [`Supercross::reset`] on every field it writes.
//! 4. **The grid**: its grid slot on 5,000 generated starts (1–8 riders, cards in any order and some
//!    with no rider, a start gate or the track's start point), against [`Supercross::shuffle_lanes`],
//!    [`Supercross::grid_spot`] (what the ground query and the entity are handed) and
//!    [`Supercross::starting_place`].
//! 5. **The sx01 image**: S196's RAM image of an sx01 race before its start, read as the port reads
//!    it — the class, the lane table, the start's timeline as a reset left it, and the one rider's lane,
//!    place and spot on the grid.

use game_sim::race::{Crossing, Gate, GridStart, LANES, Seen, Supercross, flags};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip, skip};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

const BUDGET: u64 = 100_000;

const GATE: u32 = 0x0180_0000;
const FROM: u32 = 0x0180_1000;
const TO: u32 = 0x0180_1010;
const RACE: u32 = 0x0181_0000;
const CARDS: u32 = 0x0182_0000;
const CARD_SIZE: u32 = 0x400;
const ENTITIES: u32 = 0x0183_0000;
const ENTITY_SIZE: u32 = 0x100;
const OBJECTS: u32 = 0x0184_0000;
const OBJECT_SIZE: u32 = 0x100;
const VTABLE: u32 = 0x0185_0000;
/// Not code: the entities' message slot lands here and is hooked.
const HOOK_MESSAGE: u32 = 0x0185_1000;

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 16) % n
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    /// Now and then a word no gate holds: a denormal, an Inf/NaN pattern, a signed zero, any bits.
    fn stray(&mut self, x: f32) -> f32 {
        match self.below(40) {
            0 => f32::from_bits(self.next() as u32 & 0x807f_ffff), // a denormal or zero
            1 => f32::from_bits(0x7f80_0000 | (self.next() as u32 & 0x807f_ffff)), // Inf/NaN
            2 => -0.0,
            3 => 0.0,
            4 => f32::from_bits(self.next() as u32),
            _ => x,
        }
    }
    fn unit_vector(&mut self) -> [f32; 3] {
        loop {
            let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l > 0.1 && l <= 1.0 {
                return v.map(|c| c / l);
            }
        }
    }
    fn quad(&mut self, v: [f32; 3]) -> Vf {
        let w = self.range(-2.0, 2.0);
        Vf([self.stray(v[0]), self.stray(v[1]), self.stray(v[2]), self.stray(w)])
    }
}

/// A gate standing somewhere near the origin, with its fields as the original's objects hold them
/// (now and then not).
fn gate(p: &mut Pick) -> Gate {
    let normal = match p.below(3) {
        0 => [0.0, 0.0, 1.0],
        1 => [1.0, 0.0, 0.0],
        _ => {
            let a = p.range(0.0, 6.3);
            [a.sin(), 0.0, a.cos()]
        }
    };
    let across = match p.below(4) {
        0 => p.unit_vector(),
        _ => [normal[2], if p.below(4) == 0 { p.range(-0.3, 0.3) } else { 0.0 }, -normal[0]],
    };
    let centre = [p.range(-50.0, 50.0), p.range(-5.0, 5.0), p.range(-50.0, 50.0)];
    let mut flag_bits = (p.next() as u32) & 0x1f;
    if p.below(8) != 0 {
        flag_bits |= flags::CLOSED;
    }
    if p.below(3) != 0 {
        flag_bits &= !flags::PLANE;
    }
    let width = match p.below(6) {
        0 => -p.range(0.0, 10.0),
        1 => 0.0,
        _ => p.range(1.0, 60.0),
    };
    let height = match p.below(5) {
        0 => -p.range(0.0, 5.0),
        _ => p.range(0.5, 20.0),
    };
    let reach_sq = match p.below(4) {
        0 => 0.0,
        1 => -p.range(0.0, 100.0),
        _ => p.range(1.0, 10_000.0),
    };
    Gate {
        flags: flag_bits | ((p.next() as u32) & 0xffff_ff00 & if p.below(4) == 0 { !0 } else { 0 }),
        centre: p.quad(centre),
        normal: p.quad(normal),
        across: p.quad(across),
        width: F(p.stray(width)),
        height: F(p.stray(height)),
        reach_sq: F(p.stray(reach_sq)),
    }
}

/// A step `(from, to)` near the gate: mostly through or past its plane, at its edges, or nowhere.
fn step(p: &mut Pick, g: &Gate) -> (Vf, Vf) {
    let c = [g.centre.x(), g.centre.y(), g.centre.z()].map(|v| if v.is_finite() && v.abs() < 1e6 { v } else { 0.0 });
    let n = [g.normal.x(), g.normal.y(), g.normal.z()].map(|v| if v.is_finite() && v.abs() < 10.0 { v } else { 0.0 });
    let a = [g.across.x(), g.across.y(), g.across.z()].map(|v| if v.is_finite() && v.abs() < 10.0 { v } else { 0.0 });
    let w = if g.width.0.is_finite() && g.width.0.abs() < 1e4 { g.width.0 } else { 10.0 };
    let h = if g.height.0.is_finite() && g.height.0.abs() < 1e4 { g.height.0 } else { 5.0 };
    // A point on the gate's plane: across the width (often at its edges), up the height.
    let side = match p.below(6) {
        0 => [-1.0, 1.0, -1.0001, 1.0001, 0.9999, -0.9999, 0.0][p.below(7) as usize],
        1 => p.range(-3.0, 3.0),
        _ => p.range(-1.0, 1.0),
    } * w
        * 0.5;
    let up = match p.below(5) {
        0 => h + [0.0, 0.001, -0.001, 1.0][p.below(4) as usize],
        _ => p.range(-2.0, h.max(0.0) + 2.0),
    };
    let on = [0, 1, 2].map(|i| c[i] + a[i] * side + if i == 1 { up } else { 0.0 });
    // Where along the step that point falls: inside 0..1 crosses, at the ends, or past them.
    let t = match p.below(6) {
        0 => [0.0, 1.0, -0.0001, 1.0001, 0.5][p.below(5) as usize],
        1 => p.range(-2.0, 3.0),
        _ => p.range(0.0, 1.0),
    };
    let len = match p.below(10) {
        0 => 0.0,
        1 => p.range(0.0, 0.01),
        2 => p.range(10.0, 300.0),
        _ => p.range(0.1, 8.0),
    };
    let wobble = [p.range(-0.5, 0.5), p.range(-0.5, 0.5), p.range(-0.5, 0.5)];
    let sign = if p.below(4) == 0 { -1.0 } else { 1.0 };
    let dir = [0, 1, 2].map(|i| (n[i] + wobble[i]) * sign);
    // to + (from - to)·t ≈ on, along `dir`.
    let to = [0, 1, 2].map(|i| on[i] - dir[i] * len * t);
    let from = [0, 1, 2].map(|i| to[i] + dir[i] * len);
    if p.below(12) == 0 {
        let far = [p.range(-500.0, 500.0), p.range(-50.0, 50.0), p.range(-500.0, 500.0)];
        return (p.quad(far), p.quad(to));
    }
    (p.quad(from), p.quad(to))
}

fn put_quad(ee: &mut Ee, at: u32, v: Vf) {
    for (k, b) in v.to_bits().iter().enumerate() {
        ee.set_u32(at + 4 * k as u32, *b);
    }
}

/// A quadword as it sits in RAM, raw: a stray pattern copied through is compared as itself.
fn quad(ee: &Ee, at: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|o| f32::from_bits(ee.u32_at(at + o))))
}

fn put_gate(ee: &mut Ee, at: u32, g: &Gate) {
    ee.set_u32(at + 0x30, g.flags);
    put_quad(ee, at + 0x40, g.centre);
    put_quad(ee, at + 0x50, g.normal);
    put_quad(ee, at + 0x60, g.across);
    ee.set_u32(at + 0x70, g.width.0.to_bits());
    ee.set_u32(at + 0x74, g.height.0.to_bits());
    ee.set_u32(at + 0x7c, g.reach_sq.0.to_bits());
}

#[test]
fn the_crossing_test_matches_over_twenty_thousand_steps() {
    let Some(mut ee) = game_or_skip() else { return };
    let crossed = match addr("start_gate_crossed") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x5ce1_3a00_9e37_79b9);
    let mut seen = [0usize; 5];
    for case in 0..20_000 {
        let g = gate(&mut p);
        let (from, to) = step(&mut p, &g);
        put_gate(&mut ee, GATE, &g);
        put_quad(&mut ee, FROM, from);
        put_quad(&mut ee, TO, to);
        ee.call(crossed, &[u64::from(GATE), u64::from(FROM), u64::from(TO)], &[], BUDGET);
        let want = ee.v0() as u32;
        let got = g.crossed(from, to);
        assert_eq!(got as u32, want, "case {case}: {g:?} from {from:?} to {to:?}");
        seen[want.min(4) as usize] += 1;
    }
    // Every answer comes up often enough to mean something.
    for (code, n) in seen.iter().enumerate().filter(|&(c, _)| c != 3) {
        assert!(*n > 500, "answer {code} came up only {n} times in 20,000: {seen:?}");
    }
}

/// One card of the watch's race: (where it was, its entities' (position, facing)).
struct Card {
    seen: Seen,
    penalty: F,
    entities: Vec<(Vf, Vf)>,
}

#[test]
fn the_start_gate_watch_matches_the_race_managers_tick() {
    let Some(mut ee) = game_or_skip() else { return };
    let (tick, score_tick, rand_state) =
        match (addr("race_manager_tick"), addr("score_manager_tick"), addr("rand_state")) {
            (Ok(a), Ok(b), Ok(c)) => (a, b, c),
            (Err(why), _, _) | (_, Err(why), _) | (_, _, Err(why)) => return skip(&why),
        };
    // Message 1 hands back the entity's object; message 2 is the push, recorded.
    let pushes: Rc<RefCell<Vec<(u32, Vf)>>> = Rc::default();
    let seen_pushes = pushes.clone();
    ee.hooks.insert(
        HOOK_MESSAGE,
        Box::new(move |ee| {
            let (entity, message, out) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
            let k = (entity - ENTITIES) / ENTITY_SIZE;
            match message {
                1 => ee.set_u32(out, OBJECTS + k * OBJECT_SIZE),
                2 => seen_pushes.borrow_mut().push((entity, quad(ee, out))),
                m => panic!("message {m}"),
            }
        }),
    );
    ee.hooks.insert(score_tick, Box::new(|_| {}));
    ee.set_u32(VTABLE + 0x94, HOOK_MESSAGE);

    let mut p = Pick(0x13a0_77f1_2b3c_d5e9);
    let mut caught = 0;
    for case in 0..10_000 {
        let g = gate(&mut p);
        let state = p.next();
        let mut cards: Vec<Card> = (0..1 + p.below(3))
            .map(|_| {
                let last = step(&mut p, &g).1;
                Card {
                    seen: Seen { now: last, last: p.quad([1.0, 2.0, 3.0]) },
                    penalty: F(p.range(-1.0, 2.0)),
                    entities: (0..1 + p.below(2))
                        .map(|_| {
                            let to = if p.below(2) == 0 { step(&mut p, &g).0 } else { last };
                            let facing = p.unit_vector();
                            (to, p.quad(facing))
                        })
                        .collect(),
                }
            })
            .collect();
        // The step each card's first entity takes is from its last `now`: put `now` where a generated
        // step starts so that some of them cross.
        for c in cards.iter_mut() {
            if p.below(2) == 0 {
                let (from, to) = step(&mut p, &g);
                c.seen.now = to;
                c.entities[0].0 = from;
            }
        }

        put_gate(&mut ee, GATE, &g);
        ee.set_u32(RACE + 0x4c0, 0);
        ee.set_u32(RACE + 0x3dc, GATE);
        ee.set_u32(RACE + 0x410, 0);
        ee.set_u32(RACE + 0x470, 0);
        ee.set_u32(RACE + 0x84, CARDS);
        let mut e = 0u32;
        for (k, c) in cards.iter().enumerate() {
            let at = CARDS + k as u32 * CARD_SIZE;
            put_quad(&mut ee, at + 0x60, c.seen.now);
            put_quad(&mut ee, at + 0x70, c.seen.last);
            ee.set_u32(at + 0x1c0, c.penalty.0.to_bits());
            ee.set_u32(at + 0x1c8, if k + 1 < cards.len() { at + CARD_SIZE } else { 0 });
            ee.set_u32(at + 4, ENTITIES + e * ENTITY_SIZE);
            for (j, (pos, facing)) in c.entities.iter().enumerate() {
                let ent = ENTITIES + e * ENTITY_SIZE;
                ee.set_u32(ent + 0x20, VTABLE);
                ee.set_u32(ent + 0x1c, if j + 1 < c.entities.len() { ent + ENTITY_SIZE } else { 0 });
                put_quad(&mut ee, OBJECTS + e * OBJECT_SIZE + 0x20, *facing);
                put_quad(&mut ee, OBJECTS + e * OBJECT_SIZE + 0x30, *pos);
                e += 1;
            }
        }
        ee.write(rand_state, 8, u128::from(state));
        pushes.borrow_mut().clear();
        ee.call(tick, &[u64::from(RACE)], &[0.033_333_335], BUDGET);

        let mut rng = Rng::from_state(state);
        let mut want_pushes = Vec::new();
        let mut e = 0u32;
        for (k, c) in cards.iter_mut().enumerate() {
            for (pos, facing) in &c.entities {
                if let Some(hit) = g.watch(&mut c.seen, *pos, *facing, &mut rng) {
                    want_pushes.push((ENTITIES + e * ENTITY_SIZE, hit.push));
                    c.penalty = hit.penalty;
                    caught += 1;
                }
                e += 1;
            }
            let at = CARDS + k as u32 * CARD_SIZE;
            let what = format!("case {case} card {k}: {g:?}");
            assert_eq!(quad(&ee, at + 0x60).to_bits(), c.seen.now.to_bits(), "{what}: now");
            assert_eq!(quad(&ee, at + 0x70).to_bits(), c.seen.last.to_bits(), "{what}: last");
            assert_eq!(ee.u32_at(at + 0x1c0), c.penalty.0.to_bits(), "{what}: the penalty");
        }
        let got: Vec<(u32, [u32; 4])> = pushes.borrow().iter().map(|(a, v)| (*a, v.to_bits())).collect();
        let want: Vec<(u32, [u32; 4])> = want_pushes.iter().map(|(a, v)| (*a, v.to_bits())).collect();
        assert_eq!(got, want, "case {case}: the pushes");
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "case {case}: the generator");
    }
    assert!(caught > 500, "only {caught} riders caught in 10,000 ticks");
}

// --- the activity (S13a) ---------------------------------------------------------------------------

const ACT: u32 = 0x0190_0000;
const ACT_VT: u32 = 0x0190_1000;
const CLIP: u32 = 0x0190_2000;
const CLIP_VT: u32 = 0x0190_2100;
const CLIPS: u32 = 0x0190_3000;
const DROPS: u32 = 0x0190_4000;
const APP: u32 = 0x0190_5000;
const SETTINGS: u32 = 0x0190_6000;
const VIEWPORTS: u32 = 0x0190_6100;
const START_GATE: u32 = 0x0190_7000;
const SCENE: u32 = 0x0190_8000;
const GRID_OBJ: u32 = 0x0190_9000;
const GRID_GATE: u32 = 0x0190_9100;
const TRACK: u32 = 0x0190_a000;
const START: u32 = 0x0190_b000;
const WORLD: u32 = 0x0190_c000;
const ENT_VT: u32 = 0x0190_d000;
const GRID_CARDS: u32 = 0x0191_0000;
const GRID_ENTITIES: u32 = 0x0192_0000;
/// Not code: presentation slots land here and do nothing; the second answers "is this clip playing".
const HOOK_NOTHING: u32 = 0x0190_f000;
const HOOK_PLAYING: u32 = 0x0190_f100;
const HOOK_PLACE: u32 = 0x0190_f200;

fn f32_at(ee: &Ee, at: u32) -> F {
    F(f32::from_bits(ee.u32_at(at)))
}

fn put_f(ee: &mut Ee, at: u32, x: F) {
    ee.set_u32(at, x.0.to_bits());
}

fn addrs<const N: usize>(names: [&str; N]) -> Result<[u32; N], String> {
    let mut out = [0; N];
    for (o, n) in out.iter_mut().zip(names) {
        *o = addr(n)?;
    }
    Ok(out)
}

#[test]
fn the_reset_matches_the_activitys_reset_slot() {
    let Some(mut ee) = game_or_skip() else { return };
    let [reset, app, seeded, rand_state] =
        match addrs(["activity_supercross_reset", "app_pointer", "rand_seeded", "rand_state"]) {
            Ok(a) => a,
            Err(why) => return skip(&why),
        };
    let playing = Rc::new(Cell::new(0u64));
    let answer = playing.clone();
    ee.hooks.insert(HOOK_NOTHING, Box::new(|_| {}));
    ee.hooks.insert(HOOK_PLAYING, Box::new(move |ee| ee.set(2, answer.get())));
    // The base reset calls two slots of the activity's own vtable; every clip, overlay and the sound
    // manager is the one object, whose slots do nothing.
    ee.set_u32(ACT_VT + 0x28, HOOK_NOTHING);
    ee.set_u32(ACT_VT + 0x2c, HOOK_NOTHING);
    ee.set_u32(CLIP + 8, CLIP_VT);
    for slot in [0x1c, 0x28, 0x8c, 0x90] {
        ee.set_u32(CLIP_VT + slot, HOOK_NOTHING);
    }
    ee.set_u32(CLIP_VT + 0x20, HOOK_PLAYING);
    ee.set_u32(app, APP);
    ee.set_u32(APP + 0x400, CLIP - 0x44 + 0x100);
    ee.set_u32(CLIP + 0x100, CLIP);
    ee.set_u32(APP + 0x3d4, SETTINGS);
    ee.set_u32(SETTINGS + 8, VIEWPORTS);
    // Not one, two or no viewport mode the HUD has overlays for: the reset's HUD lookups are skipped.
    ee.set_u32(VIEWPORTS + 4, 3);
    ee.set_u32(seeded, 1);
    // A last pick of -1 (none yet) asks the slot in front of the table whether it is playing.
    for k in 0..64 {
        ee.set_u32(CLIPS - 4 + 4 * k, CLIP);
    }

    let mut p = Pick(0x7e57_ac71_5a5e_0001);
    let (mut intros, mut steps) = (0, 0);
    for case in 0..10_000 {
        let mut s = Supercross::new();
        s.drop_times = (0..1 + p.below(16)).map(|_| {
            let d = p.range(0.0, 4.0);
            F(p.stray(d))
        }).collect();
        let calls = s.drop_times.len() as i32;
        s.call = match p.below(6) {
            0 => -1,
            1 => calls,
            _ => p.below(calls as u64) as i32,
        };
        s.intro_clips = 1 + p.below(12) as i32;
        s.intro = p.below(s.intro_clips as u64) as i32;
        s.intro_seen = if p.below(4) == 0 { 1 + p.below(3) as u32 } else { 0 };
        s.intro_queued = p.below(2) as u32;
        s.hud_stage = p.below(9) as i32;
        s.hud_armed = p.below(2) as u32;
        s.fresh = p.below(2) as u32;
        s.seat_flags = [p.below(2) as u32, p.below(2) as u32];
        for x in [&mut s.start_delay, &mut s.start_cue, &mut s.hud_fade, &mut s.countdown] {
            *x = F(p.range(-10.0, 10.0));
        }
        for x in [&mut s.lights_on, &mut s.lights_off, &mut s.warn_on, &mut s.warn_off] {
            *x = F(p.range(-10.0, 10.0));
        }
        let mut gate = Gate {
            flags: p.next() as u32,
            centre: Vf::ZERO,
            normal: Vf::ZERO,
            across: Vf::ZERO,
            width: F::ZERO,
            height: F::ZERO,
            reach_sq: F::ZERO,
        };
        let state = p.next();
        playing.set(p.below(2));
        let sound_on = p.below(2) as u32;

        ee.set_u32(ACT + 0x44, ACT_VT);
        ee.set_u32(ACT + 0x3a8, 0);
        ee.set_u32(ACT + 0x3ac, 0);
        ee.set_u32(ACT + 0x3e0, 0);
        ee.set_u32(ACT + 0x3f8, 0);
        ee.set_u32(ACT + 0x414, 0);
        ee.set_u32(ACT + 0x3dc, START_GATE);
        ee.set_u32(START_GATE + 0x30, gate.flags);
        ee.set_u32(APP + 0x570, sound_on);
        put_f(&mut ee, ACT + 0x10c, s.start_delay);
        put_f(&mut ee, ACT + 0x110, s.start_cue);
        put_f(&mut ee, ACT + 0x410, s.hud_fade);
        ee.set_u32(ACT + 0x438, s.intro_clips as u32);
        ee.set_u32(ACT + 0x440, CLIPS);
        for k in 0..9 {
            ee.set_u32(ACT + 0x44c + 4 * k, CLIP);
        }
        ee.set_u32(ACT + 0x470, s.hud_stage as u32);
        ee.set_u32(ACT + 0x474, s.hud_armed);
        ee.set_u32(ACT + 0x478, s.intro as u32);
        ee.set_u32(ACT + 0x47c, calls as u32);
        ee.set_u32(ACT + 0x484, CLIPS);
        ee.set_u32(ACT + 0x49c, DROPS);
        for (k, d) in s.drop_times.iter().enumerate() {
            put_f(&mut ee, DROPS + 4 * k as u32, *d);
        }
        ee.set_u32(ACT + 0x4ac, s.call as u32);
        for (o, x) in [(0x4b0, s.lights_on), (0x4b4, s.lights_off), (0x4b8, s.warn_on), (0x4bc, s.warn_off)] {
            put_f(&mut ee, ACT + o, x);
        }
        put_f(&mut ee, ACT + 0x4c0, s.countdown);
        ee.set_u32(ACT + 0x4c4, s.intro_seen);
        ee.set_u32(ACT + 0x4c8, s.intro_queued);
        ee.set_u32(ACT + 0x4cc, s.fresh);
        ee.set_u32(ACT + 0x4d0, s.seat_flags[0]);
        ee.set_u32(ACT + 0x4d4, s.seat_flags[1]);
        ee.write(rand_state, 8, u128::from(state));
        ee.call(reset, &[u64::from(ACT)], &[], BUDGET);

        let last_call = s.call;
        let mut rng = Rng::from_state(state);
        s.reset(&mut gate, &mut rng);
        if s.intro_seen == 0 {
            intros += 1;
        }
        if Rng::from_state(state).index(calls) == last_call {
            steps += 1;
        }
        let what = format!("case {case}");
        assert_eq!(ee.u32_at(START_GATE + 0x30), gate.flags, "{what}: the gate's flags");
        assert_eq!(ee.u32_at(ACT + 0x4ac) as i32, s.call, "{what}: the start call");
        assert_eq!(ee.u32_at(ACT + 0x478) as i32, s.intro, "{what}: the intro");
        for (o, x, name) in [
            (0x10c, s.start_delay, "start delay"),
            (0x110, s.start_cue, "cue"),
            (0x410, s.hud_fade, "hud fade"),
            (0x4b0, s.lights_on, "lights on"),
            (0x4b4, s.lights_off, "lights off"),
            (0x4b8, s.warn_on, "warning on"),
            (0x4bc, s.warn_off, "warning off"),
            (0x4c0, s.countdown, "countdown"),
        ] {
            assert_eq!(ee.u32_at(ACT + o), x.0.to_bits(), "{what}: {name}");
        }
        for (o, x, name) in [
            (0x470, s.hud_stage as u32, "hud stage"),
            (0x474, s.hud_armed, "hud armed"),
            (0x4c8, s.intro_queued, "intro queued"),
            (0x4cc, s.fresh, "fresh"),
            (0x4d0, s.seat_flags[0], "seat 0"),
            (0x4d4, s.seat_flags[1], "seat 1"),
        ] {
            assert_eq!(ee.u32_at(ACT + o), x, "{what}: {name}");
        }
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "{what}: the generator");
    }
    assert!(intros > 5_000 && intros < 9_000, "{intros} resets played the intro");
    assert!(steps > 300, "the pick stepped past the last call only {steps} times");
}

/// One card of a generated grid: its index, and whether it has a rider.
struct GridCard {
    id: i32,
    ridden: bool,
}

#[test]
fn the_grid_matches_the_activitys_grid_slot() {
    let Some(mut ee) = game_or_skip() else { return };
    let [grid, app, all_ai, rand_state, ground, interface, base, overlays] = match addrs([
        "activity_supercross_grid",
        "app_pointer",
        "all_ai",
        "rand_state",
        "ground_query",
        "interface_id",
        "activity_grid_base",
        "race_overlays_bind",
    ]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let grounded: Rc<RefCell<Vec<Vf>>> = Rc::default();
    let placed: Rc<RefCell<Vec<(u32, Vf, Vf)>>> = Rc::default();
    let (g, pl) = (grounded.clone(), placed.clone());
    ee.hooks.insert(HOOK_NOTHING, Box::new(|_| {}));
    ee.hooks.insert(ground, Box::new(move |ee| g.borrow_mut().push(quad(ee, ee.r(5) as u32))));
    ee.hooks.insert(
        HOOK_PLACE,
        Box::new(move |ee| pl.borrow_mut().push((ee.r(4) as u32, quad(ee, ee.r(5) as u32), quad(ee, ee.r(6) as u32)))),
    );
    ee.hooks.insert(interface, Box::new(|ee| ee.set_u32(ee.r(6) as u32, 7)));
    ee.hooks.insert(base, Box::new(|_| {}));
    ee.hooks.insert(overlays, Box::new(|_| {}));
    ee.set_u32(app, APP);
    ee.set_u32(APP + 0x3cc, SCENE);
    ee.set_u32(SCENE + 0x74, 0);
    ee.set_u32(all_ai, 0);
    ee.set_u32(ENT_VT + 0x8c, HOOK_PLACE);
    ee.set_u32(ENT_VT + 0x88, HOOK_NOTHING);
    ee.set_u32(GRID_OBJ + 0x58, GRID_GATE);
    ee.set_u32(TRACK + 0x348, START);
    ee.set_u32(TRACK + 0x428, WORLD);

    let table = Supercross::lane_table();
    let mut p = Pick(0x6a1d_0005_77ac_e5e1);
    let mut gated = 0;
    for case in 0..5_000 {
        let riders = 1 + p.below(LANES as u64) as usize;
        let mut ids: Vec<i32> = (0..riders as i32).collect();
        for k in (1..riders).rev() {
            ids.swap(k, p.below(k as u64 + 1) as usize);
        }
        let cards: Vec<GridCard> = ids.iter().map(|&id| GridCard { id, ridden: p.below(6) != 0 }).collect();
        let mut s = Supercross::new();
        s.lane_offsets = if p.below(3) == 0 { core::array::from_fn(|_| {
                let x = p.range(-1.0, 1.0);
                F(p.stray(x))
            }) } else { table };
        s.lanes = core::array::from_fn(|_| p.below(8) as i32);
        let normal = p.unit_vector();
        let start = if p.below(4) != 0 {
            gated += 1;
            let centre = [p.range(-500.0, 500.0), p.range(-50.0, 150.0), p.range(-500.0, 500.0)];
            let width = match p.below(5) {
                0 => -p.range(0.0, 10.0),
                _ => p.range(1.0, 150.0),
            };
            GridStart::Gate { centre: p.quad(centre), normal: p.quad(normal), width: F(p.stray(width)) }
        } else {
            let at = [p.range(-500.0, 500.0), p.range(-50.0, 150.0), p.range(-500.0, 500.0)];
            GridStart::Point { at: p.quad(at), facing: p.quad(normal) }
        };
        let state = p.next();

        match start {
            GridStart::Gate { centre, normal, width } => {
                ee.set_u32(ACT + 0xb8, GRID_OBJ);
                put_quad(&mut ee, GRID_GATE + 0x40, centre);
                put_quad(&mut ee, GRID_GATE + 0x50, normal);
                put_f(&mut ee, GRID_GATE + 0x70, width);
            }
            GridStart::Point { at, facing } => {
                ee.set_u32(ACT + 0xb8, 0);
                put_quad(&mut ee, START, at);
                put_quad(&mut ee, START + 0x10, facing);
            }
        }
        ee.set_u32(ACT + 0x14, 0);
        ee.set_u32(ACT + 0x8c, riders as u32);
        ee.set_u32(ACT + 0x180, TRACK);
        for k in 0..LANES as u32 {
            put_f(&mut ee, ACT + 0x3bc + 4 * k, s.lane_offsets[k as usize]);
            ee.set_u32(ACT + 0x418 + 4 * k, s.lanes[k as usize] as u32);
        }
        ee.set_u32(ACT + 0x84, GRID_CARDS);
        for (k, c) in cards.iter().enumerate() {
            let at = GRID_CARDS + k as u32 * CARD_SIZE;
            let ent = GRID_ENTITIES + k as u32 * ENTITY_SIZE;
            ee.set_u32(at, c.id as u32);
            ee.set_u32(at + 4, if c.ridden { ent } else { 0 });
            ee.set_u32(at + 0x2c, 0xdead);
            ee.set_u32(at + 0x1c8, if k + 1 < cards.len() { at + CARD_SIZE } else { 0 });
            ee.set_u32(ent + 0x20, ENT_VT);
        }
        ee.write(rand_state, 8, u128::from(state));
        grounded.borrow_mut().clear();
        placed.borrow_mut().clear();
        ee.call(grid, &[u64::from(ACT)], &[], BUDGET);

        let mut rng = Rng::from_state(state);
        s.shuffle_lanes(riders, false, &mut rng);
        let what = format!("case {case}: {riders} riders, {start:?}");
        for k in 0..riders as u32 {
            assert_eq!(ee.u32_at(ACT + 0x418 + 4 * k) as i32, s.lanes[k as usize], "{what}: lane {k}");
        }
        let mut want_ground = Vec::new();
        let mut want_placed = Vec::new();
        for (k, c) in cards.iter().enumerate() {
            let at = GRID_CARDS + k as u32 * CARD_SIZE;
            assert_eq!(ee.u32_at(at + 0x2c) as i32, s.starting_place(c.id), "{what}: card {k}'s place");
            if c.ridden {
                let (spot, facing) = s.grid_spot(start, c.id, riders as i32);
                want_ground.push(spot.to_bits());
                want_placed.push((GRID_ENTITIES + k as u32 * ENTITY_SIZE, spot.to_bits(), facing.to_bits()));
            }
        }
        let got_ground: Vec<[u32; 4]> = grounded.borrow().iter().map(|v| v.to_bits()).collect();
        let got_placed: Vec<(u32, [u32; 4], [u32; 4])> =
            placed.borrow().iter().map(|(e, a, b)| (*e, a.to_bits(), b.to_bits())).collect();
        assert_eq!(got_ground, want_ground, "{what}: the ground queries");
        assert_eq!(got_placed, want_placed, "{what}: the placements");
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "{what}: the generator");
    }
    assert!(gated > 3_000, "only {gated} starts had a gate");
}

/// S196's RAM image of an sx01 race, if this checkout has it.
fn sx01_image() -> Option<Ee> {
    let name = "s196-sx01.ee.bin";
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(name);
    match Ee::from_ram(&here) {
        Ok(ee) => Some(ee),
        Err(why) => {
            skip(&format!("{why} — S196's sx01 RAM image (`./kit state ram` of copy 2's slot 21)"));
            None
        }
    }
}

#[test]
fn the_sx01_image_holds_the_activity_as_ported() {
    let Some(ee) = sx01_image() else { return };
    let [manager, vtable] = match addrs(["activity_manager", "activity_supercross_vtable"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    assert_eq!(ee.u32_at(act + 0x44), vtable, "the live activity is the supercross class");

    // The loader's lane table, bit for bit.
    let table: Vec<u32> = (0..LANES as u32).map(|k| ee.u32_at(act + 0x3bc + 4 * k)).collect();
    let want: Vec<u32> = Supercross::lane_table().iter().map(|x| x.0.to_bits()).collect();
    assert_eq!(table, want, "the lane table");

    // The start as the last reset left it: the intro played (the timeline is not yet at the lights).
    let calls = ee.u32_at(act + 0x47c) as i32;
    let drops = ee.u32_at(act + 0x49c);
    let mut s = Supercross::new();
    s.drop_times = (0..calls as u32).map(|k| f32_at(&ee, drops + 4 * k)).collect();
    s.intro_clips = ee.u32_at(act + 0x438) as i32;
    s.lane_offsets = Supercross::lane_table();
    let (call, intro) = (ee.u32_at(act + 0x4ac) as i32, ee.u32_at(act + 0x478) as i32);
    assert_eq!(ee.u32_at(act + 0x4c4), 0, "the intro has not been seen");
    // Any generator that draws the image's two picks: the timeline depends only on them.
    s.call = -1;
    s.intro = -1;
    let state = (1..100_000u64)
        .find(|&st| {
            let mut r = Rng::from_state(st);
            r.index(calls) == call && r.index(s.intro_clips) == intro
        })
        .expect("a generator state drawing the image's picks");
    let mut gate = Gate {
        flags: 0,
        centre: Vf::ZERO,
        normal: Vf::ZERO,
        across: Vf::ZERO,
        width: F::ZERO,
        height: F::ZERO,
        reach_sq: F::ZERO,
    };
    s.reset(&mut gate, &mut Rng::from_state(state));
    for (o, x, name) in [
        (0x10c, s.start_delay, "start delay"),
        (0x110, s.start_cue, "cue"),
        (0x4b0, s.lights_on, "lights on"),
        (0x4b4, s.lights_off, "lights off"),
        (0x4b8, s.warn_on, "warning on"),
        (0x4bc, s.warn_off, "warning off"),
    ] {
        assert_eq!(ee.u32_at(act + o), x.0.to_bits(), "{name}");
    }
    assert_eq!(ee.u32_at(act + 0x4cc), s.fresh, "fresh");
    let start_gate = ee.u32_at(act + 0x3dc);
    assert_ne!(ee.u32_at(start_gate + 0x30) & flags::CLOSED, 0, "the start gate is closed");

    // The grid: one rider, lane 0, place 1, standing where its lane puts it.
    let riders = ee.u32_at(act + 0x8c) as i32;
    let card = ee.u32_at(act + 0x84);
    let id = ee.u32_at(card) as i32;
    s.lanes[id as usize] = ee.u32_at(act + 0x418 + 4 * id as u32) as i32;
    assert_eq!(ee.u32_at(card + 0x2c) as i32, s.starting_place(id), "the card's place");
    let gs = ee.u32_at(ee.u32_at(act + 0xb8) + 0x58);
    let start = GridStart::Gate { centre: quad(&ee, gs + 0x40), normal: quad(&ee, gs + 0x50), width: f32_at(&ee, gs + 0x70) };
    let (spot, _) = s.grid_spot(start, id, riders);
    let now = quad(&ee, card + 0x60);
    // The rider has sat on its bike for a while since: the ground and the suspension move it a little.
    assert!((spot.x() - now.x()).abs() < 0.05 && (spot.z() - now.z()).abs() < 0.05, "the grid spot {spot:?}, the rider at {now:?}");
}
