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
//! 6. **The clocks** (S13b): the same tick on 20,000 draws of its music and tip clocks, the music's
//!    holds, the lesson stage and `dt`, its presentation hooked, against [`Race::tick`].
//! 7. **The push**: the bike's own message 2 on 2,000 chassis of stray words, through the chassis'
//!    real vtable, against [`race::push`], compared word for word over the chassis.
//! 8. **The drop**, on the RAM image of a captured sx01 start: the real race manager's tick, nothing
//!    hooked, on the countdown's last tick with the rider moved five feet through the closed gate —
//!    the card, the penalty a tick run down, the chassis word for word, the gate, the clocks, the
//!    scene's word and the generator against [`Race::tick`], [`race::push`] and [`race::release`].
//! 9. **On the Sim**: a rider riding into the closed gate is caught once and thrown back, the countdown
//!    drops the gate and lets the bike go, and a snapshot carries the race.
//! 10. **The lap line** (S13c): the score card's line test on 20,000 generated cards (every race kind,
//!     ghost cards, lap records, stray words in every float), against [`LapRules::line`], compared on its
//!     answer, every listener call in order, every word of the card and the ghost's card, the manager's
//!     flags and its free lap records.
//! 11. **The race file** (S13e): the track object's own loader, its file reads and allocations
//!     hooked, on 12,000 generated race files (every record kind, stray words, truncated and refused
//!     files), against [`TrackFile::parse`], compared on every path, node, link, checkpoint, part and
//!     gate it builds, or on the refusal.
//! 12. **sx01 off the disc**: sx01's race file read out of `MX.PSR` against the track object in S196's
//!     RAM image — the header, both paths, all 96 nodes and the third gate, bit for bit (but the ground's
//!     node heights and a flag the walk after the load clears).
//! 13. **The gates on their props**: the same image's grid gate, start gate and finish line against
//!     [`Supercross::place_drop_gate`] and [`Supercross::place_finish_line`] on the image's own props.
//! 14. **The track's paths** (S293), each on 20,000 generated courses of one to seven linked paths
//!     (stray words, kept-off and unreached paths, short pools, signed-zero and denormal spots) laid
//!     out as the track object holds them: the spot's point and the way ahead
//!     ([`TrackPaths::spot_point`], [`TrackPaths::path_ahead`]); reach, distance, the route search, the
//!     walk along a route and its release ([`TrackPaths::distance`], [`TrackPaths::route`],
//!     [`TrackPaths::route_walk`], [`TrackPaths::free_route`]), compared on the answers, the lists, every
//!     path's flags, distance and best link and the pools; the ring test ([`race::ring_holds`]).
//! 15. **The spot search**: the track's own search for a rider's spot on the same courses, from any
//!     kept spot, any reach (negative too) and lists of paths to come by, against
//!     [`TrackPaths::find_spot`] on the answer (all three kinds), the spot, the distance, the paths and
//!     the pools.
//! 16. **The load's reach walk**: the walk from a path that clears the unreached mark, short pools and
//!     stray marks included, against [`TrackPaths::mark_reached`].
//! 17. **The race handler, per card**: the score manager's whole race tick on one card, 20,000
//!     generated cards, courses and lines (every race kind, ghost cards, lap records, done and
//!     waited-for cards, the off-track and time limits, steps through the line), its listeners hooked
//!     and the manager's work after its cards held off, against [`RaceRules::look_for_waited_for`] and
//!     [`RaceRules::card`], compared on every listener call in order, every word of the card and the
//!     ghost's card, the manager's flags, the free records, and every path's words and the pools.
//! 18. **The finish** (S294): stretches of the score manager's race handler, called at their first
//!     instruction with the handler's registers set and their end hooked — the opening over check (10,000
//!     lists against [`LapRules::check_over`]), a card's clocks (20,000 cards, made-up finishes and
//!     running clocks, against [`LapRules::clocks`]), its time limit (20,000, against
//!     [`LapRules::time_limit`]) and the close after the card loop with the ranking and gap functions
//!     it calls run for real (20,000 lists of 1–6 cards with ties, ghost cards, the generator anywhere,
//!     against [`LapRules::close`]), compared on every card word, the list's order, every listener call
//!     in order, the manager's flags, the countdown and the generator.
//! 19. **The gate line** (S292): the gate race's line test (the machine race's, and four activities')
//!     on 20,000 generated chains of one to five gates and cards waiting at any of them (every race
//!     kind, ghost cards, lap records, stray words), against [`LapRules::gate_line`], compared as the
//!     lap line is, plus the card's gate and gate counts.
//! 20. **The zone list**, as the machine race asks it: that stretch of its handler on 20,000 zone
//!     lists about a step (shut zones, the list off or empty), against [`race::zone_crossings`] on
//!     every listener call and the zone each names.
//! 21. **The stunt mode's zones**: its whole handler on 5,000 steps that cross zones; no zone is ever
//!     told, because the handler has copied the place into the tick before's first.

use game_sim::chassis::{Chassis, Deriv};
use game_sim::race::{
    self, CheckPart, Checkpoint, Course, Crossing, Gate, GridStart, LANES, LapCard, LapRecord, LapRules, LineNotice, Link,
    PathNode, Prop, Race, RaceCard, FinishNotice, RaceNotice, RacePath, RaceRules, Refused, Seen, Spot, Supercross, TrackFile, TrackPath, TrackPaths, card_flags,
    flags, race_card_flags, race_flags, race_kind,
};
use game_sim::rng::Rng;
use ps2_float::{F, Vf, fpu};
use game_sim::archive::{Psr, Res};
use ps2_oracle::{Ee, Kit, addr, game_or_skip, skip};
use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
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

/// Free RAM for the clocks' objects: the app, the music's track, the tip's lookup chain, the stages.
const CLOCK_APP: u32 = 0x0186_0000;
const TRACKS: u32 = 0x0186_1000;
const CLOCK_TRACK: u32 = 0x0186_1100;
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
    ee.set_u32(app_pointer, CLOCK_APP);
    ee.set_u32(CLOCK_APP + 0x400, MUSIC);
    ee.set_u32(MUSIC + 0x44, MUSIC + 0x100);
    ee.set_u32(CLOCK_APP + 0x3d0, FADERS);
    ee.set_u32(ui_manager, UI);
    ee.set_u32(UI + 0x30, UI + 0x100);
    ee.set_u32(UI + 0x104, UI + 0x200);
    ee.set_u32(UI + 0x200, UI + 0x300);
    ee.set_u32(UI + 0x304, UI + 0x400);
    ee.set_u32(CLOCK_TRACK + 8, TRACK_VT);
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
        ee.set_u32(TRACKS + 4 * pick, CLOCK_TRACK);
        ee.set_u32(RACE + 0x44c + 4 * stage_index, STAGES + 0x40 * stage_index);
        ee.set_u32(STAGES + 0x40 * stage_index + 0x24, stage_length.to_bits());
        ee.set_u32(LISTENER + 0x5c, if paused { 2 } else { 0 });
        ee.set_u32(CLOCK_APP + 0x570, u32::from(has_music));
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

// --- the lap line (S13c) ---------------------------------------------------------------------------

const L_MGR: u32 = 0x01a0_0000;
const L_TRACK: u32 = 0x01a0_1000;
const L_LINE: u32 = 0x01a0_2000;
const L_CARD: u32 = 0x01a0_3000;
const L_GHOST: u32 = 0x01a0_4000;
const L_OWNER: u32 = 0x01a0_5000;
const L_CHAIN: u32 = 0x01a0_5100;
const L_LISTENER: u32 = 0x01a0_5200;
const L_VT: u32 = 0x01a0_6000;
const L_LINES: u32 = 0x01a0_7000;
const L_GHOST_LINES: u32 = 0x01a0_7100;
const L_POOL: u32 = 0x01a0_8000;
const L_RECORDS: u32 = 0x01a0_9000;
const L_HOOKS: u32 = 0x01a0_a000;

/// Who a listener call went to: the card's owner, the card's chain, or the manager's listener.
const OWNER: u32 = 0;
const CHAIN: u32 = 1;
const TO_LISTENER: u32 = 2;

/// The calls one notice makes, as (who, vtable slot), with what is there to hear them.
fn told(n: LineNotice, owner: bool, chain: bool) -> Vec<(u32, u32)> {
    let fan = |o: u32, l: u32| {
        let mut v = Vec::new();
        if owner {
            v.push((OWNER, o));
        }
        if chain {
            v.push((CHAIN, o));
        }
        v.push((TO_LISTENER, l));
        v
    };
    match n {
        LineNotice::Beside => fan(0x28, 0x20),
        LineNotice::Through => fan(0x24, 0x1c),
        LineNotice::Started => vec![(TO_LISTENER, 0x4c)],
        // The original tells the owner, then walks a list starting at the owner again (not the
        // card's chain): the owner hears it twice.
        LineNotice::BestLap => {
            let mut v = if owner { vec![(OWNER, 0x10), (OWNER, 0x10)] } else { vec![] };
            v.push((TO_LISTENER, 0xc));
            v
        }
        LineNotice::Finished => fan(0x18, 0x14),
        LineNotice::RunOver => vec![(TO_LISTENER, 0x54)],
        LineNotice::GhostBest => vec![(TO_LISTENER, 0x58)],
        LineNotice::GhostReady => vec![(TO_LISTENER, 0x50)],
        LineNotice::Lap => fan(0xc, 8),
        LineNotice::Gate => fan(0x14, 0x10),
    }
}

/// A card's time word: mostly an ordinary clock, now and then zero, negative or a stray pattern.
fn time_word(p: &mut Pick, hi: f32) -> F {
    let x = match p.below(8) {
        0 => 0.0,
        1 => -p.range(0.0, 10.0),
        _ => p.range(0.0, hi),
    };
    F(p.stray(x))
}

fn spot(p: &mut Pick) -> Spot {
    let along = p.range(0.0, 2000.0);
    Spot { path: p.next() as u32, piece: p.below(40) as u32, along: F(p.stray(along)) }
}

fn lap_card(p: &mut Pick, lines: usize) -> LapCard {
    let mut flags = p.next() & 0xffff_ffff_fe00_feec;
    for (bit, odds) in [
        (card_flags::OUT, 4),
        (card_flags::TAKING_PART, 2),
        (card_flags::NO_LINE, 10),
        (card_flags::FAST_LAP, 2),
        (card_flags::BEHIND_LINE, 5),
        (card_flags::NO_BEST, 4),
    ] {
        if p.below(odds) == 0 {
            flags |= bit;
        } else {
            flags &= !bit;
        }
    }
    let race_time = time_word(p, 600.0);
    LapCard {
        flags,
        to_line_kept: time_word(p, 900.0),
        to_line: time_word(p, 900.0),
        progress: time_word(p, 5000.0),
        laps_length: time_word(p, 5000.0),
        lap_length: time_word(p, 900.0),
        place: p.below(9) as i32,
        laps: p.below(7) as i32 - if p.below(20) == 0 { 3 } else { 0 },
        runs: p.below(7) as i32,
        on_track: p.below(3) as i32,
        lap_time: time_word(p, 90.0),
        lap_start: if p.below(5) == 0 { race_time } else { time_word(p, 600.0) },
        best_lap: if p.below(3) == 0 { F::ZERO } else { time_word(p, 120.0) },
        best_lap_no: p.below(6) as i32,
        seen: Seen::default(),
        spot: spot(p),
        kept_spot: spot(p),
        race_time,
        lap_clock: time_word(p, 90.0),
        gap: time_word(p, 60.0),
        off_track: time_word(p, 10.0),
        time_left: time_word(p, 60.0),
        attack_clock: time_word(p, 60.0),
        record_word: p.next() as u32,
        lines_passed: (0..lines).map(|_| p.next() as u32 | 1).collect(),
        records: (0..p.below(4)).map(|_| record(p)).collect(),
        gate: None,
        gates_this_lap: 0,
        gates_passed: 0,
    }
}

fn record(p: &mut Pick) -> LapRecord {
    LapRecord {
        time: time_word(p, 90.0),
        start: time_word(p, 600.0),
        word: p.next() as u32,
        lap: p.below(12) as i32,
        place: p.below(9) as i32,
    }
}

fn put_spot(ee: &mut Ee, at: u32, s: Spot) {
    ee.set_u32(at, s.path);
    ee.set_u32(at + 4, s.piece);
    put_f(ee, at + 8, s.along);
}

fn spot_at(ee: &Ee, at: u32) -> Spot {
    Spot { path: ee.u32_at(at), piece: ee.u32_at(at + 4), along: f32_at(ee, at + 8) }
}

fn put_record(ee: &mut Ee, at: u32, r: &LapRecord, next: u32) {
    put_f(ee, at, r.time);
    put_f(ee, at + 4, r.start);
    ee.set_u32(at + 8, r.word);
    ee.set_u32(at + 0xc, r.lap as u32);
    ee.set_u32(at + 0x10, r.place as u32);
    ee.set_u32(at + 0x14, next);
}

/// A card into RAM at `at`, its line words at `lines` and its records at `records`.
fn put_lap_card(ee: &mut Ee, at: u32, c: &LapCard, lines: u32, records: u32) {
    ee.set_u32(at + 8, c.flags as u32);
    ee.set_u32(at + 0xc, (c.flags >> 32) as u32);
    for (o, x) in [
        (0x10, c.to_line_kept),
        (0x14, c.to_line),
        (0x1c, c.progress),
        (0x24, c.laps_length),
        (0x28, c.lap_length),
        (0x48, c.lap_time),
        (0x4c, c.lap_start),
        (0x50, c.best_lap),
        (0xb4, c.race_time),
        (0xb8, c.lap_clock),
        (0xbc, c.gap),
        (0xc0, c.off_track),
        (0xc4, c.time_left),
        (0x168, c.attack_clock),
    ] {
        put_f(ee, at + o, x);
    }
    for (o, x) in [(0x2c, c.place), (0x30, c.laps), (0x34, c.runs), (0x40, c.on_track), (0x54, c.best_lap_no)] {
        ee.set_u32(at + o, x as u32);
    }
    put_quad(ee, at + 0x60, c.seen.now);
    put_quad(ee, at + 0x70, c.seen.last);
    put_spot(ee, at + 0x88, c.spot);
    put_spot(ee, at + 0x94, c.kept_spot);
    ee.set_u32(at + 0x108, c.record_word);
    ee.set_u32(at + 0xe4, if c.lines_passed.is_empty() { 0 } else { lines });
    for (k, w) in c.lines_passed.iter().enumerate() {
        ee.set_u32(lines + 4 * k as u32, *w);
    }
    let n = c.records.len() as u32;
    for (k, r) in c.records.iter().enumerate() {
        let k = k as u32;
        put_record(ee, records + 0x20 * k, r, if k + 1 < n { records + 0x20 * (k + 1) } else { 0 });
    }
    ee.set_u32(at + 0x1bc, if n == 0 { 0 } else { records });
}

/// A card back out of RAM, its line words `lines` long.
fn lap_card_at(ee: &Ee, at: u32, lines: usize) -> LapCard {
    let i = |o: u32| ee.u32_at(at + o) as i32;
    let f = |o: u32| f32_at(ee, at + o);
    let list = ee.u32_at(at + 0xe4);
    let mut records = Vec::new();
    let mut r = ee.u32_at(at + 0x1bc);
    while r != 0 && records.len() < 64 {
        records.push(LapRecord {
            time: f32_at(ee, r),
            start: f32_at(ee, r + 4),
            word: ee.u32_at(r + 8),
            lap: ee.u32_at(r + 0xc) as i32,
            place: ee.u32_at(r + 0x10) as i32,
        });
        r = ee.u32_at(r + 0x14);
    }
    LapCard {
        flags: u64::from(ee.u32_at(at + 8)) | u64::from(ee.u32_at(at + 0xc)) << 32,
        to_line_kept: f(0x10),
        to_line: f(0x14),
        progress: f(0x1c),
        laps_length: f(0x24),
        lap_length: f(0x28),
        place: i(0x2c),
        laps: i(0x30),
        runs: i(0x34),
        on_track: i(0x40),
        lap_time: f(0x48),
        lap_start: f(0x4c),
        best_lap: f(0x50),
        best_lap_no: i(0x54),
        seen: Seen { now: quad(ee, at + 0x60), last: quad(ee, at + 0x70) },
        spot: spot_at(ee, at + 0x88),
        kept_spot: spot_at(ee, at + 0x94),
        race_time: f(0xb4),
        lap_clock: f(0xb8),
        gap: f(0xbc),
        off_track: f(0xc0),
        time_left: f(0xc4),
        attack_clock: f(0x168),
        record_word: ee.u32_at(at + 0x108),
        lines_passed: if list == 0 { Vec::new() } else { (0..lines as u32).map(|k| ee.u32_at(list + 4 * k)).collect() },
        records,
        gate: None,
        gates_this_lap: 0,
        gates_passed: 0,
    }
}

/// Every word of a card, as bits, so that a signed zero or a stray pattern compares as itself.
fn lap_card_words(c: &LapCard) -> Vec<u64> {
    let f = |x: F| u64::from(x.0.to_bits());
    let s = |x: Spot| [u64::from(x.path), u64::from(x.piece), f(x.along)];
    let mut v = vec![
        c.flags,
        f(c.to_line_kept),
        f(c.to_line),
        f(c.progress),
        f(c.laps_length),
        f(c.lap_length),
        c.place as u64,
        c.laps as u64,
        c.runs as u64,
        c.on_track as u64,
        f(c.lap_time),
        f(c.lap_start),
        f(c.best_lap),
        c.best_lap_no as u64,
        f(c.race_time),
        f(c.lap_clock),
        f(c.gap),
        f(c.off_track),
        f(c.time_left),
        f(c.attack_clock),
        u64::from(c.record_word),
    ];
    v.extend(c.seen.now.to_bits().map(u64::from));
    v.extend(c.seen.last.to_bits().map(u64::from));
    v.extend(s(c.spot));
    v.extend(s(c.kept_spot));
    v.push(c.lines_passed.len() as u64);
    v.extend(c.lines_passed.iter().map(|&w| u64::from(w)));
    for r in &c.records {
        v.extend([f(r.time), f(r.start), u64::from(r.word), r.lap as u64, r.place as u64]);
    }
    v.extend([c.gate.map_or(u64::MAX, |g| g as u64), c.gates_this_lap as u64, c.gates_passed as u64]);
    v
}

/// A line most steps can be aimed through: live, standing up, a sensible width; now and then any
/// gate the crossing test's generator makes.
fn finish_line(p: &mut Pick) -> Gate {
    let mut g = gate(p);
    if p.below(4) != 0 {
        let normal = [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]][p.below(3) as usize];
        g.flags = flags::CLOSED | (p.next() as u32 & flags::EITHER_WAY);
        g.normal = Vf([normal[0], normal[1], normal[2], 0.0]);
        g.across = Vf([normal[2], 0.0, -normal[0], 0.0]);
        g.width = F(p.range(10.0, 60.0));
        g.height = F(p.range(5.0, 20.0));
        g.reach_sq = F::ZERO;
    }
    g
}

#[test]
fn the_lap_line_matches_the_score_cards_line_test() {
    let Some(mut ee) = game_or_skip() else { return };
    let line_test = match addr("score_lap_line") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    // Every listener slot of the three that can hear lands on a hook that writes down who and which.
    let heard: Rc<RefCell<Vec<(u32, u32)>>> = Rc::default();
    for who in [OWNER, CHAIN, TO_LISTENER] {
        for slot in (0..0x80).step_by(4) {
            let at = L_HOOKS + who * 0x100 + slot;
            ee.set_u32(L_VT + who * 0x100 + slot, at);
            let heard = heard.clone();
            ee.hooks.insert(at, Box::new(move |_| heard.borrow_mut().push((who, slot))));
        }
    }

    let mut p = Pick(0x3a9c_5e17_0d44_b2f1);
    let mut said = [0usize; 9];
    let mut counted = 0;
    for case in 0..20_000 {
        let lines = p.below(5) as usize;
        let g = finish_line(&mut p);
        let length = p.range(100.0, 2000.0);
        let course = Course { line: g, line_spot: spot(&mut p), lap_length: F(p.stray(length)) };
        let mut rules = LapRules {
            flags: (p.next() as u32 & !0x410) | if p.below(2) == 0 { race_flags::KEEP_LAPS } else { 0 }
                | if p.below(5) == 0 { race_flags::FINISHED } else { 0 },
            kind: if p.below(3) == 0 { race_kind::GHOST_RACE } else { p.below(6) as i32 },
            laps: 1 + p.below(6) as i32,
            ghost_runs: p.below(5) as i32,
            record_limit: p.below(12) as i32,
            free_records: p.below(4) as u32,
            ..LapRules::default()
        };
        let mut card = lap_card(&mut p, lines);
        // A lap exactly as quick as the best one.
        if p.below(8) == 0 {
            card.best_lap = F(fpu::sub(card.race_time.0, card.lap_start.0));
        }
        let (from, to) = step(&mut p, &g);
        card.seen = Seen { now: from, last: to };
        // Past the pool, the original takes the card's oldest record and writes through a stale
        // register when it has none (see `LapRules::line`): give such a card one.
        let lap = if rules.kind == race_kind::GHOST_RACE { card.runs + 1 } else { card.laps + 1 };
        if lap >= rules.record_limit && card.records.is_empty() {
            card.records.push(record(&mut p));
        }
        let owner = p.below(5) != 0;
        let chain = p.below(3) == 0;
        let mut ghost = (owner && p.below(5) != 0).then(|| lap_card(&mut p, lines));
        // A ghost exactly as quick as the card's best, or as the lap being run.
        if let Some(gc) = ghost.as_mut() {
            match p.below(6) {
                0 => gc.best_lap = card.best_lap,
                1 => gc.best_lap = F(fpu::sub(card.race_time.0, card.lap_start.0)),
                _ => {}
            }
        }

        // The manager, the track and the line.
        ee.set_u32(L_MGR + 0x30, L_LISTENER);
        ee.set_u32(L_MGR + 0x4c, rules.record_limit as u32);
        ee.set_u32(L_MGR + 0x5c, rules.flags);
        ee.set_u32(L_MGR + 0x60, rules.kind as u32);
        ee.set_u32(L_MGR + 0x68, L_TRACK);
        ee.set_u32(L_MGR + 0x88, lines as u32);
        ee.set_u32(L_MGR + 0x94, rules.laps as u32);
        ee.set_u32(L_MGR + 0xf0, rules.ghost_runs as u32);
        for k in 0..rules.free_records {
            let next = if k + 1 < rules.free_records { L_POOL + 0x20 * (k + 1) } else { 0 };
            put_record(&mut ee, L_POOL + 0x20 * k, &record(&mut p), next);
        }
        ee.set_u32(L_MGR + 0x58, if rules.free_records == 0 { 0 } else { L_POOL });
        put_f(&mut ee, L_TRACK + 0x54, course.lap_length);
        ee.set_u32(L_TRACK + 0x5c, L_LINE);
        put_gate(&mut ee, L_LINE, &g);
        put_spot(&mut ee, L_LINE + 0x94, course.line_spot);
        ee.set_u32(L_LISTENER + 4, 0);
        ee.set_u32(L_LISTENER + 0x1c, L_VT + TO_LISTENER * 0x100);
        // The card, its owner (carrying the ghost's card) and its chain.
        put_lap_card(&mut ee, L_CARD, &card, L_LINES, L_RECORDS);
        ee.set_u32(L_CARD + 4, if owner { L_OWNER } else { 0 });
        ee.set_u32(L_CARD + 0x1c4, if chain { L_CHAIN } else { 0 });
        ee.set_u32(L_OWNER + 8, if ghost.is_some() { L_GHOST } else { 0 });
        ee.set_u32(L_OWNER + 0x1c, 0);
        ee.set_u32(L_OWNER + 0x20, L_VT + OWNER * 0x100);
        ee.set_u32(L_CHAIN + 0x1c, 0);
        ee.set_u32(L_CHAIN + 0x20, L_VT + CHAIN * 0x100);
        if let Some(gc) = &ghost {
            put_lap_card(&mut ee, L_GHOST, gc, L_GHOST_LINES, L_RECORDS + 0x400);
        }
        heard.borrow_mut().clear();
        ee.call(line_test, &[u64::from(L_MGR), u64::from(L_CARD)], &[], BUDGET);
        let answer = ee.v0() as u32;

        let (got, notices) = rules.line(&mut card, ghost.as_mut(), &course);
        let what = format!("case {case}: {rules:?} {g:?}");
        assert_eq!(answer, u32::from(got), "{what}: the answer");
        let want_heard: Vec<(u32, u32)> = notices.iter().flat_map(|&n| told(n, owner, chain)).collect();
        assert_eq!(*heard.borrow(), want_heard, "{what}: the listeners ({notices:?})");
        assert_eq!(lap_card_words(&lap_card_at(&ee, L_CARD, lines)), lap_card_words(&card), "{what}: the card");
        if let Some(gc) = &ghost {
            assert_eq!(lap_card_words(&lap_card_at(&ee, L_GHOST, lines)), lap_card_words(gc), "{what}: the ghost's card");
        }
        assert_eq!(ee.u32_at(L_MGR + 0x5c), rules.flags, "{what}: the manager's flags");
        let mut free = 0;
        let mut r = ee.u32_at(L_MGR + 0x58);
        while r != 0 && free < 64 {
            free += 1;
            r = ee.u32_at(r + 0x14);
        }
        assert_eq!(free, rules.free_records, "{what}: the free records");
        for n in notices {
            said[n as usize] += 1;
        }
        counted += usize::from(got);
    }
    // Every notice comes up often enough to mean something.
    for (n, k) in said.iter().enumerate() {
        assert!(*k > 100, "notice {n} came up only {k} times in 20,000: {said:?}");
    }
    assert!(counted > 2000, "only {counted} lines counted");
}

// --- S13e: the track's race file ---------------------------------------------------------------------

/// Free RAM for the race file's track object, the name it is opened by, and the heap the reader's
/// allocations are handed out of.
const TRACK_OBJ: u32 = 0x0193_0000;
const TRACK_NAME: u32 = 0x0193_1000;
const HEAP: u32 = 0x0194_0000;

/// A race file being written, record by record.
struct Tdf(Vec<u8>);

impl Tdf {
    fn tag(&mut self, t: u8) -> &mut Tdf {
        self.0.push(t);
        self
    }
    fn w(&mut self, v: u32) -> &mut Tdf {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn i(&mut self, v: i32) -> &mut Tdf {
        self.w(v as u32)
    }
    fn f(&mut self, x: f32) -> &mut Tdf {
        self.w(x.to_bits())
    }
}

/// A float in `lo..hi`, now and then a stray word.
fn coord(p: &mut Pick, lo: f32, hi: f32) -> f32 {
    let x = p.range(lo, hi);
    p.stray(x)
}

/// A word for a field the reader only copies: small numbers mostly, any bits sometimes.
fn any_word(p: &mut Pick) -> u32 {
    if p.below(3) == 0 { p.next() as u32 } else { p.below(8) as u32 }
}

fn gate_record(t: &mut Tdf, p: &mut Pick) {
    let a = p.range(0.0, 6.3);
    t.f(coord(p, -300.0, 300.0)).f(coord(p, -20.0, 60.0)).f(coord(p, -300.0, 300.0));
    let tilt = if p.below(3) == 0 { p.range(-0.1, 0.1) } else { 0.0 };
    t.f(p.stray(a.sin())).f(p.stray(tilt)).f(p.stray(a.cos()));
    // The file's across direction is not a unit vector (sx01's is ~28 long); now and then zero.
    let l = match p.below(10) {
        0 => 0.0,
        1 => 1.0,
        _ => p.range(0.01, 60.0),
    };
    t.f(p.stray(a.cos() * l)).f(p.stray(tilt * l)).f(p.stray(-a.sin() * l));
    t.f(coord(p, 0.0, 100.0));
}

/// A generated race file: the path count, paths of nodes, checkpoints with parts and links, gates,
/// the header and tunables anywhere, unknown tags, and — now and then — a record the reader refuses.
fn race_file(p: &mut Pick) -> Vec<u8> {
    let mut t = Tdf(Vec::new());
    let odd = |p: &mut Pick| p.below(250) == 0;
    let header = |t: &mut Tdf, p: &mut Pick| match p.below(9) {
        0 => {
            t.tag(2).f(coord(p, -300.0, 300.0));
        }
        1 => {
            t.tag(3).f(coord(p, -300.0, 300.0));
        }
        2 => {
            let s = [1.0, 3.0, 0.5, 2.5][p.below(4) as usize];
            t.tag(4).f(p.stray(s));
        }
        3 => {
            t.tag(0x29).w(any_word(p));
        }
        4 => {
            let tag = [0x13, 0x14, 0x15, 0x16, 0x28][p.below(5) as usize];
            t.tag(tag).f(coord(p, 0.0, 200.0));
        }
        5 => {
            t.tag([0x0f, 0x10, 0x19][p.below(3) as usize]);
            gate_record(t, p);
        }
        6 => {
            t.tag(0x0c).w(any_word(p));
        }
        7 => {
            // An unknown tag: its byte alone.
            t.tag([0x00, 0x11, 0x27, 0x2a, 0x40, 0xff][p.below(6) as usize]);
        }
        _ => {}
    };
    for _ in 0..p.below(4) {
        header(&mut t, p);
    }
    let count = if p.below(60) == 0 { [0, -1, -7][p.below(3) as usize] } else { 1 + p.below(4) as i32 };
    t.tag(1).i(count);
    let mut part_seen = false;
    for _ in 0..1 + p.below(count.max(1) as u64 + 1) {
        let k = if odd(p) { [-1, count][p.below(2) as usize] } else { p.below(count.max(1) as u64) as i32 };
        t.tag(5).i(k);
        let mut nodes = 0;
        let mut record = false;
        let mut parts = 0;
        let mut links: Option<i32> = None;
        for _ in 0..p.below(40) {
            match p.below(16) {
                0 => {
                    let tag = [6, 7, 0x18][p.below(3) as usize];
                    t.tag(tag).w(any_word(p));
                }
                1..=3 => {
                    t.tag(8);
                    nodes += 1;
                    for _ in 0..p.below(8) {
                        match p.below(7) {
                            0..=2 => {
                                let tag = [9, 0x0a, 0x0b][p.below(3) as usize];
                                t.tag(tag).f(coord(p, -300.0, 300.0)).f(coord(p, -20.0, 60.0)).f(coord(p, -300.0, 300.0));
                            }
                            _ => {
                                let tag = [0x17, 0x12, 0x1a, 0x1b][p.below(4) as usize];
                                t.tag(tag).w(any_word(p));
                            }
                        }
                    }
                }
                4 => {
                    // A point every node has: the path's line.
                    if nodes > 0 || odd(p) {
                        t.tag(9).f(coord(p, -300.0, 300.0)).f(coord(p, -20.0, 60.0)).f(coord(p, -300.0, 300.0));
                    }
                }
                5 => {
                    let n = if odd(p) { -1 } else { p.below(5) as i32 };
                    t.tag(0x1c).i(n);
                }
                6 | 7 => {
                    t.tag(0x1d);
                    record = true;
                    parts = 0;
                    for _ in 0..p.below(4) {
                        let tag = [0x21, 0x1e, 0x1f, 0x20][p.below(4) as usize];
                        t.tag(tag).w(any_word(p));
                    }
                }
                8 | 9 => {
                    if record || odd(p) {
                        t.tag(0x22);
                        parts += 1;
                        part_seen = true;
                    }
                    for _ in 0..p.below(5) {
                        match p.below(4) {
                            // A point needs a part in the checkpoint at hand.
                            0 if parts > 0 => {
                                t.tag(0x24).f(coord(p, -300.0, 300.0)).f(coord(p, -20.0, 60.0)).f(coord(p, -300.0, 300.0));
                            }
                            // The words go to the last part made, whichever checkpoint it is in.
                            1..=3 if part_seen || odd(p) => {
                                let tag = [0x23, 0x25, 0x26][p.below(3) as usize];
                                t.tag(tag).w(any_word(p));
                            }
                            _ => {}
                        }
                    }
                }
                10 => {
                    let n = if odd(p) { -1 } else { 1 + p.below(3) as i32 };
                    t.tag(0x0d).i(n);
                    links = Some(n);
                }
                11 => {
                    if let Some(n) = links.filter(|&n| n > 0) {
                        let c = if odd(p) { -1 } else { p.below(n as u64) as i32 };
                        let v = if odd(p) { [-1, count + 1][p.below(2) as usize] } else { p.below(count.max(0) as u64 + 1) as i32 };
                        t.tag(0x0e).i(c).i(v);
                    } else if odd(p) {
                        t.tag(0x0e).i(0).i(0);
                    }
                }
                12 if p.below(100) == 0 => {
                    // The count again: every slot emptied, the path at hand kept on.
                    t.tag(1).i(1 + p.below(4) as i32);
                }
                _ => header(&mut t, p),
            }
        }
    }
    if p.below(8) == 0 {
        // A record with no words as the file's last byte: never read.
        t.tag([8, 0x1d, 0x22, 0x05][p.below(4) as usize]);
    }
    t.0
}

/// A race file, read back out of the track object the original's reader filled, as the port holds
/// it. A link is turned back into a path index when it is a path's address.
fn track_at(ee: &Ee, at: u32) -> TrackFile {
    let f = |a: u32| F(f32::from_bits(ee.u32_at(a)));
    let count = ee.u32_at(at + 0x40) as i32;
    let slots: Vec<u32> = (0..count.max(0) as u32).map(|k| ee.u32_at(ee.u32_at(at + 0x50) + 4 * k)).collect();
    let gate = |o: u32| match ee.u32_at(at + o) {
        0 => None,
        g => Some(gate_at(ee, g)),
    };
    let paths = slots
        .iter()
        .map(|&p| {
            if p == 0 {
                return None;
            }
            let mut nodes = Vec::new();
            let mut n = ee.u32_at(p + 0x10);
            while n != 0 {
                nodes.push(PathNode {
                    id: ee.u32_at(n) as i32,
                    word_04: ee.u32_at(n + 4),
                    points: [quad(ee, n + 0x10), quad(ee, n + 0x20), quad(ee, n + 0x30)],
                    span: f(n + 0x40),
                    word_44: ee.u32_at(n + 0x44),
                    word_48: ee.u32_at(n + 0x48),
                    word_4c: ee.u32_at(n + 0x4c),
                });
                n = ee.u32_at(n + 0x54);
            }
            let links = match ee.u32_at(p + 0x2c) {
                0 => None,
                l => Some(
                    (0..ee.u32_at(p + 0x18))
                        .map(|c| {
                            let w = ee.u32_at(l + 4 * c);
                            match slots.iter().position(|&s| s == w && s != 0) {
                                Some(k) => Link::Path(k),
                                None => Link::Raw(w as i32),
                            }
                        })
                        .collect(),
                ),
            };
            let mut checkpoints = Vec::new();
            let mut r = ee.u32_at(p + 0x24);
            while r != 0 {
                let mut parts = Vec::new();
                let mut s = ee.u32_at(r + 0x14);
                while s != 0 {
                    parts.push(CheckPart {
                        word_04: ee.u32_at(s + 4),
                        point: quad(ee, s + 0x10),
                        span: f(s + 0x20),
                        word_24: ee.u32_at(s + 0x24),
                        word_28: ee.u32_at(s + 0x28),
                    });
                    s = ee.u32_at(s + 0x30);
                }
                checkpoints.push(Checkpoint {
                    id: ee.u32_at(r) as i32,
                    word_04: ee.u32_at(r + 4),
                    word_08: ee.u32_at(r + 8),
                    word_0c: ee.u32_at(r + 0xc),
                    word_10: ee.u32_at(r + 0x10),
                    parts,
                });
                r = ee.u32_at(r + 0x20);
            }
            Some(TrackPath {
                flags: ee.u32_at(p),
                length: f(p + 8),
                word_0c: ee.u32_at(p + 0xc),
                nodes,
                links,
                checkpoint_count: ee.u32_at(p + 0x20) as i32,
                checkpoints,
            })
        })
        .collect();
    TrackFile {
        offset_x: f(at + 0x80),
        offset_z: f(at + 0x88),
        scale: f(at + 0x90),
        word_34: ee.u32_at(at + 0x34),
        paths,
        nodes_made: ee.u32_at(at + 0x44) as i32,
        checkpoints_made: ee.u32_at(at + 0x48) as i32,
        grid: gate(0x58),
        line: gate(0x5c),
        third: gate(0x60),
        tunables: [0x64, 0x68, 0x6c, 0x70, 0x74].map(|o| f(at + o)),
    }
}

/// Every word of a race file, in a fixed order, so that two compare bit for bit (NaNs too). A link to
/// an empty slot is the null address it is in memory.
fn track_words(t: &TrackFile) -> Vec<u32> {
    let mut w = vec![t.offset_x.0.to_bits(), t.offset_z.0.to_bits(), t.scale.0.to_bits(), t.word_34];
    w.extend([t.nodes_made as u32, t.checkpoints_made as u32, t.paths.len() as u32]);
    w.extend(t.tunables.map(|x| x.0.to_bits()));
    let q = |w: &mut Vec<u32>, v: Vf| w.extend(v.to_bits());
    for g in [&t.grid, &t.line, &t.third] {
        match g {
            None => w.push(0xdead_0000),
            Some(g) => {
                w.extend([g.flags, g.width.0.to_bits(), g.height.0.to_bits(), g.reach_sq.0.to_bits()]);
                q(&mut w, g.centre);
                q(&mut w, g.normal);
                q(&mut w, g.across);
            }
        }
    }
    for path in &t.paths {
        let Some(path) = path else {
            w.push(0xdead_0001);
            continue;
        };
        w.extend([path.flags, path.length.0.to_bits(), path.word_0c, path.checkpoint_count as u32, path.nodes.len() as u32]);
        for n in &path.nodes {
            w.extend([n.id as u32, n.word_04, n.span.0.to_bits(), n.word_44, n.word_48, n.word_4c]);
            n.points.iter().for_each(|&v| q(&mut w, v));
        }
        match &path.links {
            None => w.push(0xdead_0002),
            Some(links) => {
                w.push(links.len() as u32);
                for l in links {
                    w.push(match *l {
                        Link::Path(k) if t.paths[k].is_some() => 0x8000_0000 | k as u32,
                        Link::Path(_) => 0,
                        Link::Raw(v) => v as u32,
                    });
                }
            }
        }
        w.push(path.checkpoints.len() as u32);
        for c in &path.checkpoints {
            w.extend([c.id as u32, c.word_04, c.word_08, c.word_0c, c.word_10, c.parts.len() as u32]);
            for s in &c.parts {
                w.extend([s.word_04, s.span.0.to_bits(), s.word_24, s.word_28]);
                q(&mut w, s.point);
            }
        }
    }
    w
}

/// The reader's file and heap, behind its hooks.
struct Disc {
    file: Vec<u8>,
    at: usize,
    heap: u32,
}

fn take(d: &RefCell<Disc>, size: u32) -> u32 {
    let mut d = d.borrow_mut();
    let at = d.heap;
    d.heap = (d.heap + size.max(1) + 15) & !15;
    at
}

#[test]
fn the_race_file_reader_matches_the_track_objects_loader() {
    let Some(mut ee) = game_or_skip() else { return };
    let names = [
        "track_ctor",
        "track_file_load",
        "fs_open",
        "fs_read",
        "fs_eof",
        "fs_close",
        "mem_calloc",
        "operator_new",
        "node_add_child",
        "track_file_fail",
        "track_first_path",
        "track_gate_spot",
        "path_spot",
        "track_start_spots",
    ];
    let [ctor, load, open, read, eof, close, calloc, new, add_child, fail, first_path, gate_spot, spot, start_spots] =
        match addrs(names) {
            Ok(a) => a,
            Err(why) => return skip(&why),
        };
    let disc = Rc::new(RefCell::new(Disc { file: Vec::new(), at: 0, heap: HEAP }));
    ee.hooks.insert(open, Box::new(|ee| ee.set(2, 1)));
    let d = disc.clone();
    ee.hooks.insert(
        read,
        Box::new(move |ee| {
            let (dst, n) = (ee.r(6) as u32, ee.r(7) as usize);
            assert!(n == 1 || n == 4, "a read of {n} bytes");
            let mut d = d.borrow_mut();
            let end = (d.at + n).min(d.file.len());
            let bytes = d.file[d.at.min(end)..end].to_vec();
            d.at = end.max(d.at);
            ee.load(dst, &bytes);
            ee.set(2, bytes.len() as u64);
        }),
    );
    let d = disc.clone();
    ee.hooks.insert(
        eof,
        Box::new(move |ee| {
            let d = d.borrow();
            ee.set(2, if d.at < d.file.len() { 0 } else { u64::MAX });
        }),
    );
    let d = disc.clone();
    ee.hooks.insert(
        calloc,
        Box::new(move |ee| {
            let size = (ee.r(4) as u32).wrapping_mul(ee.r(5) as u32);
            assert!(size < 0x1_0000, "an allocation of {size:#x}");
            let at = take(&d, size);
            ee.load(at, &vec![0; size as usize]);
            ee.set(2, u64::from(at));
        }),
    );
    let d = disc.clone();
    ee.hooks.insert(
        new,
        Box::new(move |ee| {
            let size = ee.r(4) as u32;
            let at = take(&d, size);
            // Not cleared: what the constructor does not set shows.
            ee.load(at, &vec![0xa5; size as usize]);
            ee.set(2, u64::from(at));
        }),
    );
    for at in [close, add_child, fail, first_path, spot, start_spots] {
        ee.hooks.insert(at, Box::new(|_| {}));
    }
    ee.hooks.insert(gate_spot, Box::new(|ee| ee.set(2, 1)));

    let mut p = Pick(0x7d15_0e13_2c9b_5a41);
    let (mut read_ok, mut failed, mut gates, mut records, mut parts, mut links) = (0, 0, 0, 0, 0, 0);
    for case in 0..12_000 {
        let file = race_file(&mut p);
        {
            let mut d = disc.borrow_mut();
            d.file = file.clone();
            d.at = 0;
            d.heap = HEAP;
        }
        ee.load(TRACK_OBJ, &[0x5a; 0x100]);
        ee.call(ctor, &[u64::from(TRACK_OBJ)], &[], BUDGET);
        // The two words the constructor leaves: the port starts them at 0.
        ee.set_u32(TRACK_OBJ + 0x80, 0);
        ee.set_u32(TRACK_OBJ + 0x88, 0);
        ee.call(load, &[u64::from(TRACK_OBJ), u64::from(TRACK_NAME)], &[], 20_000_000);
        let want = ee.v0() as u32;
        match TrackFile::parse(&file) {
            Ok(t) => {
                assert_eq!(want, 1, "case {case}: the original failed a file the port read: {file:02x?}");
                let got = track_at(&ee, TRACK_OBJ);
                if track_words(&t) != track_words(&got) {
                    panic!("case {case}: {file:02x?}\nport {t:#?}\noriginal {got:#?}");
                }
                read_ok += 1;
                gates += [&t.grid, &t.line, &t.third].iter().filter(|g| g.is_some()).count();
                for path in t.paths.iter().flatten() {
                    records += path.checkpoints.len();
                    parts += path.checkpoints.iter().map(|c| c.parts.len()).sum::<usize>();
                    links += path.links.as_ref().map_or(0, |l| l.len());
                }
            }
            Err(Refused::Fails) => {
                assert_eq!(want, 0, "case {case}: the original read a file the port fails: {file:02x?}");
                failed += 1;
            }
            Err(why) => panic!("case {case}: the generator wrote a file the port refuses ({why:?}): {file:02x?}"),
        }
    }
    assert!(read_ok > 8_000 && failed > 500, "{read_ok} read, {failed} failed");
    assert!(gates > 10_000 && records > 10_000 && parts > 10_000 && links > 5_000, "{gates} gates, {records} checkpoints, {parts} parts, {links} links");
}

/// sx01's race file off the disc: `tracks\sx01\sx01.tdf` in `Tracks\sx01_2.res`, inside MX.PSR.
fn sx01_race_file() -> Option<Vec<u8>> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("Tracks\\sx01_2.res").expect("sx01's second track archive");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("the archive's layout");
    let m = res.find("tracks\\sx01\\sx01.tdf").expect("sx01's race file");
    Some(res.read(&bytes, m).expect("inflate the race file"))
}

fn gate_words(g: &Gate) -> Vec<u32> {
    let mut w = vec![g.flags, g.width.0.to_bits(), g.height.0.to_bits(), g.reach_sq.0.to_bits()];
    for v in [g.centre, g.normal, g.across] {
        w.extend(v.to_bits());
    }
    w
}

#[test]
fn sx01s_race_file_off_the_disc_is_the_track_object_in_the_image() {
    let Some(file) = sx01_race_file() else { return };
    let Some(ee) = sx01_image() else { return };
    let manager = match addr("activity_manager") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let t = TrackFile::parse(&file).expect("sx01's race file reads");
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    let track = ee.u32_at(act + 0xb8);
    let ram = track_at(&ee, track);

    // The file's own words, bit for bit.
    let head = |t: &TrackFile| {
        let mut w = vec![t.offset_x.0.to_bits(), t.offset_z.0.to_bits(), t.scale.0.to_bits(), t.word_34];
        w.extend([t.nodes_made as u32, t.checkpoints_made as u32, t.paths.len() as u32]);
        w.extend(t.tunables.map(|x| x.0.to_bits()));
        w
    };
    assert_eq!(head(&t), head(&ram), "the scale, offsets, counts and tunables");
    assert_eq!((t.paths.len(), t.nodes_made, t.checkpoints_made), (2, 96, 0), "two paths, 96 nodes, no checkpoints");
    // The third gate is the file's as read: nothing after the reader touches it.
    assert_eq!(gate_words(&t.third.expect("a third gate")), gate_words(&ram.third.expect("a third gate")), "the third gate");
    assert!(t.grid.is_some() && t.line.is_some(), "the file has a grid gate and a line");

    // The race's paths, the load's walk done: the image's flags exactly.
    let walked = TrackPaths::new(&t).expect("sx01's paths walk");
    assert_eq!(
        walked.paths.iter().map(|p| p.flags).collect::<Vec<_>>(),
        ram.paths.iter().map(|p| p.as_ref().expect("the image's path").flags).collect::<Vec<_>>(),
        "the paths' flags after the walk"
    );
    // The paths: every word but the points' heights, which the activity's loader puts on the ground.
    for (k, (a, b)) in t.paths.iter().zip(&ram.paths).enumerate() {
        let (a, b) = (a.as_ref().expect("the file's path"), b.as_ref().expect("the image's path"));
        // The walk from path 0 after the load takes 0x80 off every path it reaches (checked below,
        // through `TrackPaths::new`).
        assert_eq!(
            (a.flags & !0x80, a.length.0.to_bits(), a.word_0c, a.checkpoint_count, &a.links),
            (b.flags & !0x80, b.length.0.to_bits(), b.word_0c, b.checkpoint_count, &b.links),
            "path {k}"
        );
        assert_eq!(a.nodes.len(), b.nodes.len(), "path {k}'s nodes");
        for (n, m) in a.nodes.iter().zip(&b.nodes) {
            let flat = |p: &[Vf; 3]| p.map(|v| [v.x().to_bits(), v.z().to_bits(), v.w().to_bits()]);
            assert_eq!(
                (n.id, n.word_04, n.span.0.to_bits(), n.word_44, n.word_48, n.word_4c, flat(&n.points)),
                (m.id, m.word_04, m.span.0.to_bits(), m.word_44, m.word_48, m.word_4c, flat(&m.points)),
                "path {k}, node {}",
                n.id
            );
        }
    }
}

/// The prop whose frame matches: its position at `+0xb0` and its across row at `+0x80` (the `xyz`
/// lanes, or all four).
fn find_prop(ee: &Ee, at: Vf, across: Vf, lanes: usize) -> Vec<u32> {
    let (at, across) = (at.to_bits(), across.to_bits());
    (0x0010_0000u32..0x01ff_ff00)
        .step_by(16)
        .filter(|&a| (0..lanes as u32).all(|k| ee.u32_at(a + 0xb0 + 4 * k) == at[k as usize]))
        .filter(|&a| (0..lanes as u32).all(|k| ee.u32_at(a + 0x80 + 4 * k) == across[k as usize]))
        // A scene node: its bound is clean, or the slot that cleans it is code. (Copies of the frame
        // sit in other objects too.)
        .filter(|&a| {
            let table = ee.u32_at(a + 0x38);
            ee.u8_at(a + 0x110) & 0x80 == 0
                || (0x0010_0000..0x0200_0000).contains(&table) && (0x0010_0000..0x0050_0000).contains(&ee.u32_at(table + 0x88))
        })
        .collect()
}

#[test]
fn the_supercross_loader_stands_sx01s_gates_on_its_props() {
    let Some(file) = sx01_race_file() else { return };
    let Some(mut ee) = sx01_image() else { return };
    let [manager, bound, ahead_at] = match addrs(["activity_manager", "prop_bound", "path_ahead"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let t = TrackFile::parse(&file).expect("sx01's race file reads");
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    let track = ee.u32_at(act + 0xb8);
    let (grid_at, line_at, start_at) = (ee.u32_at(track + 0x58), ee.u32_at(track + 0x5c), ee.u32_at(act + 0x3dc));
    let (want_grid, want_line, want_start) = (gate_at(&ee, grid_at), gate_at(&ee, line_at), gate_at(&ee, start_at));
    let (c, h) = (ee.scratch(0), ee.scratch(1));
    let mut prop = |ee: &mut Ee, a: u32| {
        ee.call(bound, &[u64::from(a), u64::from(c), u64::from(h)], &[], 1_000_000);
        Prop {
            across: quad(ee, a + 0x80),
            look: quad(ee, a + 0xa0),
            at: quad(ee, a + 0xb0),
            half_width: f32_at(ee, h),
            height: f32_at(ee, h + 4),
        }
    };

    // The drop gate: the grid gate stands in it, the start gate 4.5 behind it.
    let drops = find_prop(&ee, want_grid.centre, want_grid.across, 4);
    let drop = drops
        .iter()
        .map(|&a| prop(&mut ee, a))
        .find(|p| fpu::mul(2.0, p.half_width.0).to_bits() == want_grid.width.0.to_bits())
        .unwrap_or_else(|| panic!("no drop-gate prop of the grid's width among {drops:x?}"));
    let mut grid = t.grid.expect("the file's grid gate");
    let mut start = Gate::new();
    Supercross::place_drop_gate(&drop, Some(&mut grid), &mut start);
    assert_eq!(gate_words(&grid), gate_words(&want_grid), "the grid gate: {grid:?} against {want_grid:?}");
    // The start gate has been through a reset since, which closes it.
    start.flags |= flags::CLOSED;
    assert_eq!(gate_words(&start), gate_words(&want_start), "the start gate: {start:?} against {want_start:?}");

    // The finish gate: the line stands in it, facing along the path at its spot. Its bound has
    // changed since the load — no prop with its frame holds half extents of the line's width and
    // height any more — so those two come from the image's line, and what is proved here is the rest:
    // the centre, the across direction and the normal worked out from the path's direction.
    let finishes = find_prop(&ee, want_line.centre, want_line.across, 3);
    let first = *finishes.first().expect("a prop standing where the line is");
    let finish = Prop {
        half_width: F(fpu::mul(0.5, want_line.width.0)),
        height: want_line.height,
        ..prop(&mut ee, first)
    };
    let out = ee.scratch(2);
    ee.call(ahead_at, &[u64::from(track), u64::from(ee.u32_at(line_at + 0x98)), u64::from(out)], &[], 1_000_000);
    let ahead = quad(&ee, out);
    let mut line = t.line.expect("the file's line");
    Supercross::place_finish_line(&finish, ahead, &mut line);
    assert_eq!(gate_words(&line), gate_words(&want_line), "the line: {line:?} against {want_line:?}");
}

// --- S293: the track's paths, as the race asks them ---------------------------------------------------

/// Free RAM for a track object's paths: the object, its path array, the path records and their link
/// arrays, the nodes (16 slots a path), the three pools, and the queries' arguments.
const T_TRACK: u32 = 0x01b0_0000;
const T_PATHS: u32 = 0x01b0_0100;
const T_RECS: u32 = 0x01b0_1000;
const T_LINKS: u32 = 0x01b0_4000;
const T_NODES: u32 = 0x01b1_0000;
const T_ROUTES: u32 = 0x01b2_0000;
const T_PARENTS: u32 = 0x01b3_0000;
const T_CANDS: u32 = 0x01b4_0000;
const T_A: u32 = 0x01b5_0000;
const T_B: u32 = 0x01b5_0010;
const T_OUT: u32 = 0x01b5_0020;
const T_D: u32 = 0x01b5_0030;
const T_ROUTE: u32 = 0x01b5_0040;
const T_POINT: u32 = 0x01b5_0050;
const T_VIA: u32 = 0x01b5_0100;

fn t_rec(i: usize) -> u32 {
    T_RECS + 0x40 * i as u32
}

fn t_node(i: usize, n: usize) -> u32 {
    T_NODES + 0x60 * (16 * i + n) as u32
}

/// A spot of the port's into RAM as the original's three words.
fn put_t_spot(ee: &mut Ee, at: u32, s: Spot) {
    let i = s.path.saturating_sub(1) as usize;
    ee.set_u32(at, if s.path == 0 { 0 } else { t_rec(i) });
    ee.set_u32(at + 4, if s.piece == 0 { 0 } else { t_node(i, s.piece as usize - 1) });
    put_f(ee, at + 8, s.along);
}

/// A spot's three words, its `along` as bits.
fn t_bits(s: Spot) -> (u32, u32, u32) {
    (s.path, s.piece, s.along.0.to_bits())
}

fn t_spot_at(ee: &Ee, at: u32) -> Spot {
    let (path, node) = (ee.u32_at(at), ee.u32_at(at + 4));
    let p = if path == 0 { 0 } else { (path - T_RECS) / 0x40 + 1 };
    let n = if node == 0 { 0 } else { (node - T_NODES) / 0x60 % 16 + 1 };
    Spot { path: p, piece: n, along: f32_at(ee, at + 8) }
}

/// A free list of `n` entries of `size` bytes at `at`, linked through `next`; the words the queries
/// write before reading are stray, and the route entries' direction byte is 0 as the pool's calloc
/// leaves it.
fn put_pool(ee: &mut Ee, p: &mut Pick, at: u32, n: usize, size: u32, next: u32) -> u32 {
    for k in 0..n as u32 {
        let e = at + size * k;
        for o in (0..size).step_by(4) {
            ee.set_u32(e + o, p.next() as u32 & !0xff);
        }
        ee.set_u32(e + next, if k + 1 < n as u32 { e + size } else { 0 });
    }
    if n == 0 { 0 } else { at }
}

fn pool_len(ee: &Ee, head: u32, next: u32) -> usize {
    let (mut n, mut e) = (0, head);
    while e != 0 && n < 10_000 {
        n += 1;
        e = ee.u32_at(e + next);
    }
    n
}

/// The paths into RAM as the track object holds them.
fn put_paths(ee: &mut Ee, p: &mut Pick, t: &TrackPaths) {
    ee.set_u32(T_TRACK + 0x34, t.word_34);
    put_f(ee, T_TRACK + 0x38, t.ahead);
    put_f(ee, T_TRACK + 0x3c, t.behind);
    ee.set_u32(T_TRACK + 0x40, t.paths.len() as u32);
    ee.set_u32(T_TRACK + 0x50, T_PATHS);
    put_f(ee, T_TRACK + 0x54, t.lap_length);
    for (i, path) in t.paths.iter().enumerate() {
        let r = t_rec(i);
        ee.set_u32(T_PATHS + 4 * i as u32, r);
        for o in (0..0x30).step_by(4) {
            ee.set_u32(r + o, p.next() as u32);
        }
        ee.set_u32(r, path.flags);
        put_f(ee, r + 8, path.length);
        ee.set_u32(r + 0x10, t_node(i, 0));
        ee.set_u32(r + 0x14, t_node(i, path.nodes.len() - 1));
        ee.set_u32(r + 0x18, path.links.len() as u32);
        put_f(ee, r + 0x1c, path.dist);
        ee.set_u32(r + 0x28, path.best.map_or(0, t_rec));
        ee.set_u32(r + 0x2c, T_LINKS + 0x40 * i as u32);
        for (k, &l) in path.links.iter().enumerate() {
            ee.set_u32(T_LINKS + 0x40 * i as u32 + 4 * k as u32, t_rec(l));
        }
        for (n, node) in path.nodes.iter().enumerate() {
            let a = t_node(i, n);
            for o in (0..0x60).step_by(4) {
                ee.set_u32(a + o, p.next() as u32);
            }
            for (k, pt) in node.points.iter().enumerate() {
                put_quad(ee, a + 0x10 + 0x10 * k as u32, *pt);
            }
            put_f(ee, a + 0x40, node.span);
            ee.set_u32(a + 0x50, if n == 0 { 0 } else { t_node(i, n - 1) });
            ee.set_u32(a + 0x54, if n + 1 == path.nodes.len() { 0 } else { t_node(i, n + 1) });
        }
    }
    ee.set_u32(T_TRACK + 0x94, T_ROUTES);
    ee.set_u32(T_TRACK + 0x9c, T_PARENTS);
    ee.set_u32(T_TRACK + 0xa4, T_CANDS);
    let routes = put_pool(ee, p, T_ROUTES, t.free_routes, 0x10, 0xc);
    for k in 0..t.free_routes as u32 {
        ee.set_u32(T_ROUTES + 0x10 * k, 0);
    }
    ee.set_u32(T_TRACK + 0x98, routes);
    let parents = put_pool(ee, p, T_PARENTS, t.free_parents, 0xc, 8);
    ee.set_u32(T_TRACK + 0xa0, parents);
    let cands = put_pool(ee, p, T_CANDS, t.free_candidates, 0x10, 0xc);
    ee.set_u32(T_TRACK + 0xa8, cands);
    // A route walked off a path with no best link reads on from a null path (see `build_route`).
    ee.set_u32(0x28, 0);
}

/// The paths' scratch words and the pools, read back and held against the port's.
fn check_paths(ee: &Ee, t: &TrackPaths, what: &str) {
    for (i, path) in t.paths.iter().enumerate() {
        let r = t_rec(i);
        assert_eq!(ee.u32_at(r), path.flags, "{what}: path {i}'s flags");
        assert_eq!(ee.u32_at(r + 0x1c), path.dist.0.to_bits(), "{what}: path {i}'s distance");
        assert_eq!(ee.u32_at(r + 0x28), path.best.map_or(0, t_rec), "{what}: path {i}'s best link");
    }
    assert_eq!(pool_len(ee, ee.u32_at(T_TRACK + 0x98), 0xc), t.free_routes, "{what}: the route pool");
    assert_eq!(pool_len(ee, ee.u32_at(T_TRACK + 0xa0), 8), t.free_parents, "{what}: the parent pool");
    assert_eq!(pool_len(ee, ee.u32_at(T_TRACK + 0xa8), 0xc), t.free_candidates, "{what}: the candidate pool");
}

/// A flat length the way the file's reader takes it.
fn t_span(a: Vf, b: Vf) -> F {
    let (dx, dz) = (fpu::sub(b.x(), a.x()), fpu::sub(b.z(), a.z()));
    F(fpu::sqrt(fpu::madd(fpu::mula(dx, dx), dz, dz)))
}

/// A generated course: 1–7 paths of 1–10 nodes winding about the origin, each starting where an earlier
/// one ends or branching off one of its nodes, so that rings overlap; links onward (the last back to
/// the first) and a few more; stray flags, scratch words and spans now and then, and sometimes a pool
/// nearly empty.
fn t_paths(p: &mut Pick) -> TrackPaths {
    let count = 1 + p.below(7) as usize;
    let mut paths: Vec<RacePath> = Vec::new();
    for i in 0..count {
        let (mut at, mut heading) = if i == 0 || p.below(3) == 0 {
            ([p.range(-40.0, 40.0), p.range(-40.0, 40.0)], p.range(0.0, 6.3))
        } else {
            let from: &RacePath = &paths[p.below(i as u64) as usize];
            let n = if p.below(2) == 0 { from.nodes.len() - 1 } else { p.below(from.nodes.len() as u64) as usize };
            let q = from.nodes[n].points[0];
            ([q.x() + p.range(-3.0, 3.0), q.z() + p.range(-3.0, 3.0)], p.range(0.0, 6.3))
        };
        let width = p.range(4.0, 20.0);
        let n = 1 + p.below(10) as usize;
        let mut nodes = Vec::new();
        for k in 0..n {
            let (s, c) = heading.sin_cos();
            let y = p.range(-3.0, 3.0);
            let w = width * p.range(0.6, 1.4);
            let centre = Vf([at[0], y, at[1], 0.0]);
            let left = Vf([at[0] + c * w, y, at[1] - s * w, 0.0]);
            let right = Vf([at[0] - c * w, y, at[1] + s * w, 0.0]);
            let points = if p.below(10) == 0 { [centre, right, left] } else { [centre, left, right] };
            let points = if p.below(30) == 0 { [p.quad([at[0], y, at[1]]), points[1], points[2]] } else { points };
            nodes.push(PathNode { id: k as i32, word_04: 0, points, span: F::ZERO, word_44: 0, word_48: 0, word_4c: 0 });
            let step = if p.below(20) == 0 { 0.0 } else { p.range(5.0, 30.0) };
            at = [at[0] + s * step, at[1] + c * step];
            heading += p.range(-0.6, 0.6);
        }
        let mut length = F::ZERO;
        for k in 0..n - 1 {
            let span = t_span(nodes[k].points[0], nodes[k + 1].points[0]);
            nodes[k].span = F(p.stray(span.0));
            length += nodes[k].span;
        }
        // The file's 0xc1, after the load cleared 0x80 from the paths path 0 reaches.
        let mut flags = 0x41 | (p.next() as u32 & 0x2a);
        if p.below(6) == 0 {
            flags |= 0x100;
        }
        if p.below(8) == 0 {
            flags |= 0x80;
        }
        if p.below(8) == 0 {
            flags |= 0x40;
        }
        if p.below(12) == 0 {
            flags |= 0x10;
        }
        let dist = if p.below(2) == 0 {
            F::ZERO
        } else {
            let d = p.range(0.0, 500.0);
            F(p.stray(d))
        };
        let best = (p.below(3) == 0).then(|| p.below(count as u64) as usize);
        paths.push(RacePath { flags, length: F(p.stray(length.0)), nodes, links: Vec::new(), dist, best });
    }
    for i in 0..count {
        let mut links = vec![(i + 1) % count];
        for _ in 0..p.below(3) {
            links.push(p.below(count as u64) as usize);
        }
        if p.below(4) == 0 {
            let last = links.len() - 1;
            links.swap(0, last);
        }
        paths[i].links = links;
    }
    let pool = |p: &mut Pick| if p.below(8) == 0 { p.below(6) as usize } else { 10 * count };
    let (ahead, behind) = match p.below(4) {
        0 => (F(200.0), F(200.0)),
        1 => (F(125.0), F(75.0)),
        _ => (F(p.range(0.0, 150.0)), F(p.range(0.0, 150.0))),
    };
    TrackPaths {
        word_34: p.next() as u32 & 0x7,
        ahead,
        behind,
        lap_length: F(p.range(100.0, 900.0)),
        free_routes: pool(p),
        free_parents: pool(p),
        free_candidates: pool(p),
        paths,
    }
}

/// A spot on one of the paths, `along` in 0..1 (now and then exactly 0 or 1, a signed zero or a
/// denormal: the walks recurse for good on anything outside).
fn t_spot_on(p: &mut Pick, t: &TrackPaths) -> Spot {
    let i = p.below(t.paths.len() as u64) as usize;
    let n = p.below(t.paths[i].nodes.len() as u64) as usize;
    let along = match p.below(12) {
        0 => 0.0,
        1 => 1.0,
        2 => -0.0,
        3 => f32::from_bits(p.next() as u32 & 0x007f_ffff),
        _ => p.unit(),
    };
    Spot::on(i, n, F(along))
}

#[test]
fn spot_points_and_the_way_ahead_match_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let [point_fn, ahead_fn] = match addrs(["path_spot_point", "path_ahead"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x51c3_77e0_9a2d_4b18);
    let mut answers = [0usize; 4];
    for case in 0..20_000 {
        let t = t_paths(&mut p);
        put_paths(&mut ee, &mut p, &t);
        let mut spot = t_spot_on(&mut p, &t);
        if p.below(3) == 0 {
            let a = p.range(-1.0, 2.0);
            spot.along = F(p.stray(a));
        }
        if p.below(30) == 0 {
            spot.piece = 0;
        }
        if p.below(30) == 0 {
            spot = Spot::default();
        }
        let what = format!("case {case}: {spot:?}");
        let fill = Vf([p.unit(), p.unit(), p.unit(), p.unit()]);

        put_t_spot(&mut ee, T_A, spot);
        put_quad(&mut ee, T_OUT, fill);
        ee.call(point_fn, &[u64::from(T_TRACK), u64::from(T_A), u64::from(T_OUT)], &[], BUDGET);
        let (mut s, mut out) = (spot, fill);
        let got = t.spot_point(&mut s, &mut out);
        assert_eq!(ee.v0() as u32, u32::from(got), "{what}: the point's answer");
        assert_eq!(quad(&ee, T_OUT).to_bits(), out.to_bits(), "{what}: the point");
        assert_eq!(t_bits(t_spot_at(&ee, T_A)), t_bits(s), "{what}: the spot after");
        answers[usize::from(got)] += 1;

        let node = ee.u32_at(T_A + 4);
        put_quad(&mut ee, T_OUT, fill);
        ee.call(ahead_fn, &[u64::from(T_TRACK), u64::from(node), u64::from(T_OUT)], &[], BUDGET);
        let mut out = fill;
        let got = t.path_ahead(spot, &mut out);
        assert_eq!(ee.v0() as u32, u32::from(got), "{what}: the way ahead's answer");
        assert_eq!(quad(&ee, T_OUT).to_bits(), out.to_bits(), "{what}: the way ahead");
        answers[2 + usize::from(got)] += 1;
    }
    assert!(answers.iter().all(|&k| k > 1000), "{answers:?}");
}

/// The original's route list from its head word, as path indices.
fn t_route_at(ee: &Ee, head: u32) -> Vec<usize> {
    let (mut out, mut e) = (Vec::new(), ee.u32_at(head));
    while e != 0 && out.len() < 1000 {
        let path = ee.u32_at(e + 4);
        out.push(if path == 0 { usize::MAX } else { ((path - T_RECS) / 0x40) as usize });
        e = ee.u32_at(e + 0xc);
    }
    out
}

#[test]
fn distances_routes_and_walks_match_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let [distance_fn, route_fn, walk_fn, free_fn, reaches_fn] =
        match addrs(["path_distance", "track_start_spots", "path_route_walk", "path_route_free", "path_reaches"]) {
            Ok(a) => a,
            Err(why) => return skip(&why),
        };
    let mut p = Pick(0x0e6f_2d91_c4a8_3375);
    let (mut routed, mut failed, mut walked, mut kept_off) = (0, 0, 0, 0);
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        put_paths(&mut ee, &mut p, &t);
        let a = t_spot_on(&mut p, &t);
        let b = if p.below(4) == 0 { Spot { along: F(p.unit()), ..a } } else { t_spot_on(&mut p, &t) };
        let keep_off = p.below(2) == 0;
        let what = format!("case {case}: {a:?} to {b:?}, keep_off {keep_off}");
        put_t_spot(&mut ee, T_A, a);
        put_t_spot(&mut ee, T_B, b);

        ee.call(reaches_fn, &[u64::from(T_TRACK), u64::from(T_A), u64::from(T_B)], &[], BUDGET);
        let reaches = ee.v0() as u32 & 0xff;

        // The distance, then (on the paths as it left them) the route with its list, and a walk along it.
        ee.call(distance_fn, &[u64::from(T_TRACK), u64::from(T_A), u64::from(T_B), u64::from(keep_off)], &[], BUDGET);
        let d = t.distance(a, b, keep_off);
        assert_eq!(ee.f0().to_bits(), d.0.to_bits(), "{what}: the distance ({d:?})");
        check_paths(&ee, &t, &format!("{what}: after the distance"));
        kept_off += usize::from(d == F(-1.0));

        put_t_spot(&mut ee, T_A, a);
        put_t_spot(&mut ee, T_B, b);
        let fill = F(p.unit());
        put_f(&mut ee, T_D, fill);
        ee.set_u32(T_ROUTE, 0);
        let with_d = p.below(4) != 0;
        let d_arg = if with_d { T_D } else { 0 };
        let args = [u64::from(T_TRACK), u64::from(T_A), u64::from(T_B), u64::from(T_ROUTE), u64::from(d_arg), u64::from(keep_off)];
        ee.call(route_fn, &args, &[], BUDGET);
        let mut route = Vec::new();
        let mut d = fill;
        let got = t.route(a, b, Some(&mut route), if with_d { Some(&mut d) } else { None }, keep_off);
        assert_eq!(ee.v0() as i32, got, "{what}: the route's answer");
        assert_eq!(f32_at(&ee, T_D).0.to_bits(), d.0.to_bits(), "{what}: the route's length");
        if got == 1 {
            assert_eq!(t_route_at(&ee, T_ROUTE), route, "{what}: the route");
        } else {
            assert!(route.is_empty() && ee.u32_at(T_ROUTE) == 0, "{what}: a failed route keeps a list");
        }
        check_paths(&ee, &t, &format!("{what}: after the route"));
        routed += usize::from(got == 1 && !route.is_empty());
        failed += usize::from(got == 0);

        let dist = match p.below(6) {
            0 => F(0.0),
            1 => F(-p.unit()),
            _ => F(p.range(0.0, 400.0)),
        };
        let fill = Spot { path: 0, piece: 0, along: F(p.unit()) };
        put_t_spot(&mut ee, T_OUT, fill);
        ee.call(walk_fn, &[u64::from(T_TRACK), u64::from(T_A), u64::from(T_OUT), u64::from(T_ROUTE), 0], &[dist.0], BUDGET);
        let mut out = fill;
        let got = t.route_walk(dist, a, &route, &mut out);
        assert_eq!(ee.v0() as u32, u32::from(got), "{what}: the walk's answer ({dist:?} along {route:?})");
        assert_eq!(t_bits(t_spot_at(&ee, T_OUT)), t_bits(out), "{what}: the walk ({dist:?} along {route:?})");
        walked += usize::from(got && dist > F::ZERO);

        ee.call(free_fn, &[u64::from(T_TRACK), u64::from(T_ROUTE), 0], &[], BUDGET);
        t.free_route(&mut route);
        check_paths(&ee, &t, &format!("{what}: after the free"));

        // The reach test, on the spots as they were.
        let want = a.path == b.path && (if a.piece == b.piece { fpu::operand(a.along.0.to_bits()) <= fpu::operand(b.along.0.to_bits()) } else { b.piece > a.piece });
        assert_eq!(reaches, u32::from(want), "{what}: the reach test");
    }
    assert!(routed > 3000 && failed > 300 && walked > 3000 && kept_off > 1000, "{routed} {failed} {walked} {kept_off}");
}

#[test]
fn the_ring_test_matches_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let inside_fn = match addr("path_inside") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x7d40_19bb_e352_a06c);
    let mut said = [0usize; 2];
    for case in 0..20_000 {
        let t = t_paths(&mut p);
        put_paths(&mut ee, &mut p, &t);
        let i = p.below(t.paths.len() as u64) as usize;
        let path = &t.paths[i];
        if path.nodes.len() < 2 {
            continue;
        }
        let n = p.below(path.nodes.len() as u64 - 1) as usize;
        let (a, b) = (&path.nodes[n], &path.nodes[n + 1]);
        let c = a.points[0];
        let point = match p.below(5) {
            0 => a.points[p.below(3) as usize],
            1 => b.points[p.below(3) as usize],
            _ => Vf([c.x() + p.range(-40.0, 40.0), p.range(-5.0, 5.0), c.z() + p.range(-40.0, 40.0), 1.0]),
        };
        let point = Vf([p.stray(point.x()), point.y(), p.stray(point.z()), point.w()]);
        put_quad(&mut ee, T_POINT, point);
        ee.call(inside_fn, &[u64::from(T_TRACK), u64::from(T_POINT), u64::from(t_node(i, n)), u64::from(t_node(i, n + 1))], &[], BUDGET);
        let want = race::ring_holds(point, a, b);
        assert_eq!(ee.v0() as u32, u32::from(want), "case {case}: {point:?} in {a:?} {b:?}");
        said[usize::from(want)] += 1;
    }
    assert!(said.iter().all(|&k| k > 1500), "{said:?}");
}

#[test]
fn the_reach_marks_match_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let mark_fn = match addr("track_first_path") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x6a1d_93c0_b257_e48f);
    let (mut done, mut cleared, mut kept) = (0, 0, 0);
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        for path in &mut t.paths {
            if p.below(4) != 0 {
                path.flags |= 0x80;
            }
            if p.below(40) == 0 {
                path.flags |= 0x04;
            }
        }
        put_paths(&mut ee, &mut p, &t);
        let from = p.below(t.paths.len() as u64) as usize;
        let what = format!("case {case}: from {from}, {} paths, {} free", t.paths.len(), t.free_routes);
        let before: Vec<u32> = t.paths.iter().map(|q| q.flags).collect();
        ee.call(mark_fn, &[u64::from(T_TRACK), u64::from(t_rec(from))], &[], BUDGET);
        let got = t.mark_reached(from);
        assert_eq!(ee.v0() as i32, got, "{what}: the answer");
        check_paths(&ee, &t, &what);
        done += usize::from(got == 1);
        for (b, q) in before.iter().zip(&t.paths) {
            cleared += usize::from(b & 0x80 != 0 && q.flags & 0x80 == 0);
            kept += usize::from(b & 0x80 != 0 && q.flags & 0x80 != 0);
        }
    }
    assert!(done > 15_000 && done < 20_000 && cleared > 10_000 && kept > 2000, "{done} {cleared} {kept}");
}

#[test]
fn the_spot_search_matches_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let find_fn = match addr("path_find_spot") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x2b8e_6c05_f1d7_9a43);
    let mut answers = [0usize; 3];
    let mut moved = 0;
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        put_paths(&mut ee, &mut p, &t);
        // A point on or beside a stretch of path, or anywhere.
        let i = p.below(t.paths.len() as u64) as usize;
        let n = p.below(t.paths[i].nodes.len() as u64) as usize;
        let q = t.paths[i].nodes[n].points[0];
        let point = match p.below(6) {
            0 => Vf([p.range(-150.0, 150.0), p.range(-5.0, 5.0), p.range(-150.0, 150.0), 1.0]),
            _ => Vf([q.x() + p.range(-25.0, 25.0), p.range(-5.0, 5.0), q.z() + p.range(-25.0, 25.0), 1.0]),
        };
        let point = Vf([p.stray(point.x()), point.y(), p.stray(point.z()), point.w()]);
        let start = t_spot_on(&mut p, &t);
        let (mut ahead, mut behind) = match p.below(4) {
            0 => (t.ahead, t.behind),
            1 => (F(125.0), F(75.0)),
            _ => (F(p.range(0.0, 120.0)), F(p.range(0.0, 120.0))),
        };
        if p.below(15) == 0 {
            ahead = F(-p.range(0.0, 10.0));
        }
        if p.below(15) == 0 {
            behind = F(-p.range(0.0, 10.0));
        }
        let via: Vec<usize> = if p.below(4) == 0 {
            (0..1 + p.below(3)).map(|_| p.below(t.paths.len() as u64) as usize).collect()
        } else {
            Vec::new()
        };
        for (k, &lp) in via.iter().enumerate() {
            let e = T_VIA + 0x10 * k as u32;
            ee.set_u32(e + 4, t_rec(lp));
            ee.set_u32(e + 0xc, if k + 1 < via.len() { e + 0x10 } else { 0 });
        }
        let what = format!("case {case}: {point:?} from {start:?}, reach {ahead:?} {behind:?}, via {via:?}");
        put_quad(&mut ee, T_POINT, point);
        put_t_spot(&mut ee, T_A, start);
        put_t_spot(&mut ee, T_OUT, Spot { path: 0, piece: 0, along: F(p.unit()) });
        let fill = F(p.unit());
        put_f(&mut ee, T_D, fill);
        let via_arg = if via.is_empty() { 0 } else { T_VIA };
        let args = [T_TRACK, T_POINT, T_A, via_arg, T_OUT, T_D].map(u64::from);
        ee.call(find_fn, &args, &[ahead.0, behind.0], BUDGET * 10);
        let mut out = Spot::default();
        let mut d = fill;
        let got = t.find_spot(ahead, behind, point, start, &via, &mut out, Some(&mut d));
        assert_eq!(ee.v0() as i32, got, "{what}: the answer");
        assert_eq!(t_bits(t_spot_at(&ee, T_OUT)), t_bits(out), "{what}: the spot");
        assert_eq!(f32_at(&ee, T_D).0.to_bits(), d.0.to_bits(), "{what}: the distance");
        check_paths(&ee, &t, &what);
        answers[got as usize] += 1;
        moved += usize::from(got != 0 && out != start);
    }
    assert!(answers.iter().all(|&k| k > 200) && moved > 3000, "{answers:?} {moved}");
}

/// A race card back out of RAM, its spots read back as path and node indices (see `put_t_spot`).
fn race_card_at(ee: &Ee, at: u32, lines: usize) -> RaceCard {
    let mut lap = lap_card_at(ee, at, lines);
    lap.spot = t_spot_at(ee, at + 0x88);
    lap.kept_spot = t_spot_at(ee, at + 0x94);
    RaceCard {
        lap,
        on_path: f32_at(ee, at + 0x18),
        odometer: f32_at(ee, at + 0x20),
        aim: t_spot_at(ee, at + 0xa0),
        look: quad(ee, at + 0x120),
        ahead: quad(ee, at + 0x140),
        to_track: quad(ee, at + 0x180),
    }
}

/// A race card into RAM at `at`, its spots pointing into the track `put_paths` lays out.
fn put_race_card(ee: &mut Ee, at: u32, c: &RaceCard, lines: u32, records: u32) {
    put_lap_card(ee, at, &c.lap, lines, records);
    put_t_spot(ee, at + 0x88, c.lap.spot);
    put_t_spot(ee, at + 0x94, c.lap.kept_spot);
    put_t_spot(ee, at + 0xa0, c.aim);
    for (o, x) in [(0x18, c.on_path), (0x20, c.odometer)] {
        put_f(ee, at + o, x);
    }
    put_quad(ee, at + 0x120, c.look);
    put_quad(ee, at + 0x140, c.ahead);
    put_quad(ee, at + 0x180, c.to_track);
}

fn race_card_words(c: &RaceCard) -> Vec<u64> {
    let f = |x: F| u64::from(x.0.to_bits());
    let mut v = lap_card_words(&c.lap);
    v.extend([f(c.on_path), f(c.odometer), u64::from(c.aim.path), u64::from(c.aim.piece), f(c.aim.along)]);
    for q in [c.look, c.ahead, c.to_track] {
        v.extend(q.to_bits().map(u64::from));
    }
    v
}

/// Who hears a race notice, by listener slot, as `told` gives the lap line's.
fn race_told(n: RaceNotice, owner: bool, chain: bool) -> Vec<(u32, u32)> {
    let fan = |listener_first: bool, o: u32, l: u32| {
        let mut v = Vec::new();
        if listener_first {
            v.push((TO_LISTENER, l));
        }
        if owner {
            v.push((OWNER, o));
        }
        if chain {
            v.push((CHAIN, o));
        }
        if !listener_first {
            v.push((TO_LISTENER, l));
        }
        v
    };
    match n {
        RaceNotice::Line(n) => told(n, owner, chain),
        RaceNotice::Aim { .. } => {
            let mut v = fan(false, 0x20, 0);
            v.pop();
            v
        }
        RaceNotice::OffTrack => fan(true, 0x5c, 0x6c),
        RaceNotice::OffTooLong => fan(true, 0x60, 0x70),
        RaceNotice::OutOfTime => fan(false, 0x64, 0x74),
    }
}

/// A line standing across one of the paths, at a spot on it, facing along it (or any way).
fn line_on(p: &mut Pick, t: &TrackPaths) -> (Gate, Spot) {
    let mut s = t_spot_on(p, t);
    let mut at = Vf::ZERO;
    let mut ahead = Vf::ZERO;
    if !t.spot_point(&mut s, &mut at) {
        let (i, n) = s.at().unwrap();
        at = t.paths[i].nodes[n].points[0];
    }
    let mut g = finish_line(p);
    if p.below(4) != 0 {
        if !t.path_ahead(s, &mut ahead) || p.below(5) == 0 {
            let a = p.range(0.0, 6.3);
            ahead = Vf([a.sin(), 0.0, a.cos(), 0.0]);
        }
        g.centre = Vf([at.x(), at.y() - 1.0, at.z(), 1.0]);
        g.normal = Vf([ahead.x(), 0.0, ahead.z(), 0.0]);
        g.across = Vf([ahead.z(), 0.0, -ahead.x(), 0.0]);
        if p.below(3) != 0 {
            g.width = F(p.range(3.0, 12.0));
        }
    }
    (g, s)
}

#[test]
fn the_race_handler_matches_the_score_managers_per_card() {
    let Some(mut ee) = game_or_skip() else { return };
    let tick = match addr("score_race_tick") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard: Rc<RefCell<Vec<(u32, u32)>>> = Rc::default();
    for who in [OWNER, CHAIN, TO_LISTENER] {
        for slot in (0..0x80).step_by(4) {
            let at = L_HOOKS + who * 0x100 + slot;
            ee.set_u32(L_VT + who * 0x100 + slot, at);
            let heard = heard.clone();
            ee.hooks.insert(at, Box::new(move |_| heard.borrow_mut().push((who, slot))));
        }
    }
    let mut p = Pick(0x7d02_c4e9_15ab_3f68);
    let mut said = [0usize; 5];
    let (mut laps, mut progressed, mut timed_off, mut ticked_off) = (0, 0, 0, 0);
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        let (g, line_spot) = line_on(&mut p, &t);
        let course = Course { line: g, line_spot, lap_length: t.lap_length };
        let lines = p.below(4) as usize;
        let kind = [race_kind::RACE, race_kind::PRACTICE, race_kind::GHOST_RACE, race_kind::FAST_LAP, 0][p.below(5) as usize];
        // The manager's own work after its cards stays out of the way: no countdown, its end-of-race
        // calls made already, a finished race (which returns before the next handlers), no ranking.
        let mut lap_flags = (p.next() as u32 & !0x7e56) | race_flags::FINISHED | 0x2000;
        if p.below(2) == 0 {
            lap_flags |= race_flags::KEEP_LAPS;
        }
        let mut rules = RaceRules {
            lap: LapRules {
                flags: lap_flags,
                kind,
                laps: 1 + p.below(6) as i32,
                ghost_runs: p.below(5) as i32,
                record_limit: p.below(12) as i32,
                free_records: p.below(4) as u32,
                ..LapRules::default()
            },
            below_limit: match p.below(3) {
                0 => F::ZERO,
                1 => F(-p.range(0.0, 3.0)),
                _ => F(p.range(0.5, 6.0)),
            },
            time_limit: if p.below(3) == 0 { F::ZERO } else { F(p.range(10.0, 300.0)) },
            off_limit: F(p.range(0.0, 5.0)),
            aim_reach: F(p.range(0.0, 2.0)),
        };
        let mut lap = lap_card(&mut p, lines);
        for (bit, odds) in [
            (race_card_flags::WAITED_FOR, 2),
            (race_card_flags::WAITED_FOR_TOO, 4),
            (race_card_flags::FAST_LAP_CLOCK, 2),
            (race_card_flags::OUT_OF_TIME, 12),
            (race_card_flags::CLOCK_HELD, 6),
            (race_card_flags::OFF_TRACK_LIMIT, 2),
            (race_card_flags::NOT_WAITED_FOR, 8),
        ] {
            if p.below(odds) == 0 { lap.flags |= bit } else { lap.flags &= !bit }
        }
        lap.flags |= card_flags::TAKING_PART;
        if p.below(8) == 0 {
            lap.flags &= !card_flags::TAKING_PART;
        }
        if p.below(2) == 0 {
            lap.flags &= !card_flags::OUT;
        }
        lap.kept_spot = t_spot_on(&mut p, &t);
        lap.spot = if p.below(3) == 0 { lap.kept_spot } else { t_spot_on(&mut p, &t) };
        let (now, last) = match p.below(4) {
            0 => {
                let (i, n) = lap.kept_spot.at().unwrap();
                let q = t.paths[i].nodes[n].points[0];
                let now = Vf([q.x() + p.range(-15.0, 15.0), q.y() + p.range(-8.0, 3.0), q.z() + p.range(-15.0, 15.0), 1.0]);
                (now, Vf([now.x() + p.range(-3.0, 3.0), now.y(), now.z() + p.range(-3.0, 3.0), p.range(0.0, 1.0)]))
            }
            _ => {
                // A step at the line, the card kept on the path just short of it.
                if p.below(4) != 0 {
                    let along = if p.below(3) == 0 { line_spot.along } else { F(line_spot.along.0 * p.unit()) };
                    lap.kept_spot = Spot { along, ..line_spot };
                    lap.spot = lap.kept_spot;
                }
                if p.below(2) == 0 {
                    step(&mut p, &g)
                } else {
                    // Straight through the middle of the line, a short step.
                    let side = p.range(-0.4, 0.4) * g.width.0.clamp(0.0, 60.0);
                    let up = p.range(0.2, 3.0);
                    let (len, t) = (p.range(0.3, 6.0), p.unit());
                    let (n, a, c) = (g.normal, g.across, g.centre);
                    let on = [0, 1, 2].map(|i| c.0[i] + a.0[i] * side + if i == 1 { up } else { 0.0 });
                    let to = [0, 1, 2].map(|i| on[i] - n.0[i] * len * t);
                    let from = [0, 1, 2].map(|i| on[i] + n.0[i] * len * (1.0 - t));
                    (Vf([from[0], from[1], from[2], 1.0]), Vf([to[0], to[1], to[2], p.unit()]))
                }
            }
        };
        lap.seen = Seen { now, last };
        let lap_no = if kind == race_kind::GHOST_RACE { lap.runs + 1 } else { lap.laps + 1 };
        if lap_no >= rules.lap.record_limit && lap.records.is_empty() {
            lap.records.push(record(&mut p));
        }
        let dt = F([1.0 / 30.0, 1.0 / 60.0, 0.0, 0.1][p.below(4) as usize]);
        lap.gap = time_word(&mut p, 30.0);
        lap.off_track = if p.below(3) == 0 { F(p.range(4.0, 6.0)) } else { time_word(&mut p, 5.0) };
        lap.time_left = match p.below(4) {
            0 => F::ZERO,
            1 => F(p.range(0.0, 0.05)),
            _ => time_word(&mut p, 100.0),
        };
        lap.attack_clock = time_word(&mut p, 90.0);
        let mut card = RaceCard {
            lap,
            on_path: time_word(&mut p, 150.0),
            odometer: time_word(&mut p, 5000.0),
            aim: t_spot_on(&mut p, &t),
            look: p.quad([0.0, 0.0, 0.0]),
            ahead: p.quad([0.0, 0.0, 0.0]),
            to_track: p.quad([0.0, 0.0, 0.0]),
        };
        let owner = p.below(5) != 0;
        let chain = p.below(3) == 0;
        let mut ghost = (owner && p.below(5) != 0).then(|| {
            let mut g = lap_card(&mut p, lines);
            g.spot = t_spot_on(&mut p, &t);
            g.kept_spot = t_spot_on(&mut p, &t);
            g
        });
        let leader = card.lap.race_time;

        put_paths(&mut ee, &mut p, &t);
        ee.set_u32(T_TRACK + 0x5c, L_LINE);
        put_gate(&mut ee, L_LINE, &g);
        put_t_spot(&mut ee, L_LINE + 0x94, line_spot);
        ee.set_u32(L_MGR + 0x30, L_LISTENER);
        ee.set_u32(L_MGR + 0x34, L_CARD);
        ee.set_u32(L_MGR + 0x4c, rules.lap.record_limit as u32);
        ee.set_u32(L_MGR + 0x5c, rules.lap.flags);
        ee.set_u32(L_MGR + 0x60, kind as u32);
        ee.set_u32(L_MGR + 0x68, T_TRACK);
        ee.set_u32(L_MGR + 0x88, lines as u32);
        ee.set_u32(L_MGR + 0x94, rules.lap.laps as u32);
        put_f(&mut ee, L_MGR + 0x98, F::ZERO);
        put_f(&mut ee, L_MGR + 0x9c, F::ZERO);
        put_f(&mut ee, L_MGR + 0xa0, rules.time_limit);
        put_f(&mut ee, L_MGR + 0xb8, rules.below_limit);
        put_f(&mut ee, L_MGR + 0xd4, rules.off_limit);
        put_f(&mut ee, L_MGR + 0xd8, rules.aim_reach);
        ee.set_u32(L_MGR + 0xf0, rules.lap.ghost_runs as u32);
        for k in 0..rules.lap.free_records {
            let next = if k + 1 < rules.lap.free_records { L_POOL + 0x20 * (k + 1) } else { 0 };
            put_record(&mut ee, L_POOL + 0x20 * k, &record(&mut p), next);
        }
        ee.set_u32(L_MGR + 0x58, if rules.lap.free_records == 0 { 0 } else { L_POOL });
        ee.set_u32(L_LISTENER + 4, 0);
        ee.set_u32(L_LISTENER + 0x1c, L_VT + TO_LISTENER * 0x100);
        put_race_card(&mut ee, L_CARD, &card, L_LINES, L_RECORDS);
        ee.set_u32(L_CARD + 4, if owner { L_OWNER } else { 0 });
        ee.set_u32(L_CARD + 0x1c4, if chain { L_CHAIN } else { 0 });
        ee.set_u32(L_CARD + 0x1c8, 0);
        ee.set_u32(L_OWNER + 8, if ghost.is_some() { L_GHOST } else { 0 });
        ee.set_u32(L_OWNER + 0x1c, 0);
        ee.set_u32(L_OWNER + 0x20, L_VT + OWNER * 0x100);
        ee.set_u32(L_CHAIN + 0x1c, 0);
        ee.set_u32(L_CHAIN + 0x20, L_VT + CHAIN * 0x100);
        if let Some(gc) = &ghost {
            put_lap_card(&mut ee, L_GHOST, gc, L_GHOST_LINES, L_RECORDS + 0x400);
            put_t_spot(&mut ee, L_GHOST + 0x88, gc.spot);
            put_t_spot(&mut ee, L_GHOST + 0x94, gc.kept_spot);
        }
        // The route list the handler never sets (see `RaceRules::card`), and low memory's words.
        let stale = ee.stack - 4;
        ee.set_u32(stale, 0);
        ee.set_u32(0x14, 0);
        ee.set_u32(0x54, 0);
        heard.borrow_mut().clear();
        ee.call(tick, &[u64::from(L_MGR)], &[dt.0], BUDGET * 20);
        let answer = ee.v0() as u32;

        let before = card.clone();
        rules.look_for_waited_for([card.lap.flags]);
        let notices = rules.card(dt, &mut card, ghost.as_mut(), &mut t, &course, leader);
        let what = format!("case {case}: kind {kind}, {:?} -> {notices:?}", before.lap.flags);
        assert_eq!(answer, 1, "{what}: the answer");
        let mut want: Vec<(u32, u32)> = if owner { vec![(OWNER, 8)] } else { vec![] };
        want.extend(notices.iter().flat_map(|&n| race_told(n, owner, chain)));
        assert_eq!(*heard.borrow(), want, "{what}: the listeners");
        assert_eq!(race_card_words(&race_card_at(&ee, L_CARD, lines)), race_card_words(&card), "{what}: the card");
        if let Some(gc) = &ghost {
            let mut got = lap_card_at(&ee, L_GHOST, lines);
            got.spot = t_spot_at(&ee, L_GHOST + 0x88);
            got.kept_spot = t_spot_at(&ee, L_GHOST + 0x94);
            assert_eq!(lap_card_words(&got), lap_card_words(gc), "{what}: the ghost's card");
        }
        assert_eq!(ee.u32_at(L_MGR + 0x5c), rules.lap.flags, "{what}: the manager's flags");
        check_paths(&ee, &t, &what);
        let mut free = 0;
        let mut r = ee.u32_at(L_MGR + 0x58);
        while r != 0 && free < 64 {
            free += 1;
            r = ee.u32_at(r + 0x14);
        }
        assert_eq!(free, rules.lap.free_records, "{what}: the free records");
        for n in &notices {
            said[match n {
                RaceNotice::Line(_) => 0,
                RaceNotice::Aim { .. } => 1,
                RaceNotice::OffTrack => 2,
                RaceNotice::OffTooLong => 3,
                RaceNotice::OutOfTime => 4,
            }] += 1;
        }
        laps += usize::from(notices.contains(&RaceNotice::Line(LineNotice::Lap)));
        progressed += usize::from(card.lap.progress.0.to_bits() != before.lap.progress.0.to_bits());
        timed_off += usize::from(card.lap.flags & card_flags::OUT != 0 && before.lap.flags & card_flags::OUT == 0 && card.lap.laps == rules.lap.laps);
        ticked_off += usize::from(card.lap.off_track != before.lap.off_track);
    }
    assert!(
        said.iter().all(|&k| k > 200) && laps > 300 && progressed > 1000 && timed_off > 300 && ticked_off > 3000,
        "{said:?} laps {laps} progressed {progressed} out {timed_off} off-clock {ticked_off}"
    );
}

// --- S294: the race's finish -------------------------------------------------------------------------

/// Free RAM for the finish's manager, its cards (0x200 apart), their owners, chains and ghost cards,
/// the listener, the vtables and the hooks their slots land on.
const F_MGR: u32 = 0x01b0_0000;
const F_TRACK: u32 = 0x01b0_1000;
const F_LISTENER: u32 = 0x01b0_2000;
const F_HEAD: u32 = 0x01b0_3000;
const F_CARDS: u32 = 0x01b1_0000;
const F_GHOSTS: u32 = 0x01b2_0000;
const F_OWNERS: u32 = 0x01b3_0000;
const F_CHAINS: u32 = 0x01b3_1000;
const F_VT: u32 = 0x01b4_0000;
const F_HOOKS: u32 = 0x01b4_1000;

/// A call a listener heard: (who, slot, the object, then the card, the other card and the flag as
/// that slot passes them; 0 where it passes none).
type Heard = (u32, u32, u32, u32, u32, u32);

/// Every listener slot of owners, chains and the manager's listener lands on a hook that writes down
/// the call; the end of a stretch lands on one that ends the call.
fn finish_hooks(ee: &mut Ee, ends: &[u32]) -> Rc<RefCell<Vec<Heard>>> {
    let heard: Rc<RefCell<Vec<Heard>>> = Rc::default();
    for who in [OWNER, CHAIN, TO_LISTENER] {
        for slot in (0..0x80).step_by(4) {
            let at = F_HOOKS + who * 0x100 + slot;
            ee.set_u32(F_VT + who * 0x100 + slot, at);
            let heard = heard.clone();
            ee.hooks.insert(
                at,
                Box::new(move |ee| {
                    let r = |i: usize| ee.r(i) as u32;
                    let (o, l) = (who != TO_LISTENER, who == TO_LISTENER);
                    let args = match slot {
                        0x34 if o => (r(6), r(7), r(8)),
                        0x2c if l => (r(6), r(7), r(8)),
                        0x38 | 0x64 if o => (r(6), 0, 0),
                        0x30 | 0x74 if l => (r(6), 0, 0),
                        _ => (0, 0, 0),
                    };
                    heard.borrow_mut().push((who, slot, r(4), args.0, args.1, args.2));
                }),
            );
        }
    }
    for &end in ends {
        ee.hooks.insert(end, Box::new(|ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));
    }
    heard
}

/// The calls a race notice makes, given the cards' addresses in list order as it stands when told,
/// and which cards have an owner and a chain. `Over` leaves out the listener's one call after them.
fn finish_told(n: FinishNotice, at: &[u32], owner: &[bool], chain: &[bool]) -> Vec<Heard> {
    let other = |k: Option<usize>| k.map_or(0, |k| at[k]);
    let (card, o_slot, l_slot, args) = match n {
        FinishNotice::Over { card } => (card, 0x1c, None, (0, 0, 0)),
        FinishNotice::TimeUp { card } => (card, 0x64, Some(0x74), (at[card], 0, 0)),
        FinishNotice::PlaceLost { card, ahead } => (card, 0x34, Some(0x2c), (at[card], other(ahead), 1)),
        FinishNotice::PlaceGained { card, behind } => (card, 0x34, Some(0x2c), (at[card], other(behind), 0)),
        FinishNotice::Lead { card } => (card, 0x38, Some(0x30), (at[card], 0, 0)),
    };
    let id = ((at[card] - F_CARDS) / 0x200) as u32;
    let mut v = Vec::new();
    if owner[id as usize] {
        v.push((OWNER, o_slot, F_OWNERS + 0x100 * id, args.0, args.1, args.2));
    }
    if chain[id as usize] {
        v.push((CHAIN, o_slot, F_CHAINS + 0x100 * id, args.0, args.1, args.2));
    }
    if let Some(l) = l_slot {
        v.push((TO_LISTENER, l, F_LISTENER, args.0, args.1, args.2));
    }
    v
}

/// The flag bits the finish reads, each set or not at its own odds over the card's other bits.
fn finish_flags(p: &mut Pick, flags: u64) -> u64 {
    let mut f = flags;
    for (bit, odds) in [
        (card_flags::OUT, 4),
        (card_flags::TAKING_PART, 2),
        (card_flags::PLAYER, 3),
        (card_flags::ATTACK_CLOCK, 2),
        (card_flags::TIMED_OUT, 6),
        (card_flags::PLAYER_TOO, 5),
        (card_flags::BEHIND_LINE, 5),
        (card_flags::CLOCK_HELD, 4),
        (card_flags::STANDS_ASIDE, 5),
    ] {
        if p.below(odds) == 0 {
            f |= bit;
        } else {
            f &= !bit;
        }
    }
    f
}

/// A card for the finish: no line words or records; `k` its id in the record word.
fn finish_card(p: &mut Pick, k: u32) -> LapCard {
    let mut c = lap_card(p, 0);
    c.records.clear();
    c.flags = finish_flags(p, c.flags);
    c.record_word = k;
    c.place = p.below(8) as i32;
    if p.below(6) == 0 {
        c.time_left = F(p.range(0.0, 0.05));
    }
    c
}

/// The manager's words the finish reads.
fn finish_rules(p: &mut Pick) -> LapRules {
    let mut flags = p.next() as u32;
    for (bit, odds) in [
        (race_flags::OVER, 3),
        (race_flags::PLAYER_DONE, 2),
        (race_flags::FINISHED, 4),
        (race_flags::OVER_TOLD, 3),
        (race_flags::RANKED, 2),
    ] {
        if p.below(odds) == 0 {
            flags |= bit;
        } else {
            flags &= !bit;
        }
    }
    let wait = match p.below(5) {
        0 => 0.0,
        1 => -1.0,
        _ => 30.0,
    };
    let kind = p.below(6) as i32;
    let laps = if p.below(10) == 0 { p.below(3) as i32 } else { 1 + p.below(6) as i32 };
    let clock = if p.below(3) == 0 { p.range(29.9, 30.1) } else { p.range(0.0, 30.0) };
    let limit = if p.below(3) == 0 { 0.0 } else { p.range(-1.0, 300.0) };
    LapRules {
        flags,
        kind,
        laps,
        finish_wait: F(wait),
        finish_clock: F(p.stray(clock)),
        time_limit: F(p.stray(limit)),
        ..LapRules::default()
    }
}

/// The manager, `cards` linked in list order with their owners (and ghost cards on them), chains and
/// the listener.
fn put_finish(ee: &mut Ee, rules: &LapRules, cards: &[LapCard], ghosts: &[Option<LapCard>], owner: &[bool], chain: &[bool]) {
    ee.set_u32(F_MGR + 0x30, F_LISTENER);
    ee.set_u32(F_MGR + 0x34, if cards.is_empty() { 0 } else { F_CARDS });
    ee.set_u32(F_MGR + 0x5c, rules.flags);
    ee.set_u32(F_MGR + 0x60, rules.kind as u32);
    ee.set_u32(F_MGR + 0x68, F_TRACK);
    ee.set_u32(F_MGR + 0x94, rules.laps as u32);
    put_f(ee, F_MGR + 0x98, rules.finish_clock);
    put_f(ee, F_MGR + 0x9c, rules.finish_wait);
    put_f(ee, F_MGR + 0xa0, rules.time_limit);
    ee.set_u32(F_LISTENER + 4, 0);
    ee.set_u32(F_LISTENER + 0x10, 0);
    ee.set_u32(F_LISTENER + 0x14, 0);
    ee.set_u32(F_LISTENER + 0x1c, F_VT + TO_LISTENER * 0x100);
    for (k, c) in cards.iter().enumerate() {
        let k = k as u32;
        let at = F_CARDS + 0x200 * k;
        put_lap_card(ee, at, c, 0, 0);
        ee.set_u32(at + 0x1c8, if (k as usize) + 1 < cards.len() { at + 0x200 } else { 0 });
        let (own, ch) = (F_OWNERS + 0x100 * k, F_CHAINS + 0x100 * k);
        ee.set_u32(at + 4, if owner[k as usize] { own } else { 0 });
        ee.set_u32(at + 0x1c4, if chain[k as usize] { ch } else { 0 });
        let ghost = ghosts.get(k as usize).and_then(|g| g.as_ref());
        ee.set_u32(own + 8, if ghost.is_some() { F_GHOSTS + 0x200 * k } else { 0 });
        ee.set_u32(own + 0x1c, 0);
        ee.set_u32(own + 0x20, F_VT + OWNER * 0x100);
        ee.set_u32(ch + 0x1c, 0);
        ee.set_u32(ch + 0x20, F_VT + CHAIN * 0x100);
        if let Some(g) = ghost {
            put_lap_card(ee, F_GHOSTS + 0x200 * k, g, 0, 0);
        }
    }
}

#[test]
fn the_over_check_matches_the_race_handlers_opening() {
    let Some(mut ee) = game_or_skip() else { return };
    let [handler, end] = match addrs(["score_race_tick", "score_race_over_end"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    finish_hooks(&mut ee, &[end]);
    let mut p = Pick(0x51d0_77a3_9e2c_4b18);
    let mut over = [0usize; 2];
    for case in 0..10_000 {
        let mut rules = finish_rules(&mut p);
        let n = 1 + p.below(6) as usize;
        let cards: Vec<LapCard> = (0..n as u32).map(|k| finish_card(&mut p, k)).collect();
        put_finish(&mut ee, &rules, &cards, &[], &vec![false; n], &vec![false; n]);
        ee.call(handler, &[u64::from(F_MGR)], &[0.033_333_335], BUDGET);
        rules.check_over(&cards);
        assert_eq!(ee.u32_at(F_MGR + 0x5c), rules.flags, "case {case}: {rules:?}");
        over[usize::from(rules.flags & race_flags::OVER != 0)] += 1;
    }
    assert!(over.iter().all(|&k| k > 2000), "over / not over: {over:?}");
}

#[test]
fn the_clocks_match_the_race_handlers_card_clocks() {
    let Some(mut ee) = game_or_skip() else { return };
    let [start, end] = match addrs(["score_race_clocks", "score_race_clocks_end"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    finish_hooks(&mut ee, &[end]);
    let mut p = Pick(0x0c4e_9b21_d7f3_6a55);
    let (mut made_up, mut ran) = (0, 0);
    for case in 0..20_000 {
        let rules = finish_rules(&mut p);
        let mut card = finish_card(&mut p, 0);
        let now = [p.range(-500.0, 500.0), p.range(-50.0, 50.0), p.range(-500.0, 500.0)];
        card.seen = Seen { now: p.quad(now), last: p.quad([1.0, 2.0, 3.0]) };
        card.laps = if p.below(8) == 0 { rules.laps } else { p.below(7) as i32 };
        // A best lap the clamp to the race time cuts down, now and then.
        if p.below(5) == 0 {
            card.best_lap = F(p.range(100.0, 900.0));
        }
        let head = finish_card(&mut p, 1);
        let alone = p.below(4) == 0;
        put_finish(&mut ee, &rules, &[], &[], &[], &[]);
        put_lap_card(&mut ee, F_CARDS, &card, 0, 0);
        put_lap_card(&mut ee, F_HEAD, &head, 0, 0);
        ee.set_u32(F_MGR + 0x34, if alone { F_CARDS } else { F_HEAD });
        let dt = if p.below(10) == 0 { F(p.stray(0.0)) } else { F(p.range(0.0, 0.05)) };
        ee.set(16, u64::from(F_CARDS));
        ee.set(19, u64::from(F_MGR));
        ee.set_f(24, 0.0);
        ee.set_f(25, dt.0);
        ee.call(start, &[], &[], BUDGET);

        let leader = if alone { card.race_time } else { head.race_time };
        let was = card.flags;
        rules.clocks(dt, &mut card, leader);
        if was & card_flags::OUT == 0 {
            if card.flags & card_flags::OUT != 0 { made_up += 1 } else { ran += 1 }
        }
        assert_eq!(lap_card_words(&lap_card_at(&ee, F_CARDS, 0)), lap_card_words(&card), "case {case}: {rules:?}");
    }
    assert!(made_up > 2000 && ran > 2000, "made-up finishes {made_up}, clocks run {ran}");
}

#[test]
fn the_time_limit_matches_the_race_handlers_card_clock() {
    let Some(mut ee) = game_or_skip() else { return };
    let [start, end] = match addrs(["score_race_time_limit", "score_race_card_end"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard = finish_hooks(&mut ee, &[end]);
    let mut p = Pick(0x7f13_2c86_e0a4_9d3b);
    let mut ups = 0;
    for case in 0..20_000 {
        let rules = finish_rules(&mut p);
        let mut card = finish_card(&mut p, 0);
        // Time about to run out, often.
        if p.below(2) == 0 {
            card.time_left = F(p.range(0.0, 0.06));
        }
        let (owner, chain) = ([p.below(4) != 0], [p.below(3) == 0]);
        put_finish(&mut ee, &rules, &[card.clone()], &[], &owner, &chain);
        let dt = F(p.range(0.0, 0.05));
        ee.set(16, u64::from(F_CARDS));
        ee.set(19, u64::from(F_MGR));
        ee.set_f(24, 0.0);
        ee.set_f(25, dt.0);
        heard.borrow_mut().clear();
        ee.call(start, &[], &[], BUDGET);

        let up = rules.time_limit(dt, &mut card);
        let want: Vec<Heard> =
            if up { finish_told(FinishNotice::TimeUp { card: 0 }, &[F_CARDS], &owner, &chain) } else { vec![] };
        let what = format!("case {case}: {rules:?}");
        assert_eq!(*heard.borrow(), want, "{what}: the listeners");
        assert_eq!(lap_card_words(&lap_card_at(&ee, F_CARDS, 0)), lap_card_words(&card), "{what}: the card");
        ups += usize::from(up);
    }
    assert!(ups > 500, "only {ups} time limits ran out");
}

#[test]
fn the_close_matches_the_race_handlers_finish_ranking_and_gaps() {
    let Some(mut ee) = game_or_skip() else { return };
    let [start, end, rand_state] = match addrs(["score_race_close", "score_race_exit", "rand_state"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard = finish_hooks(&mut ee, &[end]);
    let mut p = Pick(0x93b8_45e1_2a6f_d07c);
    let mut said = [0usize; 5];
    let (mut draws, mut ran_out) = (0, 0);
    for case in 0..20_000 {
        let mut rules = finish_rules(&mut p);
        let n = 1 + p.below(6) as usize;
        let mut cards: Vec<LapCard> = (0..n as u32).map(|k| finish_card(&mut p, k)).collect();
        // Ties: equal progress and equal race times, and places that already stand.
        for k in 1..n {
            if p.below(5) == 0 {
                cards[k].progress = cards[k - 1].progress;
            }
            if p.below(4) == 0 {
                cards[k].race_time = cards[k - 1].race_time;
            }
            if p.below(3) == 0 {
                cards[k].place = k as i32 + 1;
            }
        }
        let owner: Vec<bool> = (0..n).map(|_| p.below(4) != 0).collect();
        let chain: Vec<bool> = (0..n).map(|_| p.below(3) == 0).collect();
        let ghosts: Vec<Option<LapCard>> = (0..n)
            .map(|k| {
                (owner[k] && p.below(4) != 0).then(|| {
                    let mut g = finish_card(&mut p, 100);
                    if p.below(3) == 0 {
                        g.best_lap = cards[k].lap_clock;
                    }
                    g
                })
            })
            .collect();
        let state = p.next();
        put_finish(&mut ee, &rules, &cards, &ghosts, &owner, &chain);
        ee.write(rand_state, 8, u128::from(state));
        let dt = F(p.range(0.0, 0.05));
        ee.set(19, u64::from(F_MGR));
        ee.set_f(24, 0.0);
        ee.set_f(25, dt.0);
        heard.borrow_mut().clear();
        ee.call(start, &[], &[], 1_000_000);

        let before: Vec<u32> = (0..n as u32).map(|k| F_CARDS + 0x200 * k).collect();
        let was_over = rules.flags & race_flags::OVER != 0;
        let mut rng = Rng::from_state(state);
        let ghost_refs: Vec<Option<&LapCard>> = ghosts.iter().map(|g| g.as_ref()).collect();
        let notices = rules.close(dt, &mut cards, &ghost_refs, &mut rng);
        let after: Vec<u32> = cards.iter().map(|c| F_CARDS + 0x200 * c.record_word).collect();
        let mut want: Vec<Heard> = Vec::new();
        for (k, &m) in notices.iter().enumerate() {
            let at = if matches!(m, FinishNotice::Over { .. }) { &before } else { &after };
            want.extend(finish_told(m, at, &owner, &chain));
            if matches!(m, FinishNotice::Over { .. }) && !matches!(notices.get(k + 1), Some(FinishNotice::Over { .. })) {
                want.push((TO_LISTENER, 0x18, F_LISTENER, 0, 0, 0));
            }
            said[match m {
                FinishNotice::Over { .. } => 0,
                FinishNotice::TimeUp { .. } => 1,
                FinishNotice::PlaceLost { .. } => 2,
                FinishNotice::PlaceGained { .. } => 3,
                FinishNotice::Lead { .. } => 4,
            }] += 1;
        }

        let what = format!("case {case}: {rules:?}");
        // The list as the original left it, card by card.
        let mut list = Vec::new();
        let mut at = ee.u32_at(F_MGR + 0x34);
        while at != 0 && list.len() < 8 {
            list.push(at);
            at = ee.u32_at(at + 0x1c8);
        }
        assert_eq!(list, after, "{what}: the list's order");
        for (c, &a) in cards.iter().zip(&after) {
            assert_eq!(lap_card_words(&lap_card_at(&ee, a, 0)), lap_card_words(c), "{what}: card {}", c.record_word);
        }
        assert_eq!(*heard.borrow(), want, "{what}: the listeners ({notices:?})");
        assert_eq!(ee.u32_at(F_MGR + 0x5c), rules.flags, "{what}: the manager's flags");
        assert_eq!(f32_at(&ee, F_MGR + 0x98).0.to_bits(), rules.finish_clock.0.to_bits(), "{what}: the countdown");
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "{what}: the generator");
        draws += usize::from(rng.state() != state);
        ran_out += usize::from(!was_over && rules.flags & race_flags::OVER != 0);
    }
    for (k, n) in said.iter().enumerate() {
        assert!(k == 1 || *n > 500, "notice {k} came up only {n} times: {said:?}");
    }
    assert!(draws > 500 && ran_out > 300, "draws {draws}, countdowns run out {ran_out}");
}

/// C31's live sx01 single race (4 riders, the player at the gate, 3 AI riders round 3 laps), list
/// order per tick: each tick's ranking is the previous tick's list ranked on this tick's progress, and,
/// until a card finishes, every gap is measured on the ranked list. Each tick's list rebuilt from the
/// one before with this tick's card words, through [`LapRules::rank`] and [`race::gaps`], against the
/// captured order, places and gaps, bit for bit.
#[test]
fn the_ranking_and_gaps_match_c31s_live_race() {
    let Ok(kit) = Kit::load() else { return skip("no kit.toml") };
    let rel = "capture/c31/sx01-ai4.csv";
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return skip(&format!("{} is not there: C31's list-ordered sx01 capture", path.display()));
    };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"));
    let (c_frame, c_flags) = (col("frame"), col("mgr_flags"));
    let card_cols: Vec<[usize; 6]> = (0..4)
        .map(|k| ["id", "flags", "progress", "race_time", "behind", "place"].map(|f| col(&format!("card{k}_0.{f}"))))
        .collect();
    struct Row {
        frame: u32,
        flags: u32,
        cards: Vec<(u32, LapCard)>,
    }
    let rows: Vec<Row> = lines
        .map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            let f = |i: usize| F(v[i].parse::<f32>().expect("a float"));
            let cards = card_cols
                .iter()
                .map(|c| {
                    let card = LapCard {
                        flags: v[c[1]].parse().expect("flags"),
                        progress: f(c[2]),
                        race_time: f(c[3]),
                        gap: f(c[4]),
                        place: v[c[5]].parse().expect("place"),
                        ..LapCard::default()
                    };
                    (v[c[0]].parse().expect("id"), card)
                })
                .collect();
            Row { frame: v[c_frame].parse().expect("frame"), flags: v[c_flags].parse().expect("flags"), cards }
        })
        .collect();
    let words = |cards: &[(u32, LapCard)]| -> Vec<(u32, i32, u32)> {
        cards.iter().map(|(id, c)| (*id, c.place, c.gap.0.to_bits())).collect()
    };
    let (mut ranked, mut gapped, mut moved) = (0, 0, 0);
    let mut torn = 0;
    for w in rows.windows(2) {
        let (was, now) = (&w[0], &w[1]);
        if now.frame != was.frame + 1 || now.flags & race_flags::RANKED == 0 || now.cards.iter().all(|(_, c)| is_zero_f(c.race_time)) {
            continue;
        }
        // A torn row (the recorder read a card a tick early or late; recorder-misses-rows-under-load):
        // every card still racing runs the one race clock, so their race times differ only in a tear.
        let split = |r: &Row| {
            let mut t = r.cards.iter().filter(|(_, c)| c.flags & card_flags::OUT == 0).map(|(_, c)| c.race_time.0.to_bits());
            t.next().is_some_and(|first| t.any(|x| x != first))
        };
        if split(was) || split(now) {
            torn += 1;
            continue;
        }
        // The list as it stood, each card with this tick's words but its place and gap from before.
        let mut ids = Vec::new();
        let mut cards: Vec<LapCard> = was
            .cards
            .iter()
            .map(|(id, old)| {
                let (_, c) = now.cards.iter().find(|(i, _)| i == id).expect("the same cards");
                ids.push(*id);
                LapCard { place: old.place, gap: old.gap, record_word: *id, ..c.clone() }
            })
            .collect();
        let _ = LapRules::rank(&mut cards);
        let finished = now.flags & race_flags::FINISHED != 0;
        if !finished {
            race::gaps(&mut cards);
            gapped += 1;
        }
        let mut got: Vec<(u32, LapCard)> = cards.into_iter().map(|c| (c.record_word, c)).collect();
        if finished {
            // No gaps measured: each keeps the captured one.
            for (id, c) in got.iter_mut() {
                c.gap = now.cards.iter().find(|(i, _)| i == id).map(|(_, n)| n.gap).unwrap_or_default();
            }
        }
        moved += usize::from(got.iter().map(|(i, _)| *i).ne(ids.iter().copied()));
        assert_eq!(words(&got), words(&now.cards), "frame {}: (id, place, gap bits) in list order", now.frame);
        ranked += 1;
    }
    assert!(torn < 100, "{torn} torn rows");
    assert!(ranked > 5000 && gapped > 2000 && moved > 0, "ranked {ranked}, gapped {gapped}, re-ordered {moved}");
}

fn is_zero_f(x: F) -> bool {
    x.0 == 0.0
}

// --- S292: the gate race's line and the zones --------------------------------------------------------

/// The gate race's chain of gates (gate k at `G_GATES + 0x100·k`, the next at its `+0x90`), and a zone
/// list laid out the same way.
const G_GATES: u32 = 0x01c0_0000;
const G_ZONES: u32 = 0x01c1_0000;

/// The calls a gate-line notice makes. The same as the lap line's but for the best lap, which the
/// card's chain hears this time and the manager's listener hears twice (the listener loop starts at
/// the listener itself, not at the next one).
fn told_gate(n: LineNotice, owner: bool, chain: bool) -> Vec<(u32, u32)> {
    if n != LineNotice::BestLap {
        return told(n, owner, chain);
    }
    let mut v = Vec::new();
    if owner {
        v.push((OWNER, 0x10));
    }
    if chain {
        v.push((CHAIN, 0x10));
    }
    v.extend([(TO_LISTENER, 0xc), (TO_LISTENER, 0xc)]);
    v
}

/// A chain of gates (or zones) into RAM at `base`, each linked to the next.
fn put_chain(ee: &mut Ee, base: u32, gates: &[Gate]) {
    for (k, g) in gates.iter().enumerate() {
        let at = base + 0x100 * k as u32;
        put_gate(ee, at, g);
        ee.set_u32(at + 0x90, if k + 1 < gates.len() { at + 0x100 } else { 0 });
    }
}

fn put_gate_words(ee: &mut Ee, at: u32, c: &LapCard) {
    ee.set_u32(at + 0x80, c.gate.map_or(0, |k| G_GATES + 0x100 * k as u32));
    ee.set_u32(at + 0x84, c.gates_this_lap as u32);
    ee.set_u32(at + 0x38, c.gates_passed as u32);
}

fn gate_card_at(ee: &Ee, at: u32) -> LapCard {
    let mut c = lap_card_at(ee, at, 0);
    let g = ee.u32_at(at + 0x80);
    c.gate = (g != 0).then(|| ((g - G_GATES) / 0x100) as usize);
    c.gates_this_lap = ee.u32_at(at + 0x84) as i32;
    c.gates_passed = ee.u32_at(at + 0x38) as i32;
    c
}

/// Every listener slot of the three that can hear lands on a hook that writes down who, which slot,
/// and the argument in `$6` (the owner's and the chain's) or `$7` (the listener's).
fn hear_all(ee: &mut Ee) -> Rc<RefCell<Vec<(u32, u32, u32)>>> {
    let heard: Rc<RefCell<Vec<(u32, u32, u32)>>> = Rc::default();
    for who in [OWNER, CHAIN, TO_LISTENER] {
        for slot in (0..0x80).step_by(4) {
            let at = L_HOOKS + who * 0x100 + slot;
            ee.set_u32(L_VT + who * 0x100 + slot, at);
            let heard = heard.clone();
            let reg = if who == TO_LISTENER { 7 } else { 6 };
            ee.hooks.insert(at, Box::new(move |ee| heard.borrow_mut().push((who, slot, ee.r(reg) as u32))));
        }
    }
    heard
}

/// The manager's listener, the card's owner (with the ghost's card, or none) and its chain.
fn put_hearers(ee: &mut Ee, owner: bool, chain: bool, ghost: bool) {
    ee.set_u32(L_MGR + 0x30, L_LISTENER);
    ee.set_u32(L_LISTENER + 4, 0);
    ee.set_u32(L_LISTENER + 0x18, 0);
    ee.set_u32(L_LISTENER + 0x1c, L_VT + TO_LISTENER * 0x100);
    ee.set_u32(L_CARD + 4, if owner { L_OWNER } else { 0 });
    ee.set_u32(L_CARD + 0x1c4, if chain { L_CHAIN } else { 0 });
    ee.set_u32(L_OWNER + 8, if ghost { L_GHOST } else { 0 });
    ee.set_u32(L_OWNER + 0x1c, 0);
    ee.set_u32(L_OWNER + 0x20, L_VT + OWNER * 0x100);
    ee.set_u32(L_CHAIN + 0x1c, 0);
    ee.set_u32(L_CHAIN + 0x20, L_VT + CHAIN * 0x100);
}

#[test]
fn the_gate_line_matches_the_score_cards_gate_test() {
    let Some(mut ee) = game_or_skip() else { return };
    let gate_line = match addr("score_gate_line") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard = hear_all(&mut ee);
    let mut p = Pick(0x51c2_9e04_77ab_3d65);
    let mut said = [0usize; 10];
    let mut wrapped = 0;
    for case in 0..20_000 {
        let n = 1 + p.below(5) as usize;
        let gates: Vec<Gate> = (0..n).map(|_| finish_line(&mut p)).collect();
        let mut rules = LapRules {
            flags: (p.next() as u32 & !0x410) | if p.below(2) == 0 { race_flags::KEEP_LAPS } else { 0 }
                | if p.below(5) == 0 { race_flags::FINISHED } else { 0 },
            kind: if p.below(3) == 0 { race_kind::GHOST_RACE } else { p.below(6) as i32 },
            laps: 1 + p.below(6) as i32,
            ghost_runs: p.below(5) as i32,
            record_limit: p.below(12) as i32,
            free_records: p.below(4) as u32,
            ..LapRules::default()
        };
        let mut card = lap_card(&mut p, 0);
        // Most cards wait at the last gate, where a lap is counted.
        let at = if p.below(2) == 0 { n - 1 } else { p.below(n as u64) as usize };
        card.gate = (p.below(12) != 0).then_some(at);
        card.gates_this_lap = p.below(5) as i32;
        card.gates_passed = p.below(20) as i32;
        if p.below(8) == 0 {
            card.best_lap = F(fpu::sub(card.race_time.0, card.lap_start.0));
        }
        let (from, to) = step(&mut p, &gates[at]);
        card.seen = Seen { now: from, last: to };
        let lap = if rules.kind == race_kind::GHOST_RACE { card.runs + 1 } else { card.laps + 1 };
        if lap >= rules.record_limit && card.records.is_empty() {
            card.records.push(record(&mut p));
        }
        let owner = p.below(5) != 0;
        let chain = p.below(3) == 0;
        let mut ghost = (owner && p.below(5) != 0).then(|| {
            let mut g = lap_card(&mut p, 0);
            g.gate = (p.below(3) != 0).then(|| p.below(n as u64) as usize);
            g.gates_this_lap = p.below(5) as i32;
            g.gates_passed = p.below(20) as i32;
            g
        });
        if let Some(gc) = ghost.as_mut() {
            match p.below(6) {
                0 => gc.best_lap = card.best_lap,
                1 => gc.best_lap = F(fpu::sub(card.race_time.0, card.lap_start.0)),
                _ => {}
            }
        }

        put_chain(&mut ee, G_GATES, &gates);
        ee.set_u32(L_MGR + 0x4c, rules.record_limit as u32);
        ee.set_u32(L_MGR + 0x5c, rules.flags);
        ee.set_u32(L_MGR + 0x60, rules.kind as u32);
        ee.set_u32(L_MGR + 0x70, G_GATES);
        ee.set_u32(L_MGR + 0x94, rules.laps as u32);
        ee.set_u32(L_MGR + 0xf0, rules.ghost_runs as u32);
        for k in 0..rules.free_records {
            let next = if k + 1 < rules.free_records { L_POOL + 0x20 * (k + 1) } else { 0 };
            put_record(&mut ee, L_POOL + 0x20 * k, &record(&mut p), next);
        }
        ee.set_u32(L_MGR + 0x58, if rules.free_records == 0 { 0 } else { L_POOL });
        put_lap_card(&mut ee, L_CARD, &card, L_LINES, L_RECORDS);
        put_gate_words(&mut ee, L_CARD, &card);
        put_hearers(&mut ee, owner, chain, ghost.is_some());
        if let Some(gc) = &ghost {
            put_lap_card(&mut ee, L_GHOST, gc, L_GHOST_LINES, L_RECORDS + 0x400);
            put_gate_words(&mut ee, L_GHOST, gc);
        }
        heard.borrow_mut().clear();
        ee.call(gate_line, &[u64::from(L_MGR), u64::from(L_CARD)], &[], BUDGET);
        let answer = ee.v0() as u32;

        let (got, notices) = rules.gate_line(&mut card, ghost.as_mut(), &gates);
        let what = format!("case {case}: {rules:?} at {at} of {n}");
        assert_eq!(answer, u32::from(got), "{what}: the answer");
        let want: Vec<(u32, u32)> = notices.iter().flat_map(|&k| told_gate(k, owner, chain)).collect();
        let got_heard: Vec<(u32, u32)> = heard.borrow().iter().map(|&(w, s, _)| (w, s)).collect();
        assert_eq!(got_heard, want, "{what}: the listeners ({notices:?})");
        assert_eq!(lap_card_words(&gate_card_at(&ee, L_CARD)), lap_card_words(&card), "{what}: the card");
        if let Some(gc) = &ghost {
            assert_eq!(lap_card_words(&gate_card_at(&ee, L_GHOST)), lap_card_words(gc), "{what}: the ghost's card");
        }
        assert_eq!(ee.u32_at(L_MGR + 0x5c), rules.flags, "{what}: the manager's flags");
        let mut free = 0;
        let mut r = ee.u32_at(L_MGR + 0x58);
        while r != 0 && free < 64 {
            free += 1;
            r = ee.u32_at(r + 0x14);
        }
        assert_eq!(free, rules.free_records, "{what}: the free records");
        for k in &notices {
            said[*k as usize] += 1;
        }
        wrapped += usize::from(notices.contains(&LineNotice::Lap));
    }
    // Every notice the gate line makes comes up often enough to mean something (it never says
    // Started).
    for (k, c) in said.iter().enumerate() {
        if k != LineNotice::Started as usize {
            assert!(*c > 100, "notice {k} came up only {c} times in 20,000: {said:?}");
        }
    }
    assert!(wrapped > 1000, "only {wrapped} laps counted: {said:?}");
}

/// What a zone notice sounds like: through is the owner's and chain's `+0x2c` and the listener's
/// `+0x24`, beside `+0x30` and `+0x28`; each with the zone.
fn told_zone(k: usize, c: Crossing, owner: bool, chain: bool) -> Vec<(u32, u32, u32)> {
    let zone = G_ZONES + 0x100 * k as u32;
    let (o, l) = if c == Crossing::Through { (0x2c, 0x24) } else { (0x30, 0x28) };
    let mut v = Vec::new();
    if owner {
        v.push((OWNER, o, zone));
    }
    if chain {
        v.push((CHAIN, o, zone));
    }
    v.push((TO_LISTENER, l, zone));
    v
}

/// Zones about a step: most of them gates it is aimed at, some shut.
fn zones_about(p: &mut Pick) -> (Vec<Gate>, Seen) {
    let n = 1 + p.below(5) as usize;
    let mut zones: Vec<Gate> = (0..n).map(|_| finish_line(p)).collect();
    let aim = p.below(n as u64) as usize;
    let (from, to) = step(p, &zones[aim]);
    // Others stand where the same step may pass them too.
    for (k, z) in zones.iter_mut().enumerate() {
        if k != aim && p.below(2) == 0 {
            z.centre = zones_centre(p, from, to);
        }
        if p.below(6) == 0 {
            z.flags &= !flags::CLOSED;
        }
    }
    (zones, Seen { now: from, last: to })
}

fn zones_centre(p: &mut Pick, from: Vf, to: Vf) -> Vf {
    let t = p.range(0.0, 1.0);
    let c = [0, 1, 2].map(|i| {
        let (a, b) = (from.0[i], to.0[i]);
        if a.is_finite() && b.is_finite() && a.abs() < 1e6 && b.abs() < 1e6 { b + (a - b) * t } else { 0.0 }
    });
    Vf([c[0], c[1] - p.range(0.0, 3.0), c[2], 0.0])
}

/// The zone list as the machine-race handler asks it: the stretch of its card loop from the zone bit's
/// test to the card's distance to its gate, entered with the manager and the card in its registers.
#[test]
fn the_zone_line_matches_the_machine_races_zone_list() {
    let Some(mut ee) = game_or_skip() else { return };
    let [start, end] = match addrs(["score_machine_zones", "score_machine_zones_end"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard = hear_all(&mut ee);
    ee.hooks.insert(end, Box::new(|ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));
    let mut p = Pick(0x0be7_4c19_d2a3_5f80);
    let (mut through, mut beside) = (0, 0);
    for case in 0..20_000 {
        let (zones, seen) = zones_about(&mut p);
        let on = p.below(8) != 0;
        let empty = p.below(20) == 0;
        let owner = p.below(5) != 0;
        let chain = p.below(3) == 0;
        put_chain(&mut ee, G_ZONES, &zones);
        ee.set_u32(L_MGR + 0x5c, (p.next() as u32 & !race_flags::ZONES) | if on { race_flags::ZONES } else { 0 });
        ee.set_u32(L_MGR + 0x78, if empty { 0 } else { G_ZONES });
        put_quad(&mut ee, L_CARD + 0x60, seen.now);
        put_quad(&mut ee, L_CARD + 0x70, seen.last);
        put_hearers(&mut ee, owner, chain, false);
        heard.borrow_mut().clear();
        ee.set(16, u64::from(L_CARD));
        ee.set(18, u64::from(L_MGR));
        ee.call(start, &[], &[], BUDGET);

        let crossed = if on && !empty { race::zone_crossings(seen, &zones) } else { Vec::new() };
        let want: Vec<(u32, u32, u32)> = crossed.iter().flat_map(|&(k, c)| told_zone(k, c, owner, chain)).collect();
        assert_eq!(*heard.borrow(), want, "case {case}: {zones:?} {seen:?}");
        through += crossed.iter().filter(|c| c.1 == Crossing::Through).count();
        beside += crossed.iter().filter(|c| c.1 == Crossing::Beside).count();
    }
    assert!(through > 2000 && beside > 2000, "through {through} beside {beside}");
}

/// The stunt-mode handler asks the same list, but only after the card's place has been copied into the
/// tick before's: its zones are never crossed, though the step it was handed crosses them.
#[test]
fn the_stunt_modes_zone_list_never_sees_a_crossing() {
    let Some(mut ee) = game_or_skip() else { return };
    let mode3 = match addr("score_mode3") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard = hear_all(&mut ee);
    let mut p = Pick(0x9d03_2c6e_b118_4a57);
    let mut would = 0;
    for case in 0..5_000 {
        let (zones, seen) = zones_about(&mut p);
        let owner = p.below(5) != 0;
        let chain = p.below(3) == 0;
        put_chain(&mut ee, G_ZONES, &zones);
        // Zones on; no jump records, no ranking by total; the end already called.
        ee.set_u32(L_MGR + 0x5c, (p.next() as u32 & !0x4000) | race_flags::ZONES | 0x2000);
        ee.set_u32(L_MGR + 0x34, L_CARD);
        ee.set_u32(L_MGR + 0x78, G_ZONES);
        put_f(&mut ee, L_MGR + 0xa0, F(if p.below(2) == 0 { 0.0 } else { p.range(1.0, 120.0) }));
        ee.set_u32(L_CARD + 8, card_flags::TAKING_PART as u32);
        ee.set_u32(L_CARD + 0xc, 0);
        put_f(&mut ee, L_CARD + 0xc4, F(p.range(-1.0, 100.0)));
        ee.set_u32(L_CARD + 0x1c8, 0);
        put_quad(&mut ee, L_CARD + 0x60, seen.now);
        put_quad(&mut ee, L_CARD + 0x70, seen.last);
        put_hearers(&mut ee, owner, chain, false);
        heard.borrow_mut().clear();
        ee.call(mode3, &[u64::from(L_MGR)], &[1.0 / 30.0], BUDGET);

        let zone_slot = |&(w, s, _): &(u32, u32, u32)| {
            if w == TO_LISTENER { s == 0x24 || s == 0x28 } else { s == 0x2c || s == 0x30 }
        };
        let heard_zones: Vec<_> = heard.borrow().iter().copied().filter(zone_slot).collect();
        let crossed = race::zone_crossings(Seen { now: seen.now, last: seen.now }, &zones);
        let want: Vec<(u32, u32, u32)> = crossed.iter().flat_map(|&(k, c)| told_zone(k, c, owner, chain)).collect();
        assert_eq!(heard_zones, want, "case {case}: {zones:?} {seen:?}");
        assert!(want.is_empty(), "case {case}: a stunt-mode zone was crossed");
        assert_eq!(quad(&ee, L_CARD + 0x70).to_bits(), seen.now.to_bits(), "case {case}: the tick before's place");
        would += usize::from(!race::zone_crossings(seen, &zones).is_empty());
    }
    assert!(would > 1000, "only {would} steps would have crossed a zone");
}
