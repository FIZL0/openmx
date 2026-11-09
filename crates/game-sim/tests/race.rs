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
//! 3. **The clocks** (S13b): the same tick on 20,000 draws of its music and tip clocks, the music's
//!    holds, the lesson stage and `dt`, its presentation hooked, against [`Race::tick`].
//! 4. **The push**: the bike's own message 2 on 2,000 chassis of stray words, through the chassis'
//!    real vtable, against [`race::push`], compared word for word over the chassis.
//! 5. **The drop**, on the RAM image of a captured sx01 start: the real race manager's tick, nothing
//!    hooked, on the countdown's last tick with the rider moved five feet through the closed gate —
//!    the card, the penalty a tick run down, the chassis word for word, the gate, the clocks, the
//!    scene's word and the generator against [`Race::tick`], [`race::push`] and [`race::release`].
//! 6. **On the Sim**: a rider riding into the closed gate is caught once and thrown back, the countdown
//!    drops the gate and lets the bike go, and a snapshot carries the race.

use game_sim::chassis::{Chassis, Deriv};
use game_sim::race::{self, Gate, Race, Seen, flags};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip, skip};
use std::cell::RefCell;
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

/// Free RAM for the clocks' objects: the app, the music's track, the tip's lookup chain, the stages.
const APP: u32 = 0x0186_0000;
const TRACKS: u32 = 0x0186_1000;
const TRACK: u32 = 0x0186_1100;
const TRACK_VT: u32 = 0x0186_1200;
/// Not code: the track's state slot lands here and is hooked.
const HOOK_TRACK_STATE: u32 = 0x0186_1300;
const LISTENER: u32 = 0x0186_1400;
const UI: u32 = 0x0186_2000;
const STAGES: u32 = 0x0186_3000;
const MUSIC: u32 = 0x0186_4000;
const FADERS: u32 = 0x0186_5000;

/// A clock's word: zero, signed zero, negative, a denormal, under a tick, over it.
fn clock(p: &mut Pick) -> f32 {
    match p.below(8) {
        0 => 0.0,
        1 => -0.0,
        2 => -p.range(0.0, 3.0),
        3 => f32::from_bits(p.next() as u32 & 0x807f_ffff),
        4 => p.range(0.0, 0.04),
        5 => [0.033_333_335, 0.001, 1.0][p.below(3) as usize],
        _ => p.range(0.0, 4.0),
    }
}

#[test]
fn the_race_clocks_match_the_race_managers_tick() {
    let Some(mut ee) = game_or_skip() else { return };
    let names = [
        "race_manager_tick",
        "score_manager_tick",
        "rand_state",
        "app_pointer",
        "race_music_count",
        "race_tip_find",
        "race_tip_show",
        "race_tip_place",
        "ui_manager",
    ];
    let mut at = Vec::new();
    for n in names {
        match addr(n) {
            Ok(a) => at.push(a),
            Err(why) => return skip(&why),
        }
    }
    let [tick, score_tick, rand_state, app_pointer, music_count, tip_find, tip_show, tip_place, ui_manager] =
        at[..].try_into().unwrap();
    // What the presentation was asked for: the music's start and the tip's lookup.
    let asked: Rc<RefCell<(bool, bool)>> = Rc::default();
    let track_state = Rc::new(RefCell::new(0u64));
    let (a, b, s) = (asked.clone(), asked.clone(), track_state.clone());
    ee.hooks.insert(score_tick, Box::new(|_| {}));
    ee.hooks.insert(
        music_count,
        Box::new(move |ee| {
            a.borrow_mut().0 = true;
            ee.set(2, 0);
        }),
    );
    ee.hooks.insert(
        tip_find,
        Box::new(move |ee| {
            b.borrow_mut().1 = true;
            ee.set(2, 0);
        }),
    );
    ee.hooks.insert(tip_show, Box::new(|_| {}));
    ee.hooks.insert(tip_place, Box::new(|_| {}));
    ee.hooks.insert(HOOK_TRACK_STATE, Box::new(move |ee| ee.set(2, *s.borrow())));
    // The app, the music's track behind +0x484, the tip's chain, all in free RAM.
    ee.set_u32(app_pointer, APP);
    ee.set_u32(APP + 0x400, MUSIC);
    ee.set_u32(MUSIC + 0x44, MUSIC + 0x100);
    ee.set_u32(APP + 0x3d0, FADERS);
    ee.set_u32(ui_manager, UI);
    ee.set_u32(UI + 0x30, UI + 0x100);
    ee.set_u32(UI + 0x104, UI + 0x200);
    ee.set_u32(UI + 0x200, UI + 0x300);
    ee.set_u32(UI + 0x304, UI + 0x400);
    ee.set_u32(TRACK + 8, TRACK_VT);
    ee.set_u32(TRACK_VT + 0x20, HOOK_TRACK_STATE);
    ee.set_u32(RACE + 0x484, TRACKS);
    ee.set_u32(RACE + 0x160, LISTENER);
    ee.set_u32(RACE + 0x3dc, GATE);
    ee.set_u32(RACE + 0x84, 0);
    ee.set_u32(GATE + 0x30, 0);

    let mut p = Pick(0x7c1f_00d3_55aa_e4b1);
    let mut seen = [0usize; 4];
    for case in 0..20_000 {
        let music_in = clock(&mut p);
        let tip_in = clock(&mut p);
        let stage_index = if p.below(3) == 0 { 0 } else { 1 + p.below(4) as u32 };
        let stage_length = match p.below(6) {
            0 => 0.0,
            1 => -0.0,
            2 => f32::from_bits(p.next() as u32 & 0x807f_ffff),
            _ => p.range(-1.0, 8.0),
        };
        let armed = p.below(3) == 0;
        let state = if p.below(5) == 0 { 2 } else { p.below(4) };
        let paused = p.below(5) == 0;
        let has_music = p.below(2) == 0;
        let pick = p.below(3) as u32;
        let dt = if p.below(4) == 0 { p.range(0.0, 0.1) } else { 0.033_333_335 };
        let rand = p.next();

        ee.set_u32(RACE + 0x4c0, music_in.to_bits());
        ee.set_u32(RACE + 0x410, tip_in.to_bits());
        ee.set_u32(RACE + 0x470, stage_index);
        ee.set_u32(RACE + 0x474, u32::from(armed));
        ee.set_u32(RACE + 0x4ac, pick);
        ee.set_u32(TRACKS + 4 * pick, TRACK);
        ee.set_u32(RACE + 0x44c + 4 * stage_index, STAGES + 0x40 * stage_index);
        ee.set_u32(STAGES + 0x40 * stage_index + 0x24, stage_length.to_bits());
        ee.set_u32(LISTENER + 0x5c, if paused { 2 } else { 0 });
        ee.set_u32(APP + 0x570, u32::from(has_music));
        *track_state.borrow_mut() = state;
        ee.write(rand_state, 8, u128::from(rand));
        *asked.borrow_mut() = (false, false);
        ee.call(tick, &[u64::from(RACE)], &[dt], BUDGET);

        let mut race = Race::new(Gate { flags: 0, ..closed_gate() }, &[], F::ZERO, Rng::from_state(rand));
        race.gate.flags = 0;
        race.countdown.running = false;
        race.music_in = F(music_in);
        race.music_held = state == 2 || paused;
        race.tip_in = F(tip_in);
        race.stage = (stage_index != 0).then_some(F(stage_length));
        race.tip_armed = armed;
        let frame = race.tick(F(dt), &[]);
        let what = format!("case {case}: music {music_in:?} tip {tip_in:?} stage {stage_index} {stage_length:?} armed {armed} dt {dt}");
        assert_eq!(ee.u32_at(RACE + 0x4c0), race.music_in.0.to_bits(), "{what}: the music clock");
        assert_eq!(ee.u32_at(RACE + 0x410), race.tip_in.0.to_bits(), "{what}: the tip clock");
        assert_eq!(ee.u32_at(RACE + 0x474), u32::from(race.tip_armed), "{what}: armed");
        let (music, tip) = *asked.borrow();
        assert_eq!(music, frame.music && has_music, "{what}: the music's start");
        assert_eq!(tip, frame.tip, "{what}: the tip");
        assert_eq!(ee.read(rand_state, 8) as u64, race.rng.state(), "{what}: the generator");
        seen[usize::from(frame.music)] += 1;
        seen[2 + usize::from(frame.tip)] += usize::from(frame.tip || race.tip_armed != armed);
    }
    assert!(seen[1] > 1000 && seen[3] > 1000, "too few run-outs: {seen:?}");
}

/// A gate the race starts behind; only its flags matter to the clocks.
fn closed_gate() -> Gate {
    Gate {
        flags: flags::CLOSED,
        centre: Vf::ZERO,
        normal: Vf([0.0, 0.0, 1.0, 0.0]),
        across: Vf([1.0, 0.0, 0.0, 0.0]),
        width: F(10.0),
        height: F(10.0),
        reach_sq: F::ZERO,
    }
}

const BIKE: u32 = 0x0187_0000;
const CHASSIS: u32 = 0x0188_0000;
const OUT: u32 = 0x0189_0000;

/// The chassis words the push touches or might, `+0x0..+0x330`, as the port's fields hold them.
fn chassis_from(words: &[u32]) -> Chassis {
    let f = |o: usize| F(f32::from_bits(words[o / 4]));
    let q = |o: usize| Vf([0, 4, 8, 12].map(|k| f32::from_bits(words[(o + k) / 4])));
    let d3 = |o: usize| Deriv { dx: [f(o), f(o + 4), f(o + 8)], prev: [f(o + 12), f(o + 16), f(o + 20)] };
    let i = |o: usize| words[o / 4] as i32;
    Chassis {
        speed: f(0x34),
        accel: q(0x70),
        ang_accel: q(0x80),
        force: q(0x130),
        torque: q(0x140),
        impulse: q(0x150),
        correction: q(0x160),
        vel: q(0x260),
        local_vel: q(0x270),
        world_ang_vel: q(0x280),
        ang_vel: q(0x290),
        free: [i(0x2a0), i(0x2a4), i(0x2a8)],
        ang_free: [i(0x2ac), i(0x2b0), i(0x2b4)],
        pos_hist: d3(0x2b8),
        vel_hist: d3(0x2d0),
        quat_hist: Deriv {
            dx: [f(0x2f0), f(0x2f4), f(0x2f8), f(0x2fc)],
            prev: [f(0x300), f(0x304), f(0x308), f(0x30c)],
        },
        ang_hist: d3(0x310),
        ..Chassis::default()
    }
}

/// [`chassis_from`] the other way: the port's fields written over `words`.
fn chassis_into(words: &mut [u32], ch: &Chassis) {
    let mut f = |o: usize, x: F| words[o / 4] = x.0.to_bits();
    f(0x34, ch.speed);
    for (o, v) in [
        (0x70, ch.accel),
        (0x80, ch.ang_accel),
        (0x130, ch.force),
        (0x140, ch.torque),
        (0x150, ch.impulse),
        (0x160, ch.correction),
        (0x260, ch.vel),
        (0x270, ch.local_vel),
        (0x280, ch.world_ang_vel),
        (0x290, ch.ang_vel),
    ] {
        for k in 0..4 {
            f(o + 4 * k, F(v.0[k]));
        }
    }
    for (o, d) in [(0x2b8, ch.pos_hist), (0x2d0, ch.vel_hist), (0x310, ch.ang_hist)] {
        for k in 0..3 {
            f(o + 4 * k, d.dx[k]);
            f(o + 12 + 4 * k, d.prev[k]);
        }
    }
    for k in 0..4 {
        f(0x2f0 + 4 * k, ch.quat_hist.dx[k]);
        f(0x300 + 4 * k, ch.quat_hist.prev[k]);
    }
    for k in 0..3 {
        words[(0x2a0 + 4 * k) / 4] = ch.free[k] as u32;
        words[(0x2ac + 4 * k) / 4] = ch.ang_free[k] as u32;
    }
}

#[test]
fn the_push_matches_the_bikes_message() {
    let Some(mut ee) = game_or_skip() else { return };
    let (message, vtable) = match (addr("bike_message"), addr("chassis_vtable")) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(why), _) | (_, Err(why)) => return skip(&why),
    };
    let mut p = Pick(0x0bad_cafe_1357_9bdf);
    for case in 0..2_000 {
        let mut words: Vec<u32> = (0..0x330 / 4)
            .map(|_| {
                let x = p.range(-50.0, 50.0);
                p.stray(x).to_bits()
            })
            .collect();
        words[2] = vtable;
        let v = [p.range(-10.0, 10.0), p.range(-10.0, 10.0), p.range(-10.0, 10.0)];
        let push = p.quad(v);
        for (k, w) in words.iter().enumerate() {
            ee.set_u32(CHASSIS + 4 * k as u32, *w);
        }
        ee.set_u32(BIKE + 0x7a4, CHASSIS);
        put_quad(&mut ee, OUT, push);
        ee.call(message, &[u64::from(BIKE), 2, u64::from(OUT), 0, 0], &[], BUDGET);

        let mut ch = chassis_from(&words);
        race::push(&mut ch, push);
        let mut want = words.clone();
        chassis_into(&mut want, &ch);
        let got: Vec<u32> = (0..words.len()).map(|k| ee.u32_at(CHASSIS + 4 * k as u32)).collect();
        for (k, (g, w)) in got.iter().zip(&want).enumerate() {
            assert_eq!(g, w, "case {case}: chassis +{:#x}", 4 * k);
        }
    }
}

/// A gate as it sits in RAM.
fn gate_at(ee: &Ee, at: u32) -> Gate {
    let f = |o: u32| F(f32::from_bits(ee.u32_at(at + o)));
    Gate {
        flags: ee.u32_at(at + 0x30),
        centre: quad(ee, at + 0x40),
        normal: quad(ee, at + 0x50),
        across: quad(ee, at + 0x60),
        width: f(0x70),
        height: f(0x74),
        reach_sq: f(0x7c),
    }
}

/// The RAM image of a supercross start (sx01, practice): the countdown running, the gate closed, the
/// one rider's bike held behind it.
const SX01: &str = "s196-sx01.ee.bin";

#[test]
fn the_drop_and_an_early_rider_match_a_captured_sx01_start() {
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let path = kit.ctx("states").join(SX01);
    if !path.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of an sx01 start (S196's slot 21)", path.display()));
    }
    let mut at = Vec::new();
    for n in ["race_manager_tick", "rand_state", "app_pointer", "activity_manager", "boot_gp"] {
        match addr(n) {
            Ok(a) => at.push(a),
            Err(why) => return skip(&why),
        }
    }
    let [tick, rand_state, app_pointer, activities, gp] = at[..].try_into().unwrap();
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(gp));
    let race_at = ee.u32_at(ee.u32_at(ee.u32_at(activities) + 0x58));
    let mgr = race_at + 0x50;
    let gate_ptr = ee.u32_at(race_at + 0x3dc);
    let card = ee.u32_at(race_at + 0x84);
    assert_ne!(card, 0, "no score card on the race");
    assert_eq!(ee.u32_at(card + 0x1c8), 0, "one rider's start");
    let bike = ee.u32_at(card + 4) - 0xf9c;
    let ch = ee.u32_at(bike + 0x7a4);
    let object = ee.u32_at(ch + 0x254) + 0x80;
    let scene = ee.u32_at(ee.u32_at(app_pointer) + 0x3cc);
    let f = |ee: &Ee, a: u32| F(f32::from_bits(ee.u32_at(a)));

    let gate = gate_at(&ee, gate_ptr);
    assert_eq!(gate.flags & flags::CLOSED, flags::CLOSED, "the gate is closed in the image");
    let seen = Seen { now: quad(&ee, card + 0x60), last: quad(&ee, card + 0x70) };
    let facing = quad(&ee, object + 0x20);
    // The countdown's last tick, with the rider five feet through the gate on it.
    let length = f(&ee, mgr + 0xbc);
    ee.set_u32(mgr + 0xc4, length.0.to_bits());
    let through = seen.now.add(gate.normal.mul_q(5.0)).merge(ps2_float::Dest::W, seen.now);
    put_quad(&mut ee, object + 0x30, through);
    let words: Vec<u32> = (0..0x330 / 4).map(|k| ee.u32_at(ch + 4 * k)).collect();
    let state = ee.read(rand_state, 8) as u64;
    let dt = 0.033_333_335;
    let flags_before = ee.u32_at(mgr + 0x5c);
    let (music_in, tip_in) = (f(&ee, race_at + 0x4c0), f(&ee, race_at + 0x410));
    assert_eq!(ee.u32_at(race_at + 0x470), 0, "no lesson stage on a race");
    assert_eq!(flags_before & 0x81, 0x80, "the countdown is running in the image");
    ee.call(tick, &[u64::from(race_at)], &[dt], 50_000_000);

    let mut race = Race::new(gate, &[(seen.now, facing)], length, Rng::from_state(state));
    race.cards[0] = seen;
    race.countdown.elapsed = length;
    race.music_in = music_in;
    race.tip_in = tip_in;
    let frame = race.tick(F(dt), &[(through, facing)]);
    assert_eq!(frame.caught.len(), 1, "the rider is caught");
    assert!(frame.dropped, "the gate drops");
    let mut port = chassis_from(&words);
    race::push(&mut port, frame.caught[0].1.push);
    race::release(&mut port);
    let mut score = game_sim::score::Score { gate_penalty: frame.caught[0].1.penalty, ..Default::default() };
    score.grow(F(dt));

    assert_eq!(ee.u32_at(gate_ptr + 0x30), race.gate.flags, "the gate's flags");
    assert_eq!(quad(&ee, card + 0x60).to_bits(), race.cards[0].now.to_bits(), "the card's now");
    assert_eq!(quad(&ee, card + 0x70).to_bits(), race.cards[0].last.to_bits(), "the card's last");
    assert_eq!(ee.u32_at(card + 0x1c0), score.gate_penalty.0.to_bits(), "the penalty, a tick run down");
    assert_eq!(ee.u32_at(race_at + 0x4c0), race.music_in.0.to_bits(), "the music clock");
    assert_eq!(ee.u32_at(race_at + 0x410), race.tip_in.0.to_bits(), "the tip clock");
    assert_eq!(ee.u32_at(scene + 0xf0), race.scene_clock.0.to_bits(), "the scene's word");
    assert_eq!(ee.u32_at(mgr + 0xc4), race.countdown.elapsed.0.to_bits(), "the countdown's elapsed");
    assert_eq!(ee.u32_at(mgr + 0x5c) & 0x81, 0x01, "the countdown ended");
    let mut want = words.clone();
    chassis_into(&mut want, &port);
    for (k, w) in want.iter().enumerate() {
        assert_eq!(ee.u32_at(ch + 4 * k as u32), *w, "chassis +{:#x}", 4 * k);
    }
    assert_eq!(ee.read(rand_state, 8) as u64, race.rng.state(), "the generator");
}

/// A ride on the stand-in bike with a race's start gate `ahead` feet in front of it, square across
/// its way, and a countdown of `length` seconds.
fn race_ride(ahead: f32, length: f32) -> game_sim::Sim {
    let mut sim = game_sim::Sim::new(7);
    let m = sim.rider.ride.transform();
    let (at, facing) = (m.0[3], m.0[2]);
    let n = {
        let l = (facing.x() * facing.x() + facing.z() * facing.z()).sqrt();
        [facing.x() / l, facing.z() / l]
    };
    let gate = Gate {
        flags: flags::CLOSED,
        centre: Vf([at.x() + n[0] * ahead, at.y() - 5.0, at.z() + n[1] * ahead, 1.0]),
        normal: Vf([n[0], 0.0, n[1], 1.0]),
        across: Vf([n[1], 0.0, -n[0], 1.0]),
        width: F(40.0),
        height: F(1000.0),
        reach_sq: F(-1.0),
    };
    sim.race = Some(Box::new(Race::new(gate, &[(at, facing)], F(length), Rng::from_state(0x1234_5678))));
    sim
}

fn gas() -> game_sim::Input {
    game_sim::Input { buttons: game_sim::input::Control::Cross.mask(), ..game_sim::Input::default() }
}

#[test]
fn a_rider_rolling_through_the_closed_gate_is_thrown_back_and_loses_the_gas() {
    let mut sim = race_ride(3.0, 30.0);
    let start = sim.bike_pos();
    let mut n = 0;
    while sim.rider.score.gate_penalty <= F::ZERO {
        sim.tick(gas());
        n += 1;
        assert!(n < 300, "the rider never reached the gate: {:?} from {start:?}", sim.bike_pos());
    }
    let race = sim.race.as_ref().unwrap();
    assert_eq!(race.gate.flags & flags::CLOSED, flags::CLOSED, "still closed");
    // Caught this tick: the card's penalty is one to two seconds, and the bike, stopped dead and sent
    // back along its nose, is going backwards.
    let p = sim.rider.score.gate_penalty;
    assert!(p > F(0.9) && p <= F(2.0), "the penalty {p:?}");
    let facing = sim.rider.ride.transform().0[2];
    let back = sim.bike_vel();
    assert!(back.x() * facing.x() + back.z() * facing.z() < 0.0, "thrown back: {back:?}");
    // With the gas cut the bike rolls back behind the gate rather than through it again at once.
    let mut caught = 1;
    let before = sim.race.as_ref().unwrap().rng.state();
    for _ in 0..20 {
        let had = sim.rider.score.gate_penalty;
        sim.tick(gas());
        caught += usize::from(sim.rider.score.gate_penalty > had);
    }
    assert_eq!(caught, 1, "caught once in the penalty");
    assert_eq!(sim.race.as_ref().unwrap().rng.state(), before, "no draws while nobody crosses");
}

#[test]
fn the_countdown_drops_the_gate_and_lets_the_riders_go() {
    let mut sim = race_ride(50.0, 1.0);
    // The bike as a start holds it, pinned but for its height.
    sim.rider.ride.chassis.free = [0, 1, 0];
    let mut n = 0;
    while sim.race.as_ref().unwrap().countdown.running {
        sim.tick(game_sim::Input::default());
        n += 1;
        assert!(n < 100, "the countdown never ended");
    }
    // 1 s of 1/30 s ticks: thirty under, the thirty-first ends it (or one either side, on rounding).
    assert!((30..=32).contains(&n), "{n} ticks");
    let race = sim.race.as_ref().unwrap();
    assert_eq!(race.gate.flags & flags::CLOSED, 0, "the gate is open");
    assert_eq!(race.music_in, F::ONE, "the music a second away");
    assert!(race.scene_clock >= F(8.0) && race.scene_clock <= F(13.0), "{:?}", race.scene_clock);
    assert_eq!(sim.rider.ride.chassis.free, [1, 1, 1], "the bike let go");
    // The music starts a second later.
    let mut n = 0;
    while sim.race.as_ref().unwrap().music_in > F::ZERO {
        sim.tick(game_sim::Input::default());
        n += 1;
        assert!(n < 100);
    }
    assert!((29..=31).contains(&n), "{n} ticks to the music");
}

#[test]
fn a_race_is_carried_by_a_snapshot_and_a_ride_without_one_is_not_changed() {
    let mut sim = race_ride(3.0, 30.0);
    (0..20).for_each(|_| sim.tick(gas()));
    let bytes = game_sim::snapshot::encode(&sim);
    let mut other = race_ride(3.0, 30.0);
    game_sim::snapshot::decode(&mut other, &bytes).expect("the state decodes");
    assert_eq!(other.hash(), sim.hash());
    assert_eq!(other.race, sim.race);
    (0..40).for_each(|_| {
        sim.tick(gas());
        other.tick(gas());
    });
    assert_eq!(other.hash(), sim.hash(), "the same 40 ticks on");

    let mut plain = game_sim::Sim::new(7);
    let mut none = game_sim::Sim::new(7);
    none.race = None;
    (0..20).for_each(|_| {
        plain.tick(gas());
        none.tick(gas());
    });
    assert_eq!(game_sim::snapshot::encode(&plain), game_sim::snapshot::encode(&none));
    let raced = race_ride(500.0, 300.0);
    assert_ne!(raced.hash(), game_sim::Sim::new(7).hash(), "a race is in the hash");
}
