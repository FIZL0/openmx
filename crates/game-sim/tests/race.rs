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
//! 22. **The start's timeline** (S13f): the supercross activity's listener on the countdown, 20,000
//!     generated clocks (zero, minus zero, denormals, each window's edges and a ulp either side, the
//!     call window's edges 1.5 s on, stray words) over windows as a reset leaves them or anywhere,
//!     zero to five actors (other classes, items with and without a body), every viewport mode, the
//!     intro seen or not, ended or not, against [`Supercross::start_timeline`] on the activity, the
//!     countdown's elapsed and accumulator, every item's body and words, the call played, the intro's
//!     fade and the overlays shown.
//! 23. **The grid with AllAI set**: the grid slot on 2,000 starts with the knob at 1, 2, −1 and the
//!     sign bit, against [`Supercross::shuffle_lanes`] with `all_ai`: no draw, the lanes in order, the
//!     race overlays' one call, no vehicle or rider reset, the same spots and places.
//! 24. **The start hold on the sx01 image**: the image's one interface item is the rider's own, its
//!     body the bike chassis, held and worded as the timeline's zero left it.
//! 25. **The line lists** (S294a): the race handler's stretch from the checkpoint clear through both
//!     line lists, 20,000 generated tracks, lists of gates (dead ones, gates on and off the paths),
//!     passed words, reaches and steps at a gate, against [`RaceRules::clear_checkpoint`] and
//!     [`RaceRules::line_lists`], compared on every listener call in order with the gate and card
//!     passed, and every path's words and the pools the path distances use.
//! 26. **C31's live sx01 laps** (S13c): both captured sx01 single races, every rider's card every tick
//!     seeded from the tick before and stepped through [`RaceRules::card`] on sx01's paths out of C31's
//!     RAM image, against the captured card: 16 laps, 9 best laps, 6 finishes, the human's 4,192
//!     off-track ticks and 19 off-track limits, bit for bit.
//! 27. **The machine race's handler** (S292a): the score manager's mode-1 handler on 20,000 lists of
//!     one to five cards over chains of one to four gates and zone lists, every listener call, word,
//!     flag, countdown, free record and the generator against `LapRules::machine_tick`.
//! 28. **The gate ranking**: its ranking alone on 20,000 lists with ties, against `LapRules::gate_rank`.
//! 29. **The Hall of Fame test** (S13d): the front end's "does this time earn a place" on 20,000
//!     tables, tracks, classes and times (whole seconds and a ulp either side, zero, negatives,
//!     integer entries), against [`race::HallOfFame::beats`].
//! 30. **A card's finish, answered**: the supercross activity's answer to the lap line's finish on
//!     20,000 cards, events, seats, tracks and flashes, against [`Supercross::race_over`] on every
//!     call (the fireworks, the win counted, each Hall of Fame entry) and the podium flashes' words.
//! 31. **The race's end, answered**: the activity's answer to the race's Over notice on 20,000 races,
//!     against [`Supercross::race_end`] on every call, the tip popup's words and the outro scene.
//! 32. **S13d's live sx01 finish**: a one-lap race the human is ended in by a poke; the winner's finish
//!     at the line through `race_over` (the flashes bit for bit), the over tick through `check_over`,
//!     `clocks` and `close` (the made-up finish, the tie draw found on the generator, the ranking)
//!     and `race_end` (the outro scene), against the captured cards, manager and activity.
//! 33. **The gate's spot** (S295): the track object's gate spot on 20,000 generated courses and gates
//!     (across a stretch, short of it, far off, no width, stray words, pools run dry), against
//!     [`TrackPaths::gate_spot`], on its answer, the spot, every path's flags and the route pool.
//! 34. **The load's end**: the track object's loader, nothing after the reader hooked, on 10,000
//!     generated loadable race files, against [`TrackPaths::load`] — every path's flags and scratch
//!     words, the three pools, the three gates' spots and the lap length.
//! 35. **sx01 loaded**: sx01's race file through [`TrackPaths::load`] against S196's image: the lap
//!     length and the third gate's spot, bit for bit.

use game_sim::chassis::{Chassis, Deriv};
use game_sim::race::{
    self, CheckPart, Checkpoint, Course, Crossing, Gate, GridStart, LANES, LapCard, LapRecord, LapRules, LineLists, LineNotice, Link, ListGate,
    MachineNotice, PathNode, Prop, Race, RaceCard, FinishNotice, RaceNotice, RacePath, RaceRules, Refused, Seen, Spot, StartItem, StartStep, Axes, Supercross, TrackFile, TrackPath, TrackPaths, card_flags,
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

/// S295: sx01's race file, loaded by [`TrackPaths::load`], against S196's track object: the lap length
/// and the third gate's spot (the supercross loader moves the grid gate and the line and spots them
/// again, so theirs are not the load's).
#[test]
fn sx01s_loaded_track_has_the_images_lap_length_and_third_gate_spot() {
    let Some(file) = sx01_race_file() else { return };
    let Some(ee) = sx01_image() else { return };
    let manager = match addr("activity_manager") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    let track = ee.u32_at(act + 0xb8);
    let t = TrackFile::parse(&file).expect("sx01's race file reads");
    let (paths, spots) = TrackPaths::load(&t).expect("sx01's track loads");
    let slots: Vec<u32> = (0..ee.u32_at(track + 0x40)).map(|k| ee.u32_at(ee.u32_at(track + 0x50) + 4 * k)).collect();
    assert_eq!(f32_at(&ee, track + 0x54).0.to_bits(), paths.lap_length.0.to_bits(), "the lap length ({:?})", paths.lap_length);
    let third = spots[2].expect("a third gate");
    assert_eq!(t_bits(loaded_spot_at(&ee, &slots, ee.u32_at(track + 0x60) + 0x94)), t_bits(third), "the third gate's spot");
    eprintln!("sx01: lap {:?}, spots {spots:?}", paths.lap_length);
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
        ee.set_u32(r + 0xc, path.word_0c);
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
    ee.set_u32(T_TRACK + 0xac, t.pick);
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
        paths.push(RacePath { flags, length: F(p.stray(length.0)), word_0c: 0, nodes, links: Vec::new(), dist, best });
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
        pick: 3,
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

/// S282b: the path spot at a point (`ai::follow::spot`), on the same generated courses: points on,
/// beside and far off the paths, holding-only or not, with and without a distance out, and pools that
/// run dry.
#[test]
fn the_path_spot_at_a_point_matches_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let spot_fn = match addr("path_spot") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x5282_b5b0_7a11_d00d);
    let mut seen = [0usize; 6];
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        put_paths(&mut ee, &mut p, &t);
        ee.set_u32(T_TRACK + 0x4c, t_rec(0));
        let i = p.below(t.paths.len() as u64) as usize;
        let n = p.below(t.paths[i].nodes.len() as u64) as usize;
        let q = t.paths[i].nodes[n].points[0];
        let point = match p.below(8) {
            0 => Vf([p.range(-150.0, 150.0), p.range(-5.0, 5.0), p.range(-150.0, 150.0), 1.0]),
            1 => q,
            _ => Vf([q.x() + p.range(-25.0, 25.0), p.range(-5.0, 5.0), q.z() + p.range(-25.0, 25.0), 1.0]),
        };
        let point = Vf([p.stray(point.x()), point.y(), p.stray(point.z()), point.w()]);
        let holding = p.below(3) == 0;
        let with_distance = p.below(2) == 0;
        let fill = Spot { path: 0, piece: 0, along: F(p.unit()) };
        let d_fill = F(p.unit());
        put_quad(&mut ee, T_POINT, point);
        put_t_spot(&mut ee, T_OUT, fill);
        put_f(&mut ee, T_D, d_fill);
        let what = format!("case {case}: {point:?}, holding {holding}, distance {with_distance}");
        let args = [
            u64::from(T_TRACK),
            u64::from(T_POINT),
            u64::from(T_OUT),
            if with_distance { u64::from(T_D) } else { 0 },
            p.next() & 1,
            u64::from(holding),
        ];
        ee.call(spot_fn, &args, &[], BUDGET * 10);
        let mut out = fill;
        let mut d = d_fill;
        let got = game_sim::ai::follow::spot(&mut t, point, holding, &mut out, with_distance.then_some(&mut d));
        assert_eq!(ee.v0() as u32, u32::from(got), "{what}: the answer");
        assert_eq!(t_bits(t_spot_at(&ee, T_OUT)), t_bits(out), "{what}: the spot");
        assert_eq!(f32_at(&ee, T_D).0.to_bits(), d.0.to_bits(), "{what}: the distance");
        check_paths(&ee, &t, &what);
        seen[usize::from(got)] += 1;
        seen[2] += usize::from(got && out == fill);
        seen[3] += usize::from(got && holding && out != fill);
        seen[4] += usize::from(got && out.path > 1);
        seen[5] += usize::from(got && with_distance && d == F(0.0));
    }
    eprintln!("path spot: refused {}, found {}, none {}, holding {}, off path 0 {}, on the line {}", seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]);
    assert!(seen[0] > 300 && seen[1] > 15_000 && seen[2] > 200 && seen[3] > 2000 && seen[4] > 3000 && seen[5] > 50, "{seen:?}");
}

const T_GATE: u32 = 0x01b6_0000;

/// A gate standing on a generated course: mostly across one of its stretches (its width spanning the
/// stretch, or falling short of it), now and then far off any path, with no width, or along the stretch
/// rather than across it; stray words now and then.
fn gate_on(p: &mut Pick, t: &TrackPaths) -> (Vf, Vf, F) {
    let i = p.below(t.paths.len() as u64) as usize;
    let nodes = &t.paths[i].nodes;
    let k = p.below(nodes.len() as u64) as usize;
    let a = nodes[k].points[0];
    let b = nodes.get(k + 1).map_or(a, |n| n.points[0]);
    let f = p.unit();
    let mut centre = [a.x() + (b.x() - a.x()) * f, p.range(-3.0, 3.0), a.z() + (b.z() - a.z()) * f];
    let (dx, dz) = (b.x() - a.x(), b.z() - a.z());
    let heading = if dx == 0.0 && dz == 0.0 { p.range(0.0, 6.3) } else { dz.atan2(dx) };
    let turn = match p.below(6) {
        0 => 0.0,
        1 => p.range(-1.5, 1.5),
        _ => std::f32::consts::FRAC_PI_2 + p.range(-0.5, 0.5),
    };
    let (s, c) = (heading + turn).sin_cos();
    let mut width = match p.below(8) {
        0 => 0.0,
        1 => p.range(0.0, 2.0),
        _ => p.range(5.0, 60.0),
    };
    if p.below(8) == 0 {
        centre[0] += p.range(-200.0, 200.0);
        centre[2] += p.range(-200.0, 200.0);
    }
    if p.below(20) == 0 {
        width = -width;
    }
    let centre = Vf([p.stray(centre[0]), centre[1], p.stray(centre[2]), 1.0]);
    let across = Vf([p.stray(c), p.range(-0.1, 0.1), p.stray(s), 1.0]);
    (centre, across, F(p.stray(width)))
}

/// S295: the gate's spot (`TrackPaths::gate_spot`) against the track object's, on the generated
/// courses: gates across stretches, short of them, far off, with no width; pools that run dry.
#[test]
fn the_gate_spot_matches_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let gate_fn = match addr("track_gate_spot") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x5295_6a7e_0dd5_1c3b);
    let (mut crossed, mut nearest, mut refused, mut later_path) = (0, 0, 0, 0);
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        put_paths(&mut ee, &mut p, &t);
        ee.set_u32(T_TRACK + 0x4c, t_rec(0));
        let (centre, across, width) = gate_on(&mut p, &t);
        for o in (0..0xa0).step_by(4) {
            ee.set_u32(T_GATE + o, p.next() as u32);
        }
        put_quad(&mut ee, T_GATE + 0x40, centre);
        put_quad(&mut ee, T_GATE + 0x60, across);
        put_f(&mut ee, T_GATE + 0x70, width);
        let fill = Spot { path: 0, piece: 0, along: F(p.unit()) };
        put_t_spot(&mut ee, T_GATE + 0x94, fill);
        let gate = Gate { centre, across, width, ..Gate::new() };
        let what = format!("case {case}: {gate:?}");

        ee.call(gate_fn, &[u64::from(T_TRACK), u64::from(T_GATE)], &[], BUDGET * 10);
        let mut out = fill;
        let mut probe = t.clone();
        let got = t.gate_spot(&gate, &mut out);
        assert_eq!(ee.v0() as u32, u32::from(got), "{what}: the answer");
        assert_eq!(t_bits(t_spot_at(&ee, T_GATE + 0x94)), t_bits(out), "{what}: the spot");
        check_paths(&ee, &t, &what);
        // Which way it answered: the path spot at the centre gives the same spot only by chance.
        let mut s = fill;
        let near = game_sim::ai::follow::spot(&mut probe, centre, false, &mut s, None);
        if !got {
            refused += 1;
        } else if near && s == out {
            nearest += 1;
        } else {
            crossed += 1;
            later_path += usize::from(out.path > 1);
        }
    }
    eprintln!("gate spot: crossed {crossed} (off path 0 {later_path}), nearest {nearest}, refused {refused}");
    assert!(crossed > 6000 && nearest > 2000 && refused > 300 && later_path > 2000, "{crossed} {nearest} {refused} {later_path}");
}

/// A race file the load's walks can run on: [`t_paths`]' course written out record by record (each
/// path's flags, nodes and links in range), now and then a scale and offsets first, and the three gates
/// each stood on the course as [`gate_on`] stands one, or left out. Now and then a path has so many
/// links that the route pool runs dry on the walks.
fn loadable_file(p: &mut Pick) -> Vec<u8> {
    let course = t_paths(p);
    let count = course.paths.len();
    let mut t = Tdf(Vec::new());
    if p.below(4) == 0 {
        t.tag(4).f([0.5, 2.5, 3.0][p.below(3) as usize]);
    }
    if p.below(4) == 0 {
        t.tag(2).f(p.range(-50.0, 50.0)).tag(3).f(p.range(-50.0, 50.0));
    }
    t.tag(1).i(count as i32);
    let dense = count > 1 && p.below(10) == 0;
    for (i, path) in course.paths.iter().enumerate() {
        t.tag(5).i(i as i32);
        let mut flags = p.next() as u32 & 0x2a & !0x04;
        if p.below(6) == 0 {
            flags |= 0x100;
        }
        if p.below(40) == 0 {
            flags |= 0x04;
        }
        t.tag(6).w(flags);
        for n in &path.nodes {
            t.tag(8);
            for (tag, pt) in [9u8, 0x0a, 0x0b].into_iter().zip(n.points) {
                t.tag(tag).f(pt.x()).f(pt.y()).f(pt.z());
            }
        }
        let mut links = path.links.clone();
        if dense {
            for _ in 0..p.below(12 * count as u64) {
                links.push(p.below(count as u64) as usize);
            }
        }
        t.tag(0x0d).i(links.len() as i32);
        for (c, &l) in links.iter().enumerate() {
            t.tag(0x0e).i(c as i32).i(l as i32);
        }
    }
    for tag in [0x0f, 0x10, 0x19] {
        if p.below(5) == 0 {
            continue;
        }
        let (centre, across, width) = gate_on(p, &course);
        let normal = p.unit_vector();
        t.tag(tag).f(centre.x()).f(centre.y()).f(centre.z()).f(normal[0]).f(normal[1]).f(normal[2]);
        t.f(across.x()).f(across.y()).f(across.z()).f(width.0);
    }
    // A last byte the reader never takes.
    t.tag(0);
    t.0
}

/// A spot in the loaded track object (path and node addresses) as path and node indices, by the
/// path slots at `slots` and each path's node list.
fn loaded_spot_at(ee: &Ee, slots: &[u32], at: u32) -> Spot {
    let (path, node) = (ee.u32_at(at), ee.u32_at(at + 4));
    let along = f32_at(ee, at + 8);
    if path == 0 {
        return Spot { path: 0, piece: node, along };
    }
    let Some(i) = slots.iter().position(|&s| s == path && s != 0) else {
        return Spot { path: path | 0x8000_0000, piece: node, along };
    };
    let (mut n, mut k) = (ee.u32_at(path + 0x10), 0u32);
    while n != 0 && n != node && k < 10_000 {
        n = ee.u32_at(n + 0x54);
        k += 1;
    }
    let piece = if node == 0 { 0 } else if n == node { k + 1 } else { node | 0x8000_0000 };
    Spot { path: i as u32 + 1, piece, along }
}

/// S295: the end of the race file's load — the walk from path 0, each gate's spot and the lap's length
/// — on 10,000 generated loadable files, the track object's own loader with nothing after the reader
/// hooked, against [`TrackPaths::load`]: every path's flags and scratch words, the three pools, the
/// gates' spots and the lap length.
#[test]
fn the_loaded_track_object_matches_trackpaths_load() {
    let Some(mut ee) = game_or_skip() else { return };
    let names = ["track_ctor", "track_file_load", "fs_open", "fs_read", "fs_eof", "fs_close", "mem_calloc", "operator_new", "node_add_child", "free"];
    let [ctor, load, open, read, eof, close, calloc, new, add_child, free] = match addrs(names) {
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
            ee.load(at, &vec![0xa5; size as usize]);
            ee.set(2, u64::from(at));
        }),
    );
    let freed = Rc::new(Cell::new(0));
    let f = freed.clone();
    ee.hooks.insert(free, Box::new(move |_| f.set(f.get() + 1)));
    for at in [close, add_child] {
        ee.hooks.insert(at, Box::new(|_| {}));
    }

    let mut p = Pick(0x5295_10ad_7e3c_9b21);
    let (mut loaded, mut refused, mut crossed_gates, mut laps, mut no_way, mut dry) = (0, 0, 0, 0, 0, 0);
    for case in 0.. {
        if loaded == 10_000 {
            break;
        }
        let file = loadable_file(&mut p);
        let tf = TrackFile::parse(&file).unwrap_or_else(|why| panic!("case {case}: a loadable file refused ({why:?}): {file:02x?}"));
        let Some((t, spots)) = TrackPaths::load(&tf) else {
            // The original measures the lap from a null path: not run.
            refused += 1;
            continue;
        };
        {
            let mut d = disc.borrow_mut();
            d.file = file.clone();
            d.at = 0;
            d.heap = HEAP;
        }
        ee.load(TRACK_OBJ, &[0x5a; 0x100]);
        ee.call(ctor, &[u64::from(TRACK_OBJ)], &[], BUDGET);
        ee.set_u32(TRACK_OBJ + 0x80, 0);
        ee.set_u32(TRACK_OBJ + 0x88, 0);
        ee.call(load, &[u64::from(TRACK_OBJ), u64::from(TRACK_NAME)], &[], 50_000_000);
        let what = format!("case {case}: {file:02x?}");
        assert_eq!(ee.v0() as u32, 1, "{what}: the load");
        assert_eq!(freed.get(), 0, "{what}: the load freed memory");

        let slots: Vec<u32> = (0..t.paths.len() as u32).map(|k| ee.u32_at(ee.u32_at(TRACK_OBJ + 0x50) + 4 * k)).collect();
        for (i, path) in t.paths.iter().enumerate() {
            let r = slots[i];
            assert_eq!(ee.u32_at(r), path.flags, "{what}: path {i}'s flags");
            assert_eq!(ee.u32_at(r + 0x1c), path.dist.0.to_bits(), "{what}: path {i}'s distance");
            let best = ee.u32_at(r + 0x28);
            assert_eq!(slots.iter().position(|&s| s == best && best != 0), path.best, "{what}: path {i}'s best link ({best:#x})");
        }
        assert_eq!(pool_len(&ee, ee.u32_at(TRACK_OBJ + 0x98), 0xc), t.free_routes, "{what}: the route pool");
        assert_eq!(pool_len(&ee, ee.u32_at(TRACK_OBJ + 0xa0), 8), t.free_parents, "{what}: the parent pool");
        assert_eq!(pool_len(&ee, ee.u32_at(TRACK_OBJ + 0xa8), 0xc), t.free_candidates, "{what}: the candidate pool");
        assert_eq!(ee.u32_at(TRACK_OBJ + 0x54), t.lap_length.0.to_bits(), "{what}: the lap length");
        for (k, (o, s)) in [0x58, 0x5c, 0x60].into_iter().zip(spots).enumerate() {
            let g = ee.u32_at(TRACK_OBJ + o);
            match s {
                None => assert_eq!(g, 0, "{what}: gate {k}"),
                Some(s) => {
                    // A spot never written is the reader's zeros, as the port's default.
                    assert_eq!(t_bits(loaded_spot_at(&ee, &slots, g + 0x94)), t_bits(s), "{what}: gate {k}'s spot");
                    crossed_gates += usize::from(s.along != F::ZERO && s.along != F(1.0));
                }
            }
        }
        if t.free_routes < 10 * t.paths.len() {
            dry += 1;
        }
        if t.lap_length == F::ZERO {
            no_way += 1;
        } else {
            laps += 1;
        }
        loaded += 1;
    }
    eprintln!("load: {loaded} loaded ({laps} laps, {no_way} without), {refused} refused, {crossed_gates} gate spots inside a stretch, {dry} dry");
    assert!(refused < 1000 && laps > 5000 && no_way > 300 && crossed_gates > 8000 && dry > 50, "{loaded} {laps} {no_way} {crossed_gates} {dry} {refused}");
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
        RaceNotice::Checkpoint { .. } | RaceNotice::OtherLine { .. } => unreachable!("no line lists here"),
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
                RaceNotice::Checkpoint { .. } | RaceNotice::OtherLine { .. } => unreachable!("no line lists here"),
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

// --- S13c: the race handler held against C31's live sx01 laps ---------------------------------------

/// sx01's track as C31's session holds it: the paths and the line, and the addresses that name each
/// path and node in a captured spot.
struct C31Track {
    paths: TrackPaths,
    course: Course,
    path_at: Vec<u32>,
    node_at: Vec<Vec<u32>>,
}

impl C31Track {
    /// The track object in C31's RAM image (the start state, before any card exists).
    fn load(ee: &Ee, activities: u32) -> C31Track {
        let act = ee.u32_at(ee.u32_at(ee.u32_at(activities) + 0x58));
        let track = ee.u32_at(act + 0xb8);
        let file = track_at(ee, track);
        let mut paths = TrackPaths::new(&file).expect("sx01's paths walk");
        paths.ahead = f32_at(ee, track + 0x38);
        paths.behind = f32_at(ee, track + 0x3c);
        paths.lap_length = f32_at(ee, track + 0x54);
        let path_at: Vec<u32> = (0..ee.u32_at(track + 0x40)).map(|k| ee.u32_at(ee.u32_at(track + 0x50) + 4 * k)).collect();
        let node_at = path_at
            .iter()
            .map(|&p| {
                let mut v = Vec::new();
                let mut n = ee.u32_at(p + 0x10);
                while n != 0 {
                    v.push(n);
                    n = ee.u32_at(n + 0x54);
                }
                v
            })
            .collect();
        let line_at = ee.u32_at(track + 0x5c);
        let mut t = C31Track { paths, course: Course { line: gate_at(ee, line_at), line_spot: Spot::default(), lap_length: f32_at(ee, track + 0x54) }, path_at, node_at };
        t.course.line_spot = t.spot(ee.u32_at(line_at + 0x94), ee.u32_at(line_at + 0x98), f32_at(ee, line_at + 0x9c)).expect("the line's spot");
        t
    }

    /// A captured spot's words as the port's path and node numbers (0, 0 for none).
    fn spot(&self, path: u32, piece: u32, along: F) -> Option<Spot> {
        if path == 0 && piece == 0 {
            return Some(Spot { path: 0, piece: 0, along });
        }
        let p = self.path_at.iter().position(|&a| a == path)?;
        let n = self.node_at[p].iter().position(|&a| a == piece)?;
        Some(Spot::on(p, n, along))
    }
}

/// A score card off a captured row's raw bytes (its first 0x1d0), as the race handler keeps it. None
/// when a spot names a node outside the track (a torn row).
fn c31_card(raw: &[u8], track: &C31Track) -> Option<RaceCard> {
    let u = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
    let f = |o: usize| F(f32::from_bits(u(o)));
    let v = |o: usize| Vf::new(f(o).0, f(o + 4).0, f(o + 8).0, f(o + 12).0);
    let spot = |o: usize| track.spot(u(o), u(o + 4), f(o + 8));
    let lap = LapCard {
        flags: u64::from(u(8)) | u64::from(u(12)) << 32,
        to_line_kept: f(0x10),
        to_line: f(0x14),
        progress: f(0x1c),
        laps_length: f(0x24),
        lap_length: f(0x28),
        place: u(0x2c) as i32,
        laps: u(0x30) as i32,
        runs: u(0x34) as i32,
        on_track: u(0x40) as i32,
        lap_time: f(0x48),
        lap_start: f(0x4c),
        best_lap: f(0x50),
        best_lap_no: u(0x54) as i32,
        seen: Seen { now: v(0x60), last: v(0x70) },
        spot: spot(0x88)?,
        kept_spot: spot(0x94)?,
        race_time: f(0xb4),
        lap_clock: f(0xb8),
        gap: f(0xbc),
        off_track: f(0xc0),
        time_left: f(0xc4),
        attack_clock: f(0x168),
        record_word: u(0x108),
        ..LapCard::default()
    };
    Some(RaceCard { lap, on_path: f(0x18), odometer: f(0x20), aim: spot(0xa0)?, look: v(0x120), ahead: v(0x140), to_track: v(0x180) })
}

/// The card flags that change in C31 by work outside the ported handler: the helpers behind card flag
/// `0x8_0000`, manager flags `0x800` / `0x1000` and the computer aim (S302), and the listeners. No bit
/// the lap line or the ported handler writes is among them.
const C31_UNPORTED_FLAGS: u64 = 0x18_3ce0;

/// The handler's words a card is compared on, named, as raw bits.
fn c31_words(c: &RaceCard) -> Vec<(&'static str, u64)> {
    let l = &c.lap;
    let b = |x: F| u64::from(x.0.to_bits());
    let q = |x: Vf| x.to_bits().iter().take(3).fold(0u64, |h, w| h.rotate_left(21) ^ u64::from(*w));
    let s = |x: Spot| u64::from(x.path) << 48 | u64::from(x.piece) << 32 | b(x.along);
    vec![
        ("flags", l.flags & !C31_UNPORTED_FLAGS),
        ("to_line_kept", b(l.to_line_kept)),
        ("to_line", b(l.to_line)),
        ("on_path", b(c.on_path)),
        ("progress", b(l.progress)),
        ("odometer", b(c.odometer)),
        ("laps_length", b(l.laps_length)),
        ("lap_length", b(l.lap_length)),
        ("laps", l.laps as u64),
        ("on_track", l.on_track as u64),
        ("lap_time", b(l.lap_time)),
        ("lap_start", b(l.lap_start)),
        ("best_lap", b(l.best_lap)),
        ("best_lap_no", l.best_lap_no as u64),
        ("last", q(l.seen.last)),
        ("spot", s(l.spot)),
        ("kept_spot", s(l.kept_spot)),
        ("race_time", b(l.race_time)),
        ("lap_clock", b(l.lap_clock)),
        ("off_track", b(l.off_track)),
        ("time_left", b(l.time_left)),
        ("ahead", q(c.ahead)),
        ("to_track", q(c.to_track)),
    ]
}

/// The score manager's words off a captured row's raw bytes (its first 0x110).
fn c31_rules(raw: &[u8]) -> RaceRules {
    let u = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
    let f = |o: usize| F(f32::from_bits(u(o)));
    RaceRules {
        lap: LapRules {
            flags: u(0x5c),
            kind: u(0x60) as i32,
            laps: u(0x94) as i32,
            ghost_runs: u(0xf0) as i32,
            record_limit: u(0x4c) as i32,
            // The free list is not captured; a race of 4 riders and 3 laps never empties it.
            free_records: 64,
            finish_wait: f(0x9c),
            finish_clock: f(0x98),
            time_limit: f(0xa0),
        },
        below_limit: f(0xb8),
        time_limit: f(0xa0),
        off_limit: f(0xd4),
        aim_reach: f(0xd8),
    }
}

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|k| u8::from_str_radix(&s[2 * k..2 * k + 2], 16).expect("hex")).collect()
}

/// C31's two live sx01 single races, every rider's score card every tick: each tick's card is the tick
/// before's, its rider moved to this tick's place, through the whole race handler per card
/// ([`RaceRules::card`]: the spot search on sx01's paths out of C31's RAM image, the distances to the
/// line, the lap line ([`LapRules::line`]) and the card set back on it, the progress, the off-track clock
/// and the way back, the clocks), against the captured card bit for bit. Every lap counted, every best
/// lap, the finish at the last lap (manager flag [`race_flags::FINISHED`]) and every off-track episode of
/// the gas-held human go through it.
#[test]
fn the_race_handler_holds_c31s_live_sx01_laps() {
    let Ok(kit) = Kit::load() else { return skip("no kit.toml") };
    let image = kit.ctx("states").join("c31-sx01-ai4.ee.bin");
    let ee = match Ee::from_ram(&image) {
        Ok(ee) => ee,
        Err(why) => return skip(&format!("{why} — C31's sx01 start image")),
    };
    let activities = match addr("activity_manager") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let track = C31Track::load(&ee, activities);
    let mut held = Vec::new();
    for (rel, prefix) in [("capture/c31/sx01-gas.csv", "card"), ("capture/c31/sx01-ai4-by-rider.csv", "rider")] {
        let Ok(text) = std::fs::read_to_string(kit.ctx(rel)) else {
            skip(&format!("{rel} is not there: C31's sx01 capture"));
            continue;
        };
        held.push((rel, c31_hold(&text, prefix, &track, rel)));
    }
    if held.is_empty() {
        return skip("no C31 capture");
    }
    eprintln!("{held:#?}");
    // About one row in five is the tick before's again, and the row after it two ticks on: the step
    // between is lost. What each capture still puts through the handler (C31's journal lists the laps).
    for (rel, h) in &held {
        assert!(h.stepped > 20_000 && h.torn < 3_000, "{rel}: {h:?}");
        assert!(h.laps >= 7 && h.bests >= 4 && h.finished == 3, "{rel}: the AI riders' laps and finishes, {h:?}");
        if rel.contains("gas") {
            assert!(h.off > 4_000 && h.too_long >= 10 && h.back >= 25, "{rel}: the human's off-track episodes, {h:?}");
        }
    }
}

fn c31_hold(text: &str, prefix: &str, track: &C31Track, what: &str) -> C31Held {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("{what}: no column {name}"));
    let (c_frame, c_dt, c_mgr) = (col("frame"), col("dt"), col("mgr_raw"));
    let c_raw: Vec<usize> = (0..4).map(|k| col(&format!("{prefix}{k}_0.raw"))).collect();
    struct Row {
        frame: u32,
        dt: F,
        mgr: Vec<u8>,
        cards: Vec<Option<RaceCard>>,
        /// The rider ids in the manager's list order (its `+0x34`, each card's next at `+0x1c8`).
        order: Vec<usize>,
    }
    // The cards sit side by side in rider id order, so a card's address ranks it.
    let order = |mgr: &[u8], raws: &[Vec<u8>]| -> Vec<usize> {
        let word = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let mut at: Vec<u32> = std::iter::once(word(mgr, 0x34)).chain(raws.iter().map(|r| word(r, 0x1c8))).filter(|&a| a != 0).collect();
        let list = at.clone();
        at.sort_unstable();
        list.iter().map(|a| at.binary_search(a).unwrap()).collect()
    };
    let rows: Vec<Row> = lines
        .map(|l| {
            let v: Vec<&str> = l.split(',').collect();
            let raws: Vec<Vec<u8>> = c_raw.iter().map(|&c| hex_bytes(v[c])).collect();
            let mgr = hex_bytes(v[c_mgr]);
            Row {
                frame: v[c_frame].parse().expect("frame"),
                dt: F(v[c_dt].parse::<f32>().expect("dt")),
                order: order(&mgr, &raws),
                cards: raws.iter().map(|r| c31_card(r, track)).collect(),
                mgr,
            }
        })
        .collect();
    let mut paths = track.paths.clone();
    let (mut stepped, mut laps, mut bests, mut finished, mut off, mut back, mut torn) = (0, 0, 0, 0, 0, 0, 0);
    let mut xor = 0u64;
    let (mut long_way, mut too_long) = (0, 0);
    let mut lap_ticks = Vec::new();
    let mut bad: std::collections::BTreeMap<&str, (usize, String)> = Default::default();
    for w in rows.windows(2) {
        let (was, now) = (&w[0], &w[1]);
        if now.frame != was.frame + 1 {
            continue;
        }
        // A torn row: every card still racing runs on one clock, so each advances by this tick's dt.
        let racing = |c: &RaceCard| c.lap.flags & card_flags::TAKING_PART != 0 && c.lap.flags & card_flags::OUT == 0;
        let clean = was.cards.iter().zip(&now.cards).all(|(a, b)| match (a, b) {
            (Some(a), Some(b)) => {
                let settled = |c: &RaceCard| c.lap.seen.now.to_bits()[..3] == c.lap.seen.last.to_bits()[..3];
                !racing(a) || !racing(b) || b.lap.race_time.0.to_bits() == (a.lap.race_time + now.dt).0.to_bits() && settled(a) && settled(b)
            }
            _ => false,
        });
        if !clean {
            torn += 1;
            continue;
        }
        let mut rules = c31_rules(&was.mgr);
        let flags: Vec<u64> = was.cards.iter().flatten().map(|c| c.lap.flags).collect();
        rules.look_for_waited_for(flags);
        for &k in &now.order {
            let (Some(a), Some(b)) = (&was.cards[k], &now.cards[k]) else { continue };
            if a.lap.flags & card_flags::TAKING_PART == 0 || a.lap.flags & card_flags::OUT != 0 {
                continue;
            }
            let mut card = a.clone();
            card.lap.seen.now = b.lap.seen.now;
            let said = rules.card(now.dt, &mut card, None, &mut paths, &track.course, F::ZERO);
            stepped += 1;
            if card.lap.laps != a.lap.laps {
                laps += 1;
                lap_ticks.push((now.frame, k, card.lap.laps));
            }
            bests += usize::from(card.lap.best_lap != a.lap.best_lap);
            finished += usize::from(card.lap.flags & card_flags::OUT != 0);
            off += usize::from(said.contains(&RaceNotice::OffTrack));
            too_long += usize::from(said.contains(&RaceNotice::OffTooLong));
            back += usize::from(a.lap.on_track == 0 && b.lap.on_track != 0);
            xor |= card.lap.flags ^ b.lap.flags;
            // The rider a little behind its kept spot on the same stretch: the spot search's distance is
            // the route forward from the kept spot, the long way round the lap, and that route's answer
            // hangs on the scratch the route searches leave on the paths. The original's drifts tick to
            // tick for a rider standing still (the other cards' searches and the unported computer aim,
            // S302, run first). Both are far past the search's reach, so the progress test, the one
            // reader of the word, reads them alike.
            let behind_kept = b.lap.spot.path == a.lap.kept_spot.path && b.lap.spot.piece == a.lap.kept_spot.piece && b.lap.spot.along < a.lap.kept_spot.along;
            let far = |x: F| x.0 > track.paths.ahead.0;
            for ((name, got), (_, want)) in c31_words(&card).into_iter().zip(c31_words(b)) {
                if got != want && name == "on_path" && behind_kept && far(card.on_path) && far(b.on_path) {
                    long_way += 1;
                    continue;
                }
                if got != want {
                    let e = bad.entry(name).or_insert((0, String::new()));
                    if e.0 < 4 {
                        e.1 += &format!(" | frame {} rider {k}: got {got:#x} want {want:#x}", now.frame);
                    }
                    e.0 += 1;
                }
            }
        }
    }
    eprintln!("{what}: {stepped} steps, {torn} torn, {laps} laps, {bests} bests, {finished} finished, {off} off-track ticks, {too_long} off too long, {back} back on, {long_way} long way round, flags xor {xor:#x}; laps (frame, rider, lap) {lap_ticks:?}");
    assert!(bad.is_empty(), "{what}: words that differ (count, first four): {bad:#?}");
    assert!(xor & !C31_UNPORTED_FLAGS == 0, "{what}: flags {xor:#x}");
    C31Held { stepped, torn, laps, bests, finished, off, too_long, back }
}

/// What one capture put through the handler.
#[derive(Debug, Default)]
struct C31Held {
    stepped: usize,
    torn: usize,
    laps: usize,
    bests: usize,
    finished: usize,
    off: usize,
    too_long: usize,
    back: usize,
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

// --- the start's timeline (S13f) -------------------------------------------------------------------

const TL_ACT: u32 = 0x01c0_0000;
const TL_APP: u32 = 0x01c0_1000;
const TL_SCENE: u32 = 0x01c0_2000;
const TL_LIST_A: u32 = 0x01c0_2100;
const TL_LIST_B: u32 = 0x01c0_2200;
const TL_CAMERA: u32 = 0x01c0_2300;
const TL_SETTINGS: u32 = 0x01c0_3000;
const TL_VIEWPORTS: u32 = 0x01c0_3100;
const TL_UI: u32 = 0x01c0_4000;
const TL_TIPS: u32 = 0x01c0_4100;
const TL_TIP: u32 = 0x01c0_4200;
const TL_CLIP_VT: u32 = 0x01c0_5000;
const TL_CALLS: u32 = 0x01c0_5100;
const TL_INTROS: u32 = 0x01c0_5200;
const TL_CLIPS: u32 = 0x01c0_6000;
const TL_ACTOR_VT: u32 = 0x01c0_7000;
const TL_NODES: u32 = 0x01c0_8000;
const TL_ACTORS: u32 = 0x01c0_9000;
const TL_ITEMS: u32 = 0x01c0_a000;
const TL_BODIES: u32 = 0x01c1_0000;
const TL_WORDS: u32 = 0x01c2_0000;
/// Not code: the actor's cast, the clips' play, "is playing" and fade slots, and the nothing slot.
const TL_HOOK_CAST: u32 = 0x01c0_f000;
const TL_HOOK_PLAY: u32 = 0x01c0_f100;
const TL_HOOK_PLAYING: u32 = 0x01c0_f200;
const TL_HOOK_FADE: u32 = 0x01c0_f300;
/// The interface class's bit, as the hooked name lookup hands it out.
const TL_CLASS_BIT: u64 = 0x40;

/// A word for the timeline's clock: zero, minus zero, a denormal, just either side of an edge, an
/// edge 1.5 s on, or anywhere around the windows.
fn timeline_clock(p: &mut Pick, edges: &[F]) -> F {
    let e = edges[p.below(edges.len() as u64) as usize];
    match p.below(10) {
        0 => F::ZERO,
        1 => F(-0.0),
        2 => F(f32::from_bits(p.next() as u32 & 0x807f_ffff)),
        3 => e,
        4 => F(f32::from_bits(e.0.to_bits().wrapping_add(1))),
        5 => F(f32::from_bits(e.0.to_bits().wrapping_sub(1))),
        6 => F(fpu::add(1.5, e.0)),
        7 => F(fpu::add(fpu::add(1.5, e.0), 0.05)),
        8 => F(p.stray(e.0)),
        _ => F(p.range(-1.0, 13.0)),
    }
}

#[test]
fn the_start_timeline_matches_the_countdown_listener() {
    let Some(mut ee) = game_or_skip() else { return };
    let [listener, app, ui, name_bit, tip_find, show, cut] = match addrs([
        "race_countdown_listener",
        "app_pointer",
        "ui_manager",
        "name_bit",
        "race_tip_find",
        "hud_overlay_show",
        "scene_camera_cut",
    ]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let played: Rc<RefCell<Vec<u32>>> = Rc::default();
    let faded: Rc<RefCell<Vec<(u32, u64)>>> = Rc::default();
    let shown: Rc<Cell<u32>> = Rc::default();
    let playing = Rc::new(Cell::new(0u64));
    let (pl, fa, sh, an) = (played.clone(), faded.clone(), shown.clone(), playing.clone());
    ee.hooks.insert(name_bit, Box::new(|ee| ee.set(2, TL_CLASS_BIT)));
    ee.hooks.insert(tip_find, Box::new(|ee| ee.set(2, 0)));
    ee.hooks.insert(show, Box::new(move |ee| {
        assert_eq!(ee.r(4) as u32, TL_TIP + 0x40, "the overlay shown");
        sh.set(sh.get() + 1);
    }));
    ee.hooks.insert(cut, Box::new(|_| {}));
    // The cast answers with the actor's item (its +0x40, 0 for an actor of another class) for the
    // interface class's bit only.
    ee.hooks.insert(TL_HOOK_CAST, Box::new(|ee| {
        let item = if ee.r(5) == TL_CLASS_BIT { u64::from(ee.u32_at(ee.r(4) as u32 + 0x40)) } else { 0 };
        ee.set(2, item);
    }));
    ee.hooks.insert(TL_HOOK_PLAY, Box::new(move |ee| pl.borrow_mut().push(ee.r(4) as u32)));
    ee.hooks.insert(TL_HOOK_PLAYING, Box::new(move |ee| ee.set(2, an.get())));
    ee.hooks.insert(TL_HOOK_FADE, Box::new(move |ee| fa.borrow_mut().push((ee.r(4) as u32, ee.r(6)))));
    ee.set_u32(app, TL_APP);
    ee.set_u32(ui, TL_UI);
    ee.set_u32(TL_UI + 0x30, TL_TIPS);
    ee.set_u32(TL_TIPS + 4, TL_TIPS + 0x80);
    ee.set_u32(TL_TIPS + 0x80, TL_TIP);
    ee.set_u32(TL_TIP + 4, TL_TIP + 0x40);
    ee.set_u32(TL_APP + 0x3cc, TL_SCENE);
    ee.set_u32(TL_APP + 0x3d4, TL_SETTINGS);
    ee.set_u32(TL_SETTINGS + 8, TL_VIEWPORTS);
    ee.set_u32(TL_SCENE + 0x30, TL_LIST_A);
    ee.set_u32(TL_LIST_A + 0x30, TL_LIST_B);
    ee.set_u32(TL_SCENE + 0x64, TL_CAMERA);
    ee.set_u32(TL_SCENE + 0x68, TL_CAMERA);
    ee.set_u32(TL_ACTOR_VT + 0xa4, TL_HOOK_CAST);
    ee.set_u32(TL_CLIP_VT + 0x1c, TL_HOOK_PLAY);
    ee.set_u32(TL_CLIP_VT + 0x20, TL_HOOK_PLAYING);
    ee.set_u32(TL_CLIP_VT + 0x34, TL_HOOK_FADE);
    for k in 0..16 {
        let clip = TL_CLIPS + 0x10 * k;
        ee.set_u32(clip + 8, TL_CLIP_VT);
        ee.set_u32(TL_CALLS + 4 * k, clip);
        ee.set_u32(TL_INTROS + 4 * k, clip + 0x800);
        ee.set_u32(clip + 0x800 + 8, TL_CLIP_VT);
    }

    let mut p = Pick(0x57a7_7713_e11e_0f13);
    let mut seen = StartStep::default();
    let (mut both, mut lifted, mut restarted) = (0, 0, 0);
    for case in 0..20_000 {
        let mut s = Supercross::new();
        // Windows as a reset leaves them (with the intro or without) or anywhere, now and then stray.
        let intro = if p.below(2) == 0 { F(6.0) } else { F::ZERO };
        let drop = F(p.range(0.0, 4.0));
        let pick = |p: &mut Pick, x: F| {
            if p.below(5) == 0 {
                let r = p.range(-2.0, 12.0);
                F(p.stray(r))
            } else {
                x
            }
        };
        s.lights_on = pick(&mut p, intro);
        s.lights_off = pick(&mut p, F(fpu::add(0.1, s.lights_on.0)));
        s.warn_on = pick(&mut p, F(fpu::add(fpu::sub(drop.0, 1.5), intro.0)));
        s.warn_off = pick(&mut p, F(fpu::add(0.1, s.warn_on.0)));
        s.start_cue = F(p.stray(0.1));
        s.start_delay = F(p.range(0.0, 12.0));
        s.intro_seen = if p.below(2) == 0 { 0 } else { 1 + p.below(3) as u32 };
        s.intro_queued = p.below(2) as u32;
        s.call = p.below(16) as i32;
        s.intro = p.below(16) as i32;
        // The countdown's elapsed time is the clock handed in; its accumulator any word.
        let edges = [s.lights_on, s.lights_off, s.warn_on, s.warn_off, F(6.0)];
        let time = timeline_clock(&mut p, &edges);
        let acc = p.next() as u32;
        let mode = p.below(5) as u32;
        playing.set(p.below(2));
        let cam_view = if p.below(3) == 0 { 0 } else { 0x1234 };
        let cam_now = if p.below(2) == 0 { cam_view } else { 0x5678 };
        let intro_camera = cam_view != 0 && cam_now == cam_view;
        // Up to five actors: some of another class, some interface items without a body.
        let actors = p.below(6) as u32;
        let mut items: Vec<Option<StartItem>> = Vec::new();
        for k in 0..actors {
            let item = (p.below(5) != 0).then(|| StartItem {
                body: (p.below(4) != 0).then(|| Axes {
                    free: core::array::from_fn(|_| p.below(3) as i32),
                    ang_free: core::array::from_fn(|_| p.below(3) as i32),
                }),
                words: core::array::from_fn(|_| p.next() as i32),
            });
            let (node, actor, at) = (TL_NODES + 0x10 * k, TL_ACTORS + 0x80 * k, TL_ITEMS + 0x100 * k);
            ee.set_u32(node, actor);
            ee.set_u32(node + 8, if k + 1 < actors { node + 0x10 } else { 0 });
            ee.set_u32(actor + 8, TL_ACTOR_VT);
            ee.set_u32(actor + 0x40, if item.is_some() { at } else { 0 });
            if let Some(it) = &item {
                let body = TL_BODIES + 0x400 * k;
                ee.set_u32(at + 0x68, if it.body.is_some() { body } else { 0 });
                if let Some(b) = it.body {
                    for (j, x) in b.free.iter().chain(&b.ang_free).enumerate() {
                        ee.set_u32(body + 0x2a0 + 4 * j as u32, *x as u32);
                    }
                }
                for (j, w) in it.words.iter().enumerate() {
                    let word = TL_WORDS + 0x40 * k + 4 * j as u32;
                    ee.set_u32(at + 8 + 4 * j as u32, word);
                    ee.set_u32(word, *w as u32);
                }
            }
            items.push(item);
        }
        ee.set_u32(TL_LIST_B + 0x34, if actors > 0 { TL_NODES } else { 0 });
        ee.set_u32(TL_CAMERA + 0x68, cam_view);
        ee.set_u32(TL_CAMERA + 0x40, cam_now);
        ee.set_u32(TL_VIEWPORTS + 4, mode);
        put_f(&mut ee, TL_ACT + 0x10c, s.start_delay);
        put_f(&mut ee, TL_ACT + 0x110, s.start_cue);
        put_f(&mut ee, TL_ACT + 0x50 + 0xc4, time);
        ee.set_u32(TL_ACT + 0x50 + 0xc8, acc);
        ee.set_u32(TL_ACT + 0x440, TL_INTROS);
        ee.set_u32(TL_ACT + 0x478, s.intro as u32);
        ee.set_u32(TL_ACT + 0x484, TL_CALLS);
        ee.set_u32(TL_ACT + 0x4ac, s.call as u32);
        for (o, x) in [(0x4b0, s.lights_on), (0x4b4, s.lights_off), (0x4b8, s.warn_on), (0x4bc, s.warn_off)] {
            put_f(&mut ee, TL_ACT + o, x);
        }
        ee.set_u32(TL_ACT + 0x4c4, s.intro_seen);
        ee.set_u32(TL_ACT + 0x4c8, s.intro_queued);
        played.borrow_mut().clear();
        faded.borrow_mut().clear();
        shown.set(0);
        ee.call(listener, &[u64::from(TL_ACT)], &[p.range(0.0, 10.0), time.0, 0.1], BUDGET);

        let mut want: Vec<StartItem> = items.iter().flatten().copied().collect();
        let step = s.start_timeline(time, &mut want, intro_camera);
        let what = format!("case {case}: t {:#010x}, {s:?}", time.0.to_bits());
        assert_eq!(ee.v0(), 1, "{what}: the answer");
        for (o, x, name) in [
            (0x10c, s.start_delay, "start delay"),
            (0x110, s.start_cue, "cue"),
            (0x4b0, s.lights_on, "lights on"),
            (0x4b4, s.lights_off, "lights off"),
            (0x4b8, s.warn_on, "warning on"),
            (0x4bc, s.warn_off, "warning off"),
        ] {
            assert_eq!(ee.u32_at(TL_ACT + o), x.0.to_bits(), "{what}: {name}");
        }
        let elapsed = step.elapsed.unwrap_or(time);
        assert_eq!(ee.u32_at(TL_ACT + 0x50 + 0xc4), elapsed.0.to_bits(), "{what}: the countdown's elapsed");
        let acc = if step.intro_ended { 0 } else { acc };
        assert_eq!(ee.u32_at(TL_ACT + 0x50 + 0xc8), acc, "{what}: the countdown's accumulator");
        assert_eq!(ee.u32_at(TL_ACT + 0x4c4), s.intro_seen, "{what}: intro seen");
        assert_eq!(ee.u32_at(TL_ACT + 0x4c8), s.intro_queued, "{what}: intro ended");
        let mut want = want.into_iter();
        for (k, item) in items.iter().enumerate() {
            let Some(_) = item else { continue };
            let w = want.next().unwrap();
            let k = k as u32;
            if let Some(b) = w.body {
                let body = TL_BODIES + 0x400 * k;
                let got: Vec<i32> = (0..6).map(|j| ee.u32_at(body + 0x2a0 + 4 * j) as i32).collect();
                let wanted: Vec<i32> = b.free.iter().chain(&b.ang_free).copied().collect();
                assert_eq!(got, wanted, "{what}: item {k}'s body");
            }
            let got: Vec<i32> = (0..4).map(|j| ee.u32_at(TL_WORDS + 0x40 * k + 4 * j) as i32).collect();
            assert_eq!(got, w.words, "{what}: item {k}'s words");
        }
        let calls: Vec<u32> = if step.call { vec![TL_CLIPS + 0x10 * s.call as u32] } else { vec![] };
        assert_eq!(*played.borrow(), calls, "{what}: the start call played");
        let fades = if step.intro_ended && playing.get() != 0 {
            vec![(TL_CLIPS + 0x800 + 0x10 * s.intro as u32, 750)]
        } else {
            vec![]
        };
        assert_eq!(*faded.borrow(), fades, "{what}: the intro faded");
        let overlays = if !step.overlays { 0 } else { [1, 2, 2, 0, 0][mode as usize] };
        assert_eq!(shown.get(), overlays, "{what}: the overlays shown");
        seen.armed |= step.armed;
        seen.intro_ended |= step.intro_ended;
        seen.overlays |= step.overlays;
        seen.call |= step.call;
        seen.warning |= step.warning;
        both += (step.armed && step.intro_ended) as u32;
        lifted += (step.elapsed == Some(F(6.0))) as u32;
        restarted += (step.elapsed == Some(F::ZERO)) as u32;
    }
    assert_eq!(seen, StartStep { armed: true, intro_ended: true, overlays: true, call: true, warning: true, elapsed: None });
    assert!(
        both > 100 && lifted > 300 && restarted > 300,
        "{both} armed with the intro's end, {lifted} jumped to the intro's end, {restarted} started over"
    );
}

#[test]
fn the_grid_with_all_ai_set_keeps_the_lanes() {
    let Some(mut ee) = game_or_skip() else { return };
    let [grid, app, all_ai, rand_state, ground, interface, base, overlays, ui, tip_find, mode, nth] = match addrs([
        "activity_supercross_grid",
        "app_pointer",
        "all_ai",
        "rand_state",
        "ground_query",
        "interface_id",
        "activity_grid_base",
        "race_overlays_bind",
        "ui_manager",
        "race_tip_find",
        "hud_overlay_mode",
        "list_nth",
    ]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let grounded: Rc<RefCell<Vec<Vf>>> = Rc::default();
    let placed: Rc<RefCell<Vec<(u32, Vf, Vf)>>> = Rc::default();
    let modes: Rc<RefCell<Vec<(u32, u64)>>> = Rc::default();
    let resets: Rc<Cell<u32>> = Rc::default();
    let (g, pl, md, r1, r2) = (grounded.clone(), placed.clone(), modes.clone(), resets.clone(), resets.clone());
    ee.hooks.insert(HOOK_NOTHING, Box::new(|_| {}));
    ee.hooks.insert(ground, Box::new(move |ee| g.borrow_mut().push(quad(ee, ee.r(5) as u32))));
    ee.hooks.insert(
        HOOK_PLACE,
        Box::new(move |ee| pl.borrow_mut().push((ee.r(4) as u32, quad(ee, ee.r(5) as u32), quad(ee, ee.r(6) as u32)))),
    );
    // The vehicles' and riders' resets are the shuffled path's: with AllAI neither the list nor the
    // interface ids are looked at.
    ee.hooks.insert(interface, Box::new(move |_| r1.set(r1.get() + 1)));
    ee.hooks.insert(nth, Box::new(move |ee| {
        r2.set(r2.get() + 1);
        ee.set(2, 0);
    }));
    ee.hooks.insert(base, Box::new(|_| {}));
    ee.hooks.insert(overlays, Box::new(|_| {}));
    ee.hooks.insert(tip_find, Box::new(|ee| ee.set(2, 0)));
    ee.hooks.insert(mode, Box::new(move |ee| md.borrow_mut().push((ee.r(4) as u32, ee.r(5)))));
    ee.set_u32(app, APP);
    ee.set_u32(ui, TL_UI);
    ee.set_u32(TL_UI + 0x30, TL_TIPS);
    ee.set_u32(TL_TIPS + 4, TL_TIPS + 0x80);
    ee.set_u32(TL_TIPS + 0x80, TL_TIP);
    ee.set_u32(TL_TIP + 4, TL_TIP + 0x40);
    ee.set_u32(APP + 0x3cc, SCENE);
    ee.set_u32(SCENE + 0x74, 0);
    ee.set_u32(ENT_VT + 0x8c, HOOK_PLACE);
    ee.set_u32(ENT_VT + 0x88, HOOK_NOTHING);
    ee.set_u32(GRID_OBJ + 0x58, GRID_GATE);
    ee.set_u32(TRACK + 0x348, START);
    ee.set_u32(TRACK + 0x428, WORLD);

    let table = Supercross::lane_table();
    let mut p = Pick(0xa11a_1000_6e1d_0013);
    for case in 0..2_000 {
        let riders = 1 + p.below(LANES as u64) as usize;
        let mut ids: Vec<i32> = (0..riders as i32).collect();
        for k in (1..riders).rev() {
            ids.swap(k, p.below(k as u64 + 1) as usize);
        }
        let cards: Vec<GridCard> = ids.iter().map(|&id| GridCard { id, ridden: p.below(6) != 0 }).collect();
        let mut s = Supercross::new();
        s.lane_offsets = table;
        s.lanes = core::array::from_fn(|_| p.below(8) as i32);
        let normal = p.unit_vector();
        let gated = p.below(4) != 0;
        let start = if gated {
            let centre = [p.range(-500.0, 500.0), p.range(-50.0, 150.0), p.range(-500.0, 500.0)];
            GridStart::Gate { centre: p.quad(centre), normal: p.quad(normal), width: F(p.range(1.0, 150.0)) }
        } else {
            let at = [p.range(-500.0, 500.0), p.range(-50.0, 150.0), p.range(-500.0, 500.0)];
            GridStart::Point { at: p.quad(at), facing: p.quad(normal) }
        };
        let state = p.next();
        let knob = [1u32, 2, 0xffff_ffff, 0x8000_0000][p.below(4) as usize];

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
        ee.set_u32(all_ai, knob);
        ee.set_u32(ACT + 0x14, 1 + p.below(4) as u32);
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
        modes.borrow_mut().clear();
        resets.set(0);
        ee.call(grid, &[u64::from(ACT)], &[], BUDGET);

        let mut rng = Rng::from_state(state);
        s.shuffle_lanes(riders, true, &mut rng);
        let what = format!("case {case}: {riders} riders, AllAI {knob:#x}, {start:?}");
        for k in 0..riders as u32 {
            assert_eq!(ee.u32_at(ACT + 0x418 + 4 * k) as i32, k as i32, "{what}: lane {k}");
            assert_eq!(s.lanes[k as usize], k as i32, "{what}: the port's lane {k}");
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
        assert_eq!(ee.read(rand_state, 8) as u64, state, "{what}: no draw");
        assert_eq!(rng.state(), state, "{what}: the port drew");
        assert_eq!(*modes.borrow(), vec![(TL_TIP + 0x40, 6)], "{what}: the race overlays' mode");
        assert_eq!(resets.get(), 0, "{what}: the vehicles were reset");
    }
}

/// S196's sx01 image, 3.6 s into the countdown and short of the lights: the start's items as the port
/// reads them. The one item is the rider's own interface, its body the rider's bike chassis; the
/// countdown's first call armed it and nothing in the timeline has run since but its idle calls.
#[test]
fn the_sx01_image_holds_the_start_hold_as_armed() {
    let Some(mut ee) = sx01_image() else { return };
    let [app_pointer, activities, gp, names, name_bit] =
        match addrs(["app_pointer", "activity_manager", "boot_gp", "actor_class_names", "name_bit"]) {
            Ok(a) => a,
            Err(why) => return skip(&why),
        };
    ee.set(28, u64::from(gp));
    let act = ee.u32_at(ee.u32_at(ee.u32_at(activities) + 0x58));
    let elapsed = f32_at(&ee, act + 0x50 + 0xc4);
    let mut s = Supercross::new();
    for (o, x) in [(0x4b0, &mut s.lights_on), (0x4b4, &mut s.lights_off), (0x4b8, &mut s.warn_on), (0x4bc, &mut s.warn_off)] {
        *x = f32_at(&ee, act + o);
    }
    s.intro_seen = ee.u32_at(act + 0x4c4);
    s.intro_queued = ee.u32_at(act + 0x4c8);
    assert!(elapsed > F::ZERO && elapsed < s.lights_on, "the image is {elapsed} s in, lights at {}", s.lights_on);
    assert_eq!((s.intro_seen, s.intro_queued), (0, 0), "the intro is still on");

    // The interface class's bit, looked up by name as the timeline does (already registered).
    let name = 0x01f0_0000;
    for (k, b) in b"MXSupercrossInterface\0".iter().enumerate() {
        ee.write(name + k as u32, 1, u128::from(*b));
    }
    let registered = ee.u32_at(names);
    ee.call(name_bit, &[u64::from(names), u64::from(name)], &[], BUDGET);
    let bit = ee.v0();
    assert_eq!(ee.u32_at(names), registered, "the class was registered before");
    let scene = ee.u32_at(ee.u32_at(app_pointer) + 0x3cc);
    let camera = ee.u32_at(scene + 0x64);
    let intro_camera = ee.u32_at(camera + 0x68) != 0 && ee.u32_at(camera + 0x40) == ee.u32_at(camera + 0x68);
    let mut node = ee.u32_at(ee.u32_at(ee.u32_at(scene + 0x30) + 0x30) + 0x34);
    let mut items = Vec::new();
    while node != 0 {
        let actor = ee.u32_at(node);
        ee.call(ee.u32_at(ee.u32_at(actor + 8) + 0xa4), &[u64::from(actor), bit], &[], BUDGET);
        if ee.v0() != 0 {
            items.push(ee.v0() as u32);
        }
        node = ee.u32_at(node + 8);
    }
    assert_eq!(items.len(), 1, "one rider, one interface");
    let item = items[0];
    let card = ee.u32_at(act + 0x84);
    let chassis = ee.u32_at(ee.u32_at(card + 4) - 0xf9c + 0x7a4);
    assert_eq!(ee.u32_at(item + 0x68), chassis, "the item's body is the rider's bike chassis");
    let read_item = |ee: &Ee| StartItem {
        body: Some(Axes {
            free: core::array::from_fn(|j| ee.u32_at(chassis + 0x2a0 + 4 * j as u32) as i32),
            ang_free: core::array::from_fn(|j| ee.u32_at(chassis + 0x2ac + 4 * j as u32) as i32),
        }),
        words: core::array::from_fn(|j| ee.u32_at(ee.u32_at(item + 8 + 4 * j as u32)) as i32),
    };
    let got = read_item(&ee);

    // Armed at the countdown's zero; since then each call (in no window) has left it.
    let mut want = [StartItem { body: Some(Axes::default()), words: [9; 4] }];
    let mut zero = s.clone();
    let armed = zero.start_timeline(F::ZERO, &mut want, intro_camera);
    assert!(armed.armed && !armed.call && !armed.warning);
    let idle = s.clone().start_timeline(elapsed, &mut want, intro_camera);
    assert_eq!(idle, StartStep::default(), "the image's clock is in no window");
    assert_eq!(got.words, want[0].words, "the item's words");
    assert_eq!(got.body.unwrap().free, want[0].body.unwrap().free, "the bike held to height only");
    // The turn freedoms read 1, 1, 0 where the arming wrote 1, 0, 1: the bike's ground step rewrites
    // them every tick (S290, the_sx01_image_s_held_bike_turns_as_its_ground_step_sets).
    assert_eq!(got.body.unwrap().ang_free, [1, 1, 0], "the bike's turn freedoms");
}

/// S290: the start hold on a rider's chassis ([`race::hold`], [`StartStep::apply`]) is what the
/// timeline writes on an item's body (the timeline is the oracled one, proof 18): over 20,000 calls
/// at clocks on and around every window edge and zero, with any freedoms to start from, the chassis
/// the step is applied to holds the item's six freedoms afterwards.
#[test]
fn the_start_hold_on_a_chassis_is_the_timelines_on_its_body() {
    let mut x = 0x9e37_79b9_u32;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        x
    };
    let (mut armed, mut warned, mut idle) = (0, 0, 0);
    for case in 0..20_000 {
        let mut s = Supercross::new();
        s.lights_on = F((next() % 80) as f32 * 0.1);
        s.lights_off = F(fpu::add(0.1, s.lights_on.0));
        s.warn_on = F((next() % 130) as f32 * 0.1);
        s.warn_off = F(fpu::add(0.1, s.warn_on.0));
        s.intro_seen = next() % 2;
        s.intro_queued = next() % 2;
        let edges = [F::ZERO, -F::ZERO, s.lights_on, s.lights_off, s.warn_on, s.warn_off, F(f32::from_bits(1))];
        let base = edges[next() as usize % edges.len()];
        let t = match next() % 4 {
            0 => base,
            1 => F(f32::from_bits(base.0.to_bits().wrapping_add(1))),
            2 => F(f32::from_bits(base.0.to_bits().wrapping_sub(1))),
            _ => F((next() % 140) as f32 * 0.1),
        };
        let free: [i32; 3] = core::array::from_fn(|_| (next() % 3) as i32);
        let ang_free: [i32; 3] = core::array::from_fn(|_| (next() % 3) as i32);
        let mut ch = Chassis { free, ang_free, ..Chassis::default() };
        let mut items = [StartItem { body: Some(Axes { free, ang_free }), words: [0; 4] }];
        let step = s.start_timeline(t, &mut items, false);
        step.apply(&mut ch);
        let want = items[0].body.unwrap();
        assert_eq!((ch.free, ch.ang_free), (want.free, want.ang_free), "case {case}: t {t:?}, step {step:?}");
        armed += usize::from(step.armed);
        warned += usize::from(step.warning);
        idle += usize::from(!step.armed && !step.warning);
    }
    assert!(armed > 1000 && warned > 1000 && idle > 1000, "armed {armed}, warned {warned}, idle {idle}");
    let mut ch = Chassis::default();
    race::hold(&mut ch);
    assert_eq!((ch.free, ch.ang_free), ([0, 1, 0], [1, 0, 1]), "the hold");
}

/// S290: the turn freedoms on S196's sx01 image (1, 1, 0 where the arming wrote 1, 0, 1) are the
/// bike's ground step's: the bike is on its ground step (its air-state word `+0x748` clear) with a
/// wheel down (`+0x574`), and that step sets them to 1, 1, 0 every tick a wheel is down
/// (`chassis::set_flags` from `move::pose`). Its linear freedoms are the hold's alone: nothing else
/// writes them between the countdown's zero and the warning.
#[test]
fn the_sx01_image_s_held_bike_turns_as_its_ground_step_sets() {
    let Some(ee) = sx01_image() else { return };
    let [activities] = match addrs(["activity_manager"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let act = ee.u32_at(ee.u32_at(ee.u32_at(activities) + 0x58));
    let bike = ee.u32_at(ee.u32_at(act + 0x84) + 4) - 0xf9c;
    let chassis = ee.u32_at(bike + 0x7a4);
    assert_eq!(ee.u32_at(bike + 0x748), 0, "the bike runs its ground step");
    assert_ne!(ee.u32_at(bike + 0x574), 0, "a wheel is down");
    let mut held = Chassis::default();
    race::hold(&mut held);
    game_sim::chassis::set_flags(&mut held, 1, 1, i32::from(ee.u32_at(bike + 0x574) == 0));
    let read = |o: u32| -> [i32; 3] { core::array::from_fn(|j| ee.u32_at(chassis + o + 4 * j as u32) as i32) };
    assert_eq!(read(0x2a0), held.free, "held to height only");
    assert_eq!(read(0x2ac), held.ang_free, "the ground step's turn freedoms over the hold's");
}


// --- S294a: the race handler's line lists ------------------------------------------------------------

/// Free RAM for the line lists' manager, card, owner, chain, listener, gates, vtables and hooks; the
/// paths go where `put_paths` lays them.
const LL_MGR: u32 = 0x01c0_0000;
const LL_CARD: u32 = 0x01c0_1000;
const LL_OWNER: u32 = 0x01c0_2000;
const LL_CHAIN: u32 = 0x01c0_2100;
const LL_LISTENER: u32 = 0x01c0_2200;
const LL_LINES: u32 = 0x01c0_3000;
const LL_RECORDS: u32 = 0x01c0_4000;
const LL_GATES: u32 = 0x01c1_0000;
const LL_VT: u32 = 0x01c2_0000;
const LL_HOOKS: u32 = 0x01c2_1000;

/// The calls a line-list crossing makes: (who, slot, the gate, and the card for the listener).
fn lists_told(n: RaceNotice, gates: &[u32], others: &[u32], owner: bool, chain: bool) -> Vec<(u32, u32, u32, u32)> {
    let (gate, o, l) = match n {
        RaceNotice::Checkpoint { line, through } => (gates[line], if through { 0x24 } else { 0x28 }, if through { 0x1c } else { 0x20 }),
        RaceNotice::OtherLine { line, through } => (others[line], if through { 0x2c } else { 0x30 }, if through { 0x24 } else { 0x28 }),
        _ => panic!("{n:?} is not a line list's"),
    };
    let mut v = Vec::new();
    if owner {
        v.push((OWNER, o, gate, 0));
    }
    if chain {
        v.push((CHAIN, o, gate, 0));
    }
    v.push((TO_LISTENER, l, gate, LL_CARD));
    v
}

#[test]
fn the_line_lists_match_the_race_handlers_card_stretch() {
    let Some(mut ee) = game_or_skip() else { return };
    let [start, end] = match addrs(["score_race_lines", "score_race_lines_end"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard: Rc<RefCell<Vec<(u32, u32, u32, u32)>>> = Rc::default();
    for who in [OWNER, CHAIN, TO_LISTENER] {
        for slot in (0..0x80).step_by(4) {
            let at = LL_HOOKS + who * 0x100 + slot;
            ee.set_u32(LL_VT + who * 0x100 + slot, at);
            let heard = heard.clone();
            ee.hooks.insert(
                at,
                Box::new(move |ee| {
                    let r = |i: usize| ee.r(i) as u32;
                    let told = if who == TO_LISTENER { (r(7), r(6)) } else { (r(6), 0) };
                    assert_eq!(r(5), LL_MGR, "the manager is passed along");
                    heard.borrow_mut().push((who, slot, told.0, told.1));
                }),
            );
        }
    }
    ee.hooks.insert(end, Box::new(|ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));
    let mut p = Pick(0x51d3_08aa_e62c_94f7);
    let mut said = [0usize; 4];
    let (mut passed, mut far, mut cleared) = (0, 0, 0);
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        let make = |p: &mut Pick, t: &TrackPaths| {
            let (mut gate, spot) = if p.below(4) == 0 { (gate(p), t_spot_on(p, t)) } else { line_on(p, t) };
            if p.below(6) == 0 {
                gate.flags &= !flags::CLOSED;
            }
            ListGate { gate, spot }
        };
        let lists = LineLists {
            checkpoints: (0..p.below(5)).map(|_| make(&mut p, &t)).collect(),
            others: (0..p.below(4)).map(|_| make(&mut p, &t)).collect(),
            reach: match p.below(5) {
                0 => F::ZERO,
                1 => F(-p.range(0.0, 10.0)),
                2 => F(p.range(0.0, 20.0)),
                _ => F(p.range(20.0, 400.0)),
            },
        };
        let mut rules = RaceRules::default();
        rules.lap.flags = p.next() as u32;
        for bit in [race_flags::CHECKPOINTS, race_flags::ZONES] {
            if p.below(5) == 0 { rules.lap.flags &= !bit } else { rules.lap.flags |= bit }
        }
        let n = lists.checkpoints.len();
        let mut lap = lap_card(&mut p, 0);
        if p.below(2) == 0 {
            lap.flags |= race_card_flags::WAITED_FOR;
        }
        if p.below(4) != 0 {
            lap.lines_passed = (0..n).map(|_| if p.below(3) == 0 { p.next() as u32 | 1 } else { 0 }).collect();
        }
        lap.kept_spot = t_spot_on(&mut p, &t);
        let all: Vec<ListGate> = lists.checkpoints.iter().chain(&lists.others).copied().collect();
        lap.spot = match p.below(4) {
            0 => lap.kept_spot,
            1 if !all.is_empty() => all[p.below(all.len() as u64) as usize].spot,
            _ => t_spot_on(&mut p, &t),
        };
        let (now, last) = if all.is_empty() || p.below(6) == 0 {
            let (x, z) = (p.range(-60.0, 60.0), p.range(-60.0, 60.0));
            (p.quad([x, 0.0, z]), p.quad([0.0, 0.0, 0.0]))
        } else {
            let k = p.below(all.len() as u64) as usize;
            step(&mut p, &all[k].gate)
        };
        lap.seen = Seen { now, last };
        let card = RaceCard {
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

        put_paths(&mut ee, &mut p, &t);
        put_race_card(&mut ee, LL_CARD, &card, LL_LINES, LL_RECORDS);
        ee.set_u32(LL_CARD + 4, if owner { LL_OWNER } else { 0 });
        ee.set_u32(LL_CARD + 0x1c4, if chain { LL_CHAIN } else { 0 });
        ee.set_u32(LL_OWNER + 0x20, LL_VT + OWNER * 0x100);
        ee.set_u32(LL_CHAIN + 0x1c, 0);
        ee.set_u32(LL_CHAIN + 0x20, LL_VT + CHAIN * 0x100);
        ee.set_u32(LL_LISTENER + 4, 0);
        ee.set_u32(LL_LISTENER + 0x1c, LL_VT + TO_LISTENER * 0x100);
        let put_list = |ee: &mut Ee, p: &mut Pick, base: u32, list: &[ListGate]| -> Vec<u32> {
            let at: Vec<u32> = (0..list.len() as u32).map(|k| base + 0x100 * k).collect();
            for (k, g) in list.iter().enumerate() {
                for o in (0..0xa0).step_by(4) {
                    ee.set_u32(at[k] + o, p.next() as u32);
                }
                put_gate(ee, at[k], &g.gate);
                ee.set_u32(at[k] + 0x90, at.get(k + 1).copied().unwrap_or(0));
                put_t_spot(ee, at[k] + 0x94, g.spot);
            }
            at
        };
        let gates = put_list(&mut ee, &mut p, LL_GATES, &lists.checkpoints);
        let others = put_list(&mut ee, &mut p, LL_GATES + 0x8000, &lists.others);
        ee.set_u32(LL_MGR + 0x30, LL_LISTENER);
        ee.set_u32(LL_MGR + 0x5c, rules.lap.flags);
        ee.set_u32(LL_MGR + 0x68, T_TRACK);
        ee.set_u32(LL_MGR + 0x70, gates.first().copied().unwrap_or(0));
        ee.set_u32(LL_MGR + 0x78, others.first().copied().unwrap_or(0));
        put_f(&mut ee, LL_MGR + 0x100, lists.reach);
        ee.set_u32(0x28, 0);
        heard.borrow_mut().clear();
        ee.set(16, u64::from(LL_CARD));
        ee.set(19, u64::from(LL_MGR));
        ee.set_f(24, 0.0);
        ee.call(start, &[], &[], BUDGET * 20);

        let flags_before: Vec<u32> = t.paths.iter().map(|q| q.flags).collect();
        rules.clear_checkpoint(&card, &mut t);
        let notices = rules.line_lists(&card, &lists, &mut t);
        let what = format!("case {case}: flags {:#x}, {} + {} lines -> {notices:?}", rules.lap.flags, n, lists.others.len());
        let want: Vec<_> = notices.iter().flat_map(|&k| lists_told(k, &gates, &others, owner, chain)).collect();
        assert_eq!(*heard.borrow(), want, "{what}: the listeners");
        check_paths(&ee, &t, &what);
        cleared += usize::from(t.paths.iter().map(|q| q.flags).ne(flags_before));
        for k in &notices {
            said[match k {
                RaceNotice::Checkpoint { through: true, .. } => 0,
                RaceNotice::Checkpoint { through: false, .. } => 1,
                RaceNotice::OtherLine { through: true, .. } => 2,
                _ => 3,
            }] += 1;
        }
        if rules.lap.flags & race_flags::CHECKPOINTS != 0 {
            for (k, g) in lists.checkpoints.iter().enumerate() {
                if g.gate.flags & flags::CLOSED == 0 {
                    continue;
                }
                if card.lap.lines_passed.get(k).is_some_and(|&w| w != 0) {
                    passed += 1;
                } else if !notices.iter().any(|x| matches!(x, RaceNotice::Checkpoint { line, .. } if *line == k))
                    && g.gate.crossed(now, last) != Crossing::Not
                {
                    far += 1;
                }
            }
        }
    }
    assert!(said.iter().all(|&k| k > 300) && passed > 2000 && far > 300 && cleared > 300, "{said:?} passed {passed} far {far} cleared {cleared}");
}


// --- S292a: the machine race's handler ---------------------------------------------------------------

/// Free RAM for the machine race: its manager, listener, cards (0x200 apart), their owners, chains,
/// ghost cards and records, the free records, the chains of gates and zones, the vtables and hooks.
const M_MGR: u32 = 0x01d0_0000;
const M_LISTENER: u32 = 0x01d0_1000;
const M_CARDS: u32 = 0x01d1_0000;
const M_GHOSTS: u32 = 0x01d2_0000;
const M_OWNERS: u32 = 0x01d3_0000;
const M_CHAINS: u32 = 0x01d3_1000;
const M_RECORDS: u32 = 0x01d4_0000;
const M_POOL: u32 = 0x01d5_0000;
const M_GATES: u32 = 0x01d6_0000;
const M_ZONES: u32 = 0x01d7_0000;
const M_VT: u32 = 0x01d8_0000;
const M_HOOKS: u32 = 0x01d8_1000;

/// A call a machine-race listener heard: who, its slot, the object, up to three word arguments as the
/// slot passes them (0 where it passes none), and the two vectors a `+0x20` call points at.
type MHeard = (u32, u32, u32, [u32; 3], [u32; 8]);

fn m_card(k: usize) -> u32 {
    M_CARDS + 0x200 * k as u32
}

/// Every listener slot lands on a hook that writes the call down; `ends` end the call.
fn machine_hooks(ee: &mut Ee, ends: &[u32]) -> Rc<RefCell<Vec<MHeard>>> {
    let heard: Rc<RefCell<Vec<MHeard>>> = Rc::default();
    for who in [OWNER, CHAIN, TO_LISTENER] {
        for slot in (0..0x80).step_by(4) {
            let at = M_HOOKS + who * 0x100 + slot;
            ee.set_u32(M_VT + who * 0x100 + slot, at);
            let heard = heard.clone();
            ee.hooks.insert(
                at,
                Box::new(move |ee| {
                    let r = |i: usize| ee.r(i) as u32;
                    let l = who == TO_LISTENER;
                    let mut v = [0u32; 8];
                    let args = match slot {
                        0x08 if !l => [r(5), 0, 0],
                        0x20 if !l => {
                            for i in 0..4u32 {
                                v[i as usize] = ee.u32_at(r(5) + 4 * i);
                                v[4 + i as usize] = ee.u32_at(r(6) + 4 * i);
                            }
                            [0, 0, 0]
                        }
                        0x2c | 0x30 if !l => [r(6), 0, 0],
                        0x24 | 0x28 if l => [r(6), r(7), 0],
                        0x5c | 0x64 | 0x38 if !l => [r(6), 0, 0],
                        0x6c | 0x74 | 0x30 if l => [r(6), 0, 0],
                        0x34 if !l => [r(6), r(7), r(8)],
                        0x2c if l => [r(6), r(7), r(8)],
                        _ => [0, 0, 0],
                    };
                    heard.borrow_mut().push((who, slot, r(4), args, v));
                }),
            );
        }
    }
    for &end in ends {
        ee.hooks.insert(end, Box::new(|ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));
    }
    heard
}

/// The card's own words the machine race reads and writes (no spots, no line words).
fn put_machine_card(ee: &mut Ee, k: usize, c: &RaceCard, n: usize, owner: bool, chain: bool, ghost: Option<&LapCard>) {
    let at = m_card(k);
    put_lap_card(ee, at, &c.lap, 0, M_RECORDS + 0x400 * k as u32);
    ee.set_u32(at + 0x80, c.lap.gate.map_or(0, |g| M_GATES + 0x100 * g as u32));
    ee.set_u32(at + 0x84, c.lap.gates_this_lap as u32);
    ee.set_u32(at + 0x38, c.lap.gates_passed as u32);
    put_f(ee, at + 0x20, c.odometer);
    put_quad(ee, at + 0x120, c.look);
    put_quad(ee, at + 0x180, c.to_track);
    ee.set_u32(at + 0x1c8, if k + 1 < n { m_card(k + 1) } else { 0 });
    let k = k as u32;
    let (own, ch) = (M_OWNERS + 0x100 * k, M_CHAINS + 0x100 * k);
    ee.set_u32(at + 4, if owner { own } else { 0 });
    ee.set_u32(at + 0x1c4, if chain { ch } else { 0 });
    ee.set_u32(own + 8, if ghost.is_some() { M_GHOSTS + 0x200 * k } else { 0 });
    ee.set_u32(own + 0x1c, 0);
    ee.set_u32(own + 0x20, M_VT + OWNER * 0x100);
    ee.set_u32(ch + 0x1c, 0);
    ee.set_u32(ch + 0x20, M_VT + CHAIN * 0x100);
    if let Some(g) = ghost {
        let ga = M_GHOSTS + 0x200 * k;
        put_lap_card(ee, ga, g, 0, M_RECORDS + 0x400 * k + 0x200);
        ee.set_u32(ga + 0x80, g.gate.map_or(0, |x| M_GATES + 0x100 * x as u32));
        ee.set_u32(ga + 0x84, g.gates_this_lap as u32);
        ee.set_u32(ga + 0x38, g.gates_passed as u32);
    }
}

fn machine_lap_at(ee: &Ee, at: u32) -> LapCard {
    let mut c = lap_card_at(ee, at, 0);
    let g = ee.u32_at(at + 0x80);
    c.gate = (g != 0).then(|| ((g - M_GATES) / 0x100) as usize);
    c.gates_this_lap = ee.u32_at(at + 0x84) as i32;
    c.gates_passed = ee.u32_at(at + 0x38) as i32;
    c
}

fn machine_card_at(ee: &Ee, at: u32) -> RaceCard {
    RaceCard {
        lap: machine_lap_at(ee, at),
        odometer: f32_at(ee, at + 0x20),
        look: quad(ee, at + 0x120),
        to_track: quad(ee, at + 0x180),
        ..RaceCard::default()
    }
}

/// The calls a machine notice makes for card `k` (at `at`), with what is there to hear it.
fn machine_told(n: MachineNotice, at: u32, k: u32, owner: bool, chain: bool) -> Vec<MHeard> {
    let (own, ch) = (M_OWNERS + 0x100 * k, M_CHAINS + 0x100 * k);
    let obj = |who: u32| match who {
        OWNER => own,
        CHAIN => ch,
        _ => M_LISTENER,
    };
    let fan = |o: u32, oa: [u32; 3], l: Option<(u32, [u32; 3])>, listener_first: bool| {
        let mut v = Vec::new();
        let l = l.map(|(s, a)| (TO_LISTENER, s, M_LISTENER, a, [0; 8]));
        if listener_first {
            v.extend(l);
        }
        if owner {
            v.push((OWNER, o, own, oa, [0; 8]));
        }
        if chain {
            v.push((CHAIN, o, ch, oa, [0; 8]));
        }
        if !listener_first {
            v.extend(l);
        }
        v
    };
    match n {
        MachineNotice::Look => fan(8, [at, 0, 0], None, false),
        MachineNotice::Line(n) => told_gate(n, owner, chain).into_iter().map(|(w, s)| (w, s, obj(w), [0; 3], [0; 8])).collect(),
        MachineNotice::Zone { zone, crossing } => {
            let z = M_ZONES + 0x100 * zone as u32;
            let (o, l) = if crossing == Crossing::Through { (0x2c, 0x24) } else { (0x30, 0x28) };
            fan(o, [z, 0, 0], Some((l, [at, z, 0])), false)
        }
        MachineNotice::Aimed => fan(0x5c, [at, 0, 0], Some((0x6c, [at, 0, 0])), true),
        MachineNotice::Aim { point, ahead } => {
            let mut v = [0u32; 8];
            v[..4].copy_from_slice(&point.to_bits());
            v[4..].copy_from_slice(&ahead.to_bits());
            let mut out = Vec::new();
            if owner {
                out.push((OWNER, 0x20, own, [0; 3], v));
            }
            if chain {
                out.push((CHAIN, 0x20, ch, [0; 3], v));
            }
            out
        }
        MachineNotice::TimeUp => fan(0x64, [at, 0, 0], Some((0x74, [at, 0, 0])), false),
    }
}

/// The calls a finish notice makes, as `finish_told` gives them for the race handler.
fn machine_finish_told(n: FinishNotice, at: &[u32], owner: &[bool], chain: &[bool]) -> Vec<MHeard> {
    let other = |k: Option<usize>| k.map_or(0, |k| at[k]);
    let (card, o_slot, l_slot, args) = match n {
        FinishNotice::Over { card } => (card, 0x1c, None, [0, 0, 0]),
        FinishNotice::TimeUp { card } => (card, 0x64, Some(0x74), [at[card], 0, 0]),
        FinishNotice::PlaceLost { card, ahead } => (card, 0x34, Some(0x2c), [at[card], other(ahead), 1]),
        FinishNotice::PlaceGained { card, behind } => (card, 0x34, Some(0x2c), [at[card], other(behind), 0]),
        FinishNotice::Lead { card } => (card, 0x38, Some(0x30), [at[card], 0, 0]),
    };
    let id = (at[card] - M_CARDS) / 0x200;
    let mut v = Vec::new();
    if owner[id as usize] {
        v.push((OWNER, o_slot, M_OWNERS + 0x100 * id, args, [0; 8]));
    }
    if chain[id as usize] {
        v.push((CHAIN, o_slot, M_CHAINS + 0x100 * id, args, [0; 8]));
    }
    if let Some(l) = l_slot {
        v.push((TO_LISTENER, l, M_LISTENER, args, [0; 8]));
    }
    v
}

/// A machine-race card: a lap card with every flag the handler reads at its own odds (never the
/// unported helper's `0x8_0000`), a gate to wait at, a step about it, and the handler's own words.
fn machine_card(p: &mut Pick, k: usize, gates: &[Gate], rules: &LapRules) -> RaceCard {
    let mut lap = lap_card(p, 0);
    lap.flags = finish_flags(p, lap.flags) & !0x8_0000;
    if p.below(3) == 0 {
        lap.flags |= card_flags::TAKING_PART;
        lap.flags &= !(card_flags::OUT | card_flags::TIMED_OUT);
    }
    lap.record_word = k as u32;
    let n = gates.len();
    let at = if p.below(2) == 0 { n - 1 } else { p.below(n as u64) as usize };
    lap.gate = (p.below(15) != 0).then_some(at);
    lap.gates_this_lap = p.below(5) as i32;
    lap.gates_passed = p.below(4) as i32;
    let (from, to) = step(p, &gates[at]);
    lap.seen = Seen { now: from, last: to };
    // Riders far from their gate too, past the 50 ft the aim leads by.
    if p.below(4) == 0 {
        let (x, y, z) = (p.range(-200.0, 200.0), p.range(-20.0, 20.0), p.range(-200.0, 200.0));
        lap.seen.now = p.quad([x, y, z]);
        let (dx, dz) = (p.range(-5.0, 5.0), p.range(-5.0, 5.0));
        lap.seen.last = p.quad([x + dx, y, z + dz]);
    }
    // A card waiting at no gate whose gate line is skipped reads low memory in the original.
    if lap.gate.is_none() && !(lap.seen.last.x() >= 0.0) {
        lap.seen.last.0[0] = p.range(0.0, 50.0);
    }
    if p.below(8) == 0 {
        lap.best_lap = F(fpu::sub(lap.race_time.0, lap.lap_start.0));
    }
    if p.below(6) == 0 {
        lap.laps = rules.laps - 1;
    }
    if p.below(4) == 0 {
        lap.time_left = F(p.range(0.0, 0.06));
    }
    let lap_no = if rules.kind == race_kind::GHOST_RACE { lap.runs + 1 } else { lap.laps + 1 };
    if lap_no >= rules.record_limit && lap.records.is_empty() {
        lap.records.push(record(p));
    }
    let odometer = time_word(p, 9000.0);
    let (lx, lz, tx, tz) = (p.range(-1.0, 1.0), p.range(-1.0, 1.0), p.range(-1.0, 1.0), p.range(-1.0, 1.0));
    RaceCard {
        lap,
        odometer,
        look: p.quad([lx, 0.0, lz]),
        to_track: p.quad([tx, 0.0, tz]),
        ..RaceCard::default()
    }
}

fn machine_rules(p: &mut Pick) -> LapRules {
    let mut rules = finish_rules(p);
    for (bit, odds) in [(race_flags::ZONES, 2), (race_flags::KEEP_LAPS, 2)] {
        if p.below(odds) == 0 {
            rules.flags |= bit;
        } else {
            rules.flags &= !bit;
        }
    }
    if p.below(3) == 0 {
        rules.kind = race_kind::GHOST_RACE;
    }
    rules.laps = 1 + p.below(5) as i32;
    rules.ghost_runs = p.below(5) as i32;
    rules.record_limit = p.below(12) as i32;
    rules.free_records = p.below(6) as u32;
    rules
}

/// The machine race's whole handler on 20,000 generated lists of one to five cards, chains of one to
/// four gates and zone lists, against [`LapRules::machine_tick`]: every listener call in order with its
/// card, zone, other card and the two vectors it is pointed at, the handler's answer, the list's order,
/// every word of every card and ghost card, the manager's flags, the countdown, the free records and
/// the generator. The manager holds no track, so the path steering (unported) is never taken.
#[test]
fn the_machine_race_matches_the_score_managers_mode_1_handler() {
    let Some(mut ee) = game_or_skip() else { return };
    let [handler, rand_state] = match addrs(["score_machine_race_tick", "rand_state"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard = machine_hooks(&mut ee, &[]);
    let mut p = Pick(0x3a7c_15e2_90bd_4f61);
    let mut said = std::collections::BTreeMap::<String, usize>::new();
    let (mut stopped, mut near, mut far, mut made_up) = (0, 0, 0, 0);
    for case in 0..20_000 {
        let mut rules = machine_rules(&mut p);
        let n = 1 + p.below(5) as usize;
        let gates: Vec<Gate> = (0..1 + p.below(4) as usize).map(|_| finish_line(&mut p)).collect();
        let (mut zones, _) = zones_about(&mut p);
        if p.below(8) == 0 {
            zones.clear();
        }
        let mut cards: Vec<RaceCard> = (0..n).map(|k| machine_card(&mut p, k, &gates, &rules)).collect();
        // Zones the riders' steps cross, now and then.
        if !zones.is_empty() && p.below(2) == 0 {
            let k = p.below(n as u64) as usize;
            let z = p.below(zones.len() as u64) as usize;
            let (from, to) = step(&mut p, &zones[z]);
            cards[k].lap.seen = Seen { now: from, last: to };
            if cards[k].lap.gate.is_none() && !(to.x() >= 0.0) {
                cards[k].lap.gate = Some(0);
            }
        }
        let owner: Vec<bool> = (0..n).map(|_| p.below(4) != 0).collect();
        let chain: Vec<bool> = (0..n).map(|_| p.below(3) == 0).collect();
        let mut ghosts: Vec<Option<LapCard>> = (0..n)
            .map(|k| {
                (owner[k] && p.below(4) == 0).then(|| {
                    let mut g = lap_card(&mut p, 0);
                    g.gate = (p.below(3) != 0).then(|| p.below(gates.len() as u64) as usize);
                    g.gates_this_lap = p.below(5) as i32;
                    g.gates_passed = p.below(20) as i32;
                    if p.below(3) == 0 {
                        g.best_lap = cards[k].lap.best_lap;
                    }
                    g
                })
            })
            .collect();

        put_chain(&mut ee, M_GATES, &gates);
        put_chain(&mut ee, M_ZONES, &zones);
        ee.set_u32(M_MGR + 0x30, M_LISTENER);
        ee.set_u32(M_MGR + 0x34, M_CARDS);
        ee.set_u32(M_MGR + 0x4c, rules.record_limit as u32);
        ee.set_u32(M_MGR + 0x5c, rules.flags);
        ee.set_u32(M_MGR + 0x60, rules.kind as u32);
        ee.set_u32(M_MGR + 0x68, 0);
        ee.set_u32(M_MGR + 0x70, M_GATES);
        ee.set_u32(M_MGR + 0x78, if zones.is_empty() { 0 } else { M_ZONES });
        ee.set_u32(M_MGR + 0x94, rules.laps as u32);
        put_f(&mut ee, M_MGR + 0x98, rules.finish_clock);
        put_f(&mut ee, M_MGR + 0x9c, rules.finish_wait);
        put_f(&mut ee, M_MGR + 0xa0, rules.time_limit);
        ee.set_u32(M_MGR + 0xf0, rules.ghost_runs as u32);
        for k in 0..rules.free_records {
            let next = if k + 1 < rules.free_records { M_POOL + 0x20 * (k + 1) } else { 0 };
            put_record(&mut ee, M_POOL + 0x20 * k, &record(&mut p), next);
        }
        ee.set_u32(M_MGR + 0x58, if rules.free_records == 0 { 0 } else { M_POOL });
        for (o, w) in [(4, 0), (0x10, 0), (0x14, 0), (0x18, 0), (0x1c, M_VT + TO_LISTENER * 0x100)] {
            ee.set_u32(M_LISTENER + o, w);
        }
        for k in 0..n {
            put_machine_card(&mut ee, k, &cards[k], n, owner[k], chain[k], ghosts[k].as_ref());
        }
        let state = p.next();
        ee.write(rand_state, 8, u128::from(state));
        let dt = if p.below(10) == 0 { F(p.stray(0.0)) } else { F(p.range(0.0, 0.05)) };
        heard.borrow_mut().clear();
        ee.call(handler, &[u64::from(M_MGR)], &[dt.0], 2_000_000);
        let answer = ee.v0() as u32;

        let was = rules;
        let mut rng = Rng::from_state(state);
        let tick = rules.machine_tick(dt, &mut cards, &mut ghosts, &gates, &zones, &mut rng);
        let what = format!("case {case}: {was:?}, {n} cards, {} gates", gates.len());

        let mut want: Vec<MHeard> = Vec::new();
        for &(k, m) in &tick.said {
            want.extend(machine_told(m, m_card(k), k as u32, owner[k], chain[k]));
            let name = format!("{m:?}");
            *said.entry(name.split([' ', '(', '{']).next().unwrap().to_string()).or_default() += 1;
            if let MachineNotice::Line(l) = m {
                *said.entry(format!("{l:?}")).or_default() += 1;
            }
        }
        let before: Vec<u32> = (0..n).map(m_card).collect();
        let after: Vec<u32> = cards.iter().map(|c| m_card(c.lap.record_word as usize)).collect();
        for (k, &m) in tick.finish.iter().enumerate() {
            let at = if matches!(m, FinishNotice::Over { .. }) { &before } else { &after };
            want.extend(machine_finish_told(m, at, &owner, &chain));
            if matches!(m, FinishNotice::Over { .. }) && !matches!(tick.finish.get(k + 1), Some(FinishNotice::Over { .. })) {
                want.push((TO_LISTENER, 0x18, M_LISTENER, [0; 3], [0; 8]));
            }
            let name = format!("{m:?}");
            *said.entry(name.split([' ', '{']).next().unwrap().to_string()).or_default() += 1;
        }
        assert_eq!(*heard.borrow(), want, "{what}: the listeners ({tick:?})");
        assert_eq!(answer, u32::from(tick.went_on), "{what}: the answer");
        let mut list = Vec::new();
        let mut at = ee.u32_at(M_MGR + 0x34);
        while at != 0 && list.len() < 8 {
            list.push(at);
            at = ee.u32_at(at + 0x1c8);
        }
        assert_eq!(list, after, "{what}: the list's order");
        for (c, &a) in cards.iter().zip(&after) {
            assert_eq!(race_card_words(&machine_card_at(&ee, a)), race_card_words(c), "{what}: card {}", c.lap.record_word);
        }
        for (k, g) in ghosts.iter().enumerate() {
            if let Some(g) = g {
                let ga = M_GHOSTS + 0x200 * k as u32;
                assert_eq!(lap_card_words(&machine_lap_at(&ee, ga)), lap_card_words(g), "{what}: ghost {k}");
            }
        }
        assert_eq!(ee.u32_at(M_MGR + 0x5c), rules.flags, "{what}: the manager's flags");
        assert_eq!(f32_at(&ee, M_MGR + 0x98).0.to_bits(), rules.finish_clock.0.to_bits(), "{what}: the countdown");
        assert_eq!(pool_len(&ee, ee.u32_at(M_MGR + 0x58), 0x14), rules.free_records as usize, "{what}: the free records");
        assert_eq!(ee.read(rand_state, 8) as u64, rng.state(), "{what}: the generator");

        stopped += usize::from(!tick.went_on);
        for (k, c) in cards.iter().enumerate() {
            let _ = k;
            if c.lap.gate.is_some() {
                if c.lap.to_line_kept.0 <= 50.0 { near += 1 } else { far += 1 }
            }
        }
        made_up += usize::from(was.flags & race_flags::OVER != 0 && tick.went_on);
    }
    for key in ["Look", "Line", "Zone", "Aimed", "Aim", "TimeUp", "Over", "PlaceLost", "PlaceGained", "Lead", "Gate", "Lap", "BestLap", "Finished", "RunOver", "GhostReady"] {
        let c = said.get(key).copied().unwrap_or(0);
        // A ghost run's end needs a ghost race on its last lap with a lap closed: rarer.
        let floor = if matches!(key, "RunOver" | "GhostReady") { 40 } else { 100 };
        assert!(c > floor, "{key} came up only {c} times: {said:?}");
    }
    assert!(stopped > 200 && near > 2000 && far > 2000 && made_up > 1000, "stopped {stopped} near {near} far {far} made up {made_up}: {said:?}");
}

/// The gate race's ranking on its own, 20,000 lists of one to six cards with ties in gates passed and
/// in distance to the next gate, against [`LapRules::gate_rank`] on the list's order, the places and
/// every listener call.
#[test]
fn the_gate_ranking_matches_the_score_managers() {
    let Some(mut ee) = game_or_skip() else { return };
    let rank = match addr("score_gate_rank") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let heard = machine_hooks(&mut ee, &[]);
    let mut p = Pick(0x6d02_b8e7_1c49_a355);
    let mut said = [0usize; 3];
    for case in 0..20_000 {
        let n = 1 + p.below(6) as usize;
        let mut cards: Vec<LapCard> = (0..n)
            .map(|k| {
                let mut c = finish_card(&mut p, k as u32);
                c.gates_passed = p.below(4) as i32 - i32::from(p.below(20) == 0);
                c
            })
            .collect();
        for k in 1..n {
            if p.below(4) == 0 {
                cards[k].to_line = cards[k - 1].to_line;
            }
            if p.below(3) == 0 {
                cards[k].place = k as i32 + 1;
            }
        }
        let owner: Vec<bool> = (0..n).map(|_| p.below(4) != 0).collect();
        let chain: Vec<bool> = (0..n).map(|_| p.below(3) == 0).collect();
        ee.set_u32(M_MGR + 0x30, M_LISTENER);
        ee.set_u32(M_MGR + 0x34, M_CARDS);
        for (o, w) in [(4, 0), (0x10, 0), (0x14, 0), (0x18, 0), (0x1c, M_VT + TO_LISTENER * 0x100)] {
            ee.set_u32(M_LISTENER + o, w);
        }
        for (k, c) in cards.iter().enumerate() {
            let rc = RaceCard { lap: c.clone(), ..RaceCard::default() };
            put_machine_card(&mut ee, k, &rc, n, owner[k], chain[k], None);
        }
        heard.borrow_mut().clear();
        ee.call(rank, &[u64::from(M_MGR)], &[], 1_000_000);
        assert_eq!(ee.v0(), 1, "case {case}: the answer");

        let notices = LapRules::gate_rank(&mut cards);
        let after: Vec<u32> = cards.iter().map(|c| m_card(c.record_word as usize)).collect();
        let want: Vec<MHeard> = notices.iter().flat_map(|&m| machine_finish_told(m, &after, &owner, &chain)).collect();
        for m in &notices {
            said[match m {
                FinishNotice::PlaceLost { .. } => 0,
                FinishNotice::PlaceGained { .. } => 1,
                _ => 2,
            }] += 1;
        }
        let mut list = Vec::new();
        let mut at = ee.u32_at(M_MGR + 0x34);
        while at != 0 && list.len() < 8 {
            list.push(at);
            at = ee.u32_at(at + 0x1c8);
        }
        assert_eq!(list, after, "case {case}: the list's order");
        for (c, &a) in cards.iter().zip(&after) {
            assert_eq!(lap_card_words(&machine_lap_at(&ee, a)), lap_card_words(c), "case {case}: card {}", c.record_word);
        }
        assert_eq!(*heard.borrow(), want, "case {case}: the listeners");
    }
    assert!(said.iter().all(|&k| k > 1000), "{said:?}");
}

// --- the race's results (S13d) ------------------------------------------------------------------------

const FAME: u32 = 0x0190_0000;

/// A kept Hall of Fame word near `time`: a time a little either side, the time itself, a whole second,
/// a count (for the counts table), or a stray word.
fn fame_word(p: &mut Pick, time: f32) -> u32 {
    match p.below(9) {
        0 => time.to_bits(),
        1 => time.trunc().to_bits(),
        2 => (time.trunc() + 1.0).to_bits(),
        3 => (time as i32).wrapping_add(p.below(3) as i32 - 1) as u32,
        4 => 0,
        5 => p.stray(time).to_bits(),
        _ => p.range(time * 0.8, time * 1.2).to_bits(),
    }
}

fn fame_time(p: &mut Pick) -> F {
    let x = match p.below(10) {
        0 => 0.0,
        1 => p.below(400) as f32,
        2 => -p.range(0.0, 3.0),
        3 => p.range(0.0, 1.0),
        _ => p.range(20.0, 600.0),
    };
    F(p.stray(x))
}

#[test]
fn the_hall_of_fame_test_matches_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    let [beats] = match addrs(["hall_of_fame_beats"]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut fame = race::HallOfFame::new();
    for (k, w) in fame.words.iter().enumerate() {
        ee.set_u32(FAME + 4 * k as u32, *w);
    }
    let mut p = Pick(0x7a3e_1d55_c0b2_9f41);
    let mut answers = [[0u32; 2]; 11];
    let mut cases = 0;
    while cases < 20_000 {
        let table = p.below(13) as u32;
        let track = p.below(30) as i32 - 2;
        let class = p.below(7) as i32 - 1;
        let time = fame_time(&mut p);
        if let Some(at) = race::HallOfFame::offset(table, track, class) {
            for i in 0..5 {
                let w = fame_word(&mut p, time.0);
                fame.words[at / 4 + i] = w;
                ee.set_u32(FAME + at as u32 + 4 * i as u32, w);
            }
        } else if table <= 10 {
            continue; // the original reads past its tables
        }
        ee.call(beats, &[u64::from(FAME), u64::from(table), track as i64 as u64, class as i64 as u64], &[time.0], BUDGET);
        let want = ee.r(2) as u32;
        let got = fame.beats(table, track, class, time);
        assert_eq!(want, u32::from(got), "case {cases}: table {table} track {track} class {class} time {time:?}");
        if table <= 10 {
            answers[table as usize][want as usize] += 1;
        }
        cases += 1;
    }
    assert!(answers.iter().all(|a| a[0] > 150 && a[1] > 150), "per table (no, yes): {answers:?}");
}

const R_APP: u32 = 0x0191_0000;
const R_FE: u32 = R_APP + 0x1000;
const R_EVENT: u32 = R_APP + 0x2000;
const R_LISTS: u32 = R_APP + 0x3000;
const R_STATS: u32 = R_APP + 0x4000;
const R_PROFILES: u32 = R_APP + 0x5000;
const R_ACT: u32 = 0x0192_0000;
const R_WORLD: u32 = 0x0193_0000;
const R_FLASH: u32 = 0x0193_1000;
const R_MENU: u32 = 0x0193_2000;
const R_CARD: u32 = 0x0194_0000;
const R_ITEMS: u32 = 0x0195_0000;
const R_ITEM_VT: u32 = 0x0196_0000;
const R_CAST: u32 = 0x0197_0000;
const R_TRACK: u32 = 0x0198_0000;
/// Not code: the items' cast slot lands here and is hooked.
const R_HOOK_CAST: u32 = 0x0196_1000;

#[derive(Debug, PartialEq)]
enum OverCall {
    Fireworks(u32, u32),
    Stat(u32, u32, u32, u32),
    Fame(u32, u32, u32),
}

#[test]
fn the_race_over_matches_the_supercross_activitys_answer() {
    let Some(mut ee) = game_or_skip() else { return };
    let [over, app, id, nth, list, find, fireworks, stat, mark] = match addrs([
        "race_over_card",
        "app_pointer",
        "interface_id",
        "list_nth",
        "results_track_list",
        "results_track_find",
        "race_fireworks",
        "profile_stat_add",
        "fame_menu_mark",
    ]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let calls: Rc<RefCell<Vec<OverCall>>> = Rc::default();
    let found = Rc::new(Cell::new(0u32));
    ee.hooks.insert(id, Box::new(|_| {}));
    ee.hooks.insert(nth, Box::new(|ee| ee.set(2, u64::from(R_ITEMS + ee.r(5) as u32 * 0x1000))));
    ee.hooks.insert(R_HOOK_CAST, Box::new(|ee| {
        let k = (ee.r(4) as u32 - R_ITEMS) / 0x1000;
        ee.set(2, u64::from(R_CAST + k * 0x100 + 0x24));
    }));
    ee.hooks.insert(list, Box::new(|ee| ee.set(2, u64::from(R_LISTS + 0x100))));
    let f = found.clone();
    ee.hooks.insert(find, Box::new(move |ee| ee.set(2, u64::from(f.get()))));
    let c = calls.clone();
    ee.hooks.insert(fireworks, Box::new(move |ee| c.borrow_mut().push(OverCall::Fireworks(ee.r(4) as u32, ee.r(5) as u32))));
    let c = calls.clone();
    ee.hooks.insert(stat, Box::new(move |ee| {
        c.borrow_mut().push(OverCall::Stat(ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32, ee.f(12).to_bits()))
    }));
    let c = calls.clone();
    ee.hooks.insert(mark, Box::new(move |ee| c.borrow_mut().push(OverCall::Fame(ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32))));

    ee.set_u32(app, R_APP);
    ee.set_u32(R_APP + 0x3d4, R_FE);
    ee.set_u32(R_FE + 0xc, R_EVENT);
    ee.set_u32(R_APP + 0x3d8, R_LISTS);
    ee.set_u32(R_APP + 0x3e0, FAME);
    ee.set_u32(R_APP + 0x3e4, R_STATS);
    ee.set_u32(R_ACT + 0x180, R_WORLD);
    ee.set_u32(R_ACT + 0x3b8, R_MENU);
    ee.set_u32(R_ITEM_VT + 0xa4, R_HOOK_CAST);
    // Below the tables, where a track the list does not hold (-1) reads: words no time beats.
    for k in 0..0x14 {
        ee.set_u32(FAME - 0x50 + 4 * k, 0xff7f_ffff);
    }
    let mut fame = race::HallOfFame::new();
    let act = Supercross::new();
    let mut p = Pick(0x2b94_d1e0_5c37_8a16);
    let (mut fired, mut wins, mut marks, mut flashed) = (0, 0, 0, 0);
    for case in 0..20_000 {
        calls.borrow_mut().clear();
        let event = race::ResultEvent {
            league: [0, 0, 1, 2][p.below(4) as usize],
            activity: if p.below(4) == 0 { [0, 1, 5][p.below(3) as usize] } else { 2 },
            class: p.below(4) as i32,
            laps: [3, 5, 10, 1, 2, 4, 3, 5, 10][p.below(9) as usize],
            mode: i32::from(p.below(3) == 0),
            alt_outro: false,
        };
        ee.set_u32(R_FE + 4, event.mode as u32);
        ee.set_u32(R_EVENT, event.league as u32);
        ee.set_u32(R_EVENT + 4, event.activity as u32);
        ee.set_u32(R_EVENT + 0x10c, event.class as u32);
        ee.write(R_EVENT + 0x115, 1, u128::from(event.laps));
        let track = if p.below(6) == 0 { -1 } else { p.below(26) as i32 };
        found.set(if track < 0 { 0 } else { R_TRACK });
        ee.set_u32(R_TRACK + 4, track as u32);

        let mut card = LapCard {
            flags: p.next() & !0x4 | if p.below(3) != 0 { 0x4 } else { 0 },
            place: p.below(7) as i32 - 1,
            best_lap: fame_time(&mut p),
            race_time: fame_time(&mut p),
            ..LapCard::default()
        };
        if p.below(4) == 0 {
            card.place = 1;
        }
        ee.write(R_CARD + 8, 8, u128::from(card.flags));
        ee.set_u32(R_CARD + 0x2c, card.place as u32);
        put_f(&mut ee, R_CARD + 0x50, card.best_lap);
        put_f(&mut ee, R_CARD + 0xb4, card.race_time);

        // One to three interface items, the card's among them (or not: the last is taken).
        let items = 1 + p.below(3) as u32;
        let mine = p.below(u64::from(items) + 1) as u32;
        ee.set_u32(R_ACT + 0x14, items);
        let mut seats = Vec::new();
        for k in 0..items {
            let item = R_ITEMS + k * 0x1000;
            ee.set_u32(item + 8, R_ITEM_VT);
            let seat = p.below(4) as u32;
            ee.set_u32(item + 0x6b0, seat);
            ee.set_u32(R_CAST + k * 0x100, if k == mine { R_CARD } else { R_CARD + 0x1e0 });
            seats.push(seat);
        }
        let taken = mine.min(items - 1);
        let seat_no = seats[taken as usize];
        let keeps = p.below(3) != 0;
        let profile = p.below(3) != 0;
        ee.set_u32(R_ACT + 0x4d0 + 4 * seat_no, u32::from(keeps));
        ee.set_u32(R_APP + 0x3f4 + 4 * seat_no, if profile { R_PROFILES + seat_no * 0x100 } else { 0 });
        ee.set_u32(R_PROFILES + seat_no * 0x100 + 0xcc, 7 + seat_no);
        let seat = race::ResultSeat { seat: seat_no as i32, keeps_records: keeps, profile };

        let levels = race::FlashLevels { shortest: time_word(&mut p, 4.0), player_win: time_word(&mut p, 2.0), podium: time_word(&mut p, 2.0) };
        put_f(&mut ee, R_WORLD + 0x538, levels.shortest);
        put_f(&mut ee, R_WORLD + 0x53c, levels.player_win);
        put_f(&mut ee, R_WORLD + 0x544, levels.podium);
        let mut flashes = (p.below(3) != 0).then(|| race::PodiumFlashes {
            from: time_word(&mut p, 1.0),
            length: time_word(&mut p, 4.0),
            rate: time_word(&mut p, 4.0),
            half: time_word(&mut p, 4.0),
        });
        ee.set_u32(R_ACT + 0x414, if flashes.is_some() { R_FLASH } else { 0 });
        if let Some(fl) = &flashes {
            for (o, x) in [(0x5c, fl.from), (0x64, fl.length), (0x68, fl.rate), (0x6c, fl.half)] {
                put_f(&mut ee, R_FLASH + o, x);
            }
        }

        // The entries the card's two times are tried against, near those times.
        let base = if event.league != 0 { 4 } else { 0 };
        for (table, time) in [(base, card.best_lap), (base + 1, card.race_time), (base + 2, card.race_time), (base + 3, card.race_time)] {
            if let Some(at) = race::HallOfFame::offset(table, track, event.class) {
                for i in 0..5 {
                    let w = fame_word(&mut p, time.0);
                    fame.words[at / 4 + i] = w;
                    ee.set_u32(FAME + at as u32 + 4 * i as u32, w);
                }
            }
        }

        ee.call(over, &[u64::from(R_ACT), 0, u64::from(R_CARD)], &[], BUDGET);
        let got = act.race_over(&card, Some(seat), &event, track, &fame, flashes.as_mut(), &levels);

        let mut want = Vec::new();
        if got.fireworks {
            want.push(OverCall::Fireworks(R_ACT, R_ITEMS + taken * 0x1000));
        }
        if let Some(s) = got.win {
            want.push(OverCall::Stat(R_STATS, 10, 7 + s as u32, 1.0f32.to_bits()));
        }
        for e in &got.fame {
            want.push(OverCall::Fame(R_MENU, e.seat as u32, e.track as u32));
        }
        assert_eq!(*calls.borrow(), want, "case {case}: {event:?} {card:?} seat {seat:?} track {track}");
        if let Some(fl) = &flashes {
            let at = |o| ee.u32_at(R_FLASH + o);
            let port = [fl.from, fl.length, fl.rate, fl.half].map(|x| x.0.to_bits());
            assert_eq!([at(0x5c), at(0x64), at(0x68), at(0x6c)], port, "case {case}: flashes");
        }
        fired += usize::from(got.fireworks);
        wins += usize::from(got.win.is_some());
        marks += got.fame.len();
        flashed += usize::from(flashes.is_some() && event.activity == 2 && card.place < 4);
    }
    assert!(fired > 2000 && wins > 500 && marks > 4000 && flashed > 4000, "fireworks {fired}, wins {wins}, entries {marks}, flashes {flashed}");
}

// --- S13d: the results held against a live sx01 finish ------------------------------------------------

/// One string or number field of a capture's JSON line (the capture's lines are flat).
fn s13d_field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let at = line.find(&format!("\"{key}\": "))? + key.len() + 4;
    let rest = &line[at..];
    if let Some(s) = rest.strip_prefix('"') {
        return s.split('"').next();
    }
    rest.split([',', '}']).next()
}

/// A score card's results words off a captured row's raw bytes, its id (`+0`) kept in
/// `record_word` so it can be followed through the ranking.
fn s13d_card(raw: &[u8]) -> LapCard {
    let u = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
    let f = |o: usize| F(f32::from_bits(u(o)));
    LapCard {
        flags: u64::from(u(8)) | u64::from(u(12)) << 32,
        progress: f(0x1c),
        place: u(0x2c) as i32,
        laps: u(0x30) as i32,
        lap_time: f(0x48),
        lap_start: f(0x4c),
        best_lap: f(0x50),
        race_time: f(0xb4),
        lap_clock: f(0xb8),
        gap: f(0xbc),
        record_word: u(0),
        ..LapCard::default()
    }
}

struct S13dRow {
    frame: u32,
    dt: F,
    rand: u64,
    mgr: Vec<u8>,
    cards: Vec<Vec<u8>>,
    flash: Vec<u8>,
    act: Vec<u8>,
}

impl S13dRow {
    /// The card ids in the manager's list order: from its head (`+0x34`) each card's next (`+0x1c8`),
    /// `cards` the cards' addresses by id.
    fn order(&self, cards: &[u32]) -> Vec<usize> {
        let word = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let mut list = Vec::new();
        let mut at = word(&self.mgr, 0x34);
        while let Some(k) = cards.iter().position(|&a| a == at && list.len() < cards.len()) {
            list.push(k);
            at = word(&self.cards[k], 0x1c8);
        }
        list
    }

    fn flash_f(&self, o: usize) -> F {
        F(f32::from_bits(u32::from_le_bytes(self.flash[o..o + 4].try_into().unwrap())))
    }
}

/// S13d's live sx01 single race of one lap (`context/capture/s13d/sx01-finish.jsonl`,
/// research/s13d/finish.py): four riders, the human idle at the gate and marked done by a poke once an
/// AI rider has finished, so the race ends the next tick. Held: each AI rider's finish at the line
/// through [`Supercross::race_over`] (the podium flashes' half and rate, bit for bit, and their length
/// one tick on); the over tick leaving the flashes alone (a made-up finish is never answered); and the
/// over tick itself through [`LapRules::check_over`], [`LapRules::clocks`] and [`LapRules::close`] —
/// the made-up finish of the rider still out, the tie draw pushing the human's time past it, found on
/// the generator a few draws on from the tick before's, and the ranking — against every card's
/// results words, the list order and the manager's flags.
#[test]
fn the_results_hold_s13ds_live_sx01_finish() {
    let Ok(kit) = Kit::load() else { return skip("no kit.toml") };
    let Ok(text) = std::fs::read_to_string(kit.ctx("capture/s13d/sx01-finish.jsonl")) else {
        return skip("capture/s13d/sx01-finish.jsonl is not there: research/s13d/run.sh records it");
    };
    let hex32 = |h: &str| u32::from_le_bytes(hex_bytes(h)[..4].try_into().unwrap());
    let mut rows = Vec::new();
    let (mut before, mut world) = (None, None);
    for line in text.lines() {
        match s13d_field(line, "tag") {
            Some("before") => before = Some(line),
            Some("world") => world = s13d_field(line, "world").map(hex_bytes),
            Some(_) => {}
            None if line.contains("\"poke\"") => {}
            None => rows.push(S13dRow {
                frame: s13d_field(line, "frame").unwrap().parse().unwrap(),
                dt: F(f32::from_bits(hex32(s13d_field(line, "dt").unwrap()))),
                rand: u64::from_le_bytes(hex_bytes(s13d_field(line, "rand").unwrap())[..8].try_into().unwrap()),
                mgr: hex_bytes(s13d_field(line, "mgr").unwrap()),
                cards: (0..4).map(|k| hex_bytes(s13d_field(line, &format!("card{k}")).unwrap())).collect(),
                flash: s13d_field(line, "flash").map(hex_bytes).unwrap_or_default(),
                act: hex_bytes(s13d_field(line, "act").unwrap()),
            }),
        }
    }
    let (Some(before), Some(world)) = (before, world) else { return skip("the capture has no before or world line") };
    let list = before.split("\"cards\": [").nth(1).and_then(|r| r.split(']').next()).expect("the cards' addresses");
    let addresses: Vec<u32> = list.split(", ").map(|a| a.parse().unwrap()).collect();
    let event_raw = hex_bytes(s13d_field(before, "event").unwrap());
    let fe = hex_bytes(s13d_field(before, "fe").unwrap());
    let w = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let event = race::ResultEvent { league: w(&event_raw, 0) as i32, activity: w(&event_raw, 4) as i32, class: w(&event_raw, 0x10c) as i32, laps: event_raw[0x115], mode: w(&fe, 4) as i32, alt_outro: event_raw[0x116] != 0 };
    let wf = |o: usize| F(f32::from_bits(w(&world, o - 0x530)));
    let levels = race::FlashLevels { shortest: wf(0x538), player_win: wf(0x53c), podium: wf(0x544) };
    assert_eq!((event.activity, event.laps), (2, 1), "a single race of one lap: {event:?}");
    let act = Supercross::new();
    let out = |c: &LapCard| c.flags & card_flags::OUT != 0;

    // Each finish at the line (a card goes out on a tick the race is not over), the tick before
    // there to read the flashes as they stood.
    let (mut line_finishes, mut fireworks, mut over_ticks) = (0, 0, 0);
    for p in rows.windows(2) {
        let (was, now) = (&p[0], &p[1]);
        if now.frame != was.frame + 1 || was.flash.is_empty() || now.flash.is_empty() {
            continue;
        }
        let over_now = w(&now.mgr, 0x5c) & race_flags::OVER != 0;
        let over_was = w(&was.mgr, 0x5c) & race_flags::OVER != 0;
        let words = |r: &S13dRow| [0x5c, 0x64, 0x68, 0x6c].map(|o| r.flash_f(o).0.to_bits());
        if over_now && !over_was {
            // The race's end: the half and rate stay as they were (the flashes only run on).
            over_ticks += 1;
            assert_eq!(words(now)[2..], words(was)[2..], "frame {}: the over tick set the flashes", now.frame);
            continue;
        }
        for k in 0..4 {
            let (a, b) = (s13d_card(&was.cards[k]), s13d_card(&now.cards[k]));
            if out(&a) || !out(&b) || over_now {
                continue;
            }
            let mut fl = race::PodiumFlashes { from: was.flash_f(0x5c), length: was.flash_f(0x64), rate: was.flash_f(0x68), half: was.flash_f(0x6c) };
            let said = act.race_over(&b, None, &event, -1, &race::HallOfFame::new(), Some(&mut fl), &levels);
            eprintln!("frame {}: card {k} finished at the line in place {}: {said:?}, flashes {fl:?}", now.frame, b.place);
            assert_eq!(said.fireworks, b.place == 1, "frame {}: card {k}", now.frame);
            assert!(said.win.is_none() && said.fame.is_empty(), "frame {}: an AI card has no seat", now.frame);
            fireworks += usize::from(said.fireworks);
            if b.place < 4 {
                // The flashes then run one tick: their length counts down.
                let got = [fl.from.0.to_bits(), fpu::sub(fl.length.0, now.dt.0).to_bits(), fl.rate.0.to_bits(), fl.half.0.to_bits()];
                assert_eq!(got[1..], words(now)[1..], "frame {}: card {k}'s flashes (from, length, rate, half)", now.frame);
            }
            line_finishes += 1;
        }
    }
    eprintln!("{line_finishes} finishes at the line held, {fireworks} with fireworks, {over_ticks} over ticks");
    assert!(line_finishes >= 1 && fireworks == 1 && over_ticks == 1, "the winner's finish at the line and the race's end");

    // The over tick: the tick before's cards in list order, through the handler's end.
    let at = rows.iter().position(|r| w(&r.mgr, 0x5c) & race_flags::OVER != 0).expect("the race ends");
    let (was, now) = (&rows[at - 1], &rows[at]);
    assert_eq!(now.frame, was.frame + 1, "the tick before the over tick is there");
    let mut rules = c31_rules(&was.mgr).lap;
    let mut cards: Vec<LapCard> = was.order(&addresses).into_iter().map(|k| s13d_card(&was.cards[k])).collect();
    rules.check_over(&cards);
    assert!(rules.flags & race_flags::OVER != 0, "no player still racing");
    let leader = cards[0].race_time;
    for c in cards.iter_mut() {
        if !out(c) {
            rules.clocks(now.dt, c, leader);
        }
        // The path work before the clocks, unported here, moves the progress the ranking reads.
        c.progress = s13d_card(&now.cards[c.record_word as usize]).progress;
    }
    // The draw is one rand() somewhere in the tick: the riders' and the world's own draws come first.
    let want: Vec<LapCard> = now.order(&addresses).into_iter().map(|k| s13d_card(&now.cards[k])).collect();
    let key = |c: &LapCard| (c.record_word, c.flags & card_flags::OUT, c.place, c.laps, c.race_time.0.to_bits(), c.lap_time.0.to_bits(), c.best_lap.0.to_bits());
    let mut rng = Rng::from_state(was.rand);
    let mut found = None;
    for draws in 0..5_000 {
        let (mut r, mut rl, mut cs) = (rng, rules, cards.clone());
        rl.close(now.dt, &mut cs, &[], &mut r);
        if cs.iter().map(key).eq(want.iter().map(key)) {
            found = Some((draws, rl.flags, r));
            break;
        }
        rng.rand();
    }
    let Some((draws, flags, mut r)) = found else {
        let mut cs = cards.clone();
        rules.close(now.dt, &mut cs, &[], &mut Rng::from_state(was.rand));
        panic!("frame {}: no draw gives the captured results:\n got {:?}\nwant {:?}", now.frame, cs.iter().map(key).collect::<Vec<_>>(), want.iter().map(key).collect::<Vec<_>>());
    };
    let mask = race_flags::OVER | race_flags::OVER_TOLD;
    assert_eq!(flags & mask, w(&now.mgr, 0x5c) & mask, "frame {}: the manager's flags", now.frame);
    // The tick's last state is on from the draw.
    let after = (0..5_000).find(|_| {
        let hit = r.state() == now.rand;
        r.rand();
        hit
    });
    eprintln!("frame {}: the tie draw is {draws} draws on, {after:?} more to the end of the tick; results {:?}", now.frame, want.iter().map(key).collect::<Vec<_>>());
    assert!(after.is_some(), "frame {}: the tick's generator is not on from the draw", now.frame);
    let pushed = cards.windows(2).filter(|p| p[0].race_time.0 > p[1].race_time.0).count();
    assert!(pushed >= 1, "the capture's over tick has a time to push");

    // The activity's answer to the end, the same tick: the outro scene by the human's place. The
    // player's card is the one flagged so (the seat item's own word is not captured); its statistics
    // tracker is not captured either, so the tip popup is not compared. The cue had something to
    // play: the outro is armed by the activity's own tick after (`Race::tick`), not here.
    let mut sx = Supercross::new();
    sx.hud_stage = w(&was.act, 0x470) as i32;
    sx.hud_armed = w(&was.act, 0x474);
    let ends: Vec<(i32, Option<race::EndSeat>)> = want.iter().map(|c| (c.place, (c.flags & card_flags::PLAYER != 0).then_some(race::EndSeat::default()))).collect();
    let said = sx.race_end(&ends, &event, false);
    eprintln!("frame {}: the race's end {said:?}, scene {}", now.frame, sx.hud_stage);
    assert_eq!((sx.hud_stage as u32, sx.hud_armed), (w(&now.act, 0x470), w(&now.act, 0x474)), "frame {}: the outro scene and armed word", now.frame);
    assert!(said.results && said.cue == Some(6), "the human last: scene 6");
}

// --- S13d: the race's end, handed to the front end ----------------------------------------------------

const E_UI: u32 = 0x01a0_0000;
const E_MENUS: u32 = E_UI + 0x100;
const E_RESULTS: u32 = E_UI + 0x1000;
const E_POPUP: u32 = E_UI + 0x2000;
const E_MGR: u32 = 0x01a1_0000;
const E_CARDS: u32 = 0x01a2_0000;
const E_OWNERS: u32 = 0x01a3_0000;
const E_ITEMS: u32 = 0x01a4_0000;
const E_SEATS: u32 = 0x01a5_0000;
const E_STATS: u32 = 0x01a6_0000;
const E_RIDERS: u32 = 0x01a7_0000;
const E_TOLD: u32 = 0x01a8_0000;
const E_CUES: u32 = 0x01a9_0000;
const E_FADER: u32 = 0x01aa_0000;
/// Not code: the vtable slots below land on these and are hooked.
const E_HOOK_STATS: u32 = 0x01ab_0000;
const E_HOOK_RIDER: u32 = 0x01ab_0100;
const E_HOOK_TOLD: u32 = 0x01ab_0200;
const E_HOOK_CUE: u32 = 0x01ab_0300;
const E_HOOK_FADE: u32 = 0x01ab_0400;
const E_VT: u32 = 0x01ac_0000;

#[derive(Debug, PartialEq)]
enum EndCall {
    Results(u32, u32),
    Popup(u32, u32),
    RiderTold(u32),
    Cue(u32),
    Fade(u32),
}

/// The supercross activity's answer to the race's Over notice, the whole function on 20,000 races:
/// one to six cards in list order at any places (ties, none in first, places 0 and below), players
/// and computer riders, the two seats, statistics trackers with and without preload and clutch use,
/// the event a single race or not and its `+0x116` byte, the outro already picked or armed or not,
/// one to three interface items, a cue that ends at once or not, the music fade on or off — against
/// [`Supercross::race_end`], compared on every call it makes in order with its arguments, the tip
/// popup's four words and the activity's scene, armed word and clock.
#[test]
fn the_race_end_matches_the_supercross_activitys_answer() {
    let Some(mut ee) = game_or_skip() else { return };
    let [end, app, ui, find, id, nth, bit, take, arm] = match addrs([
        "race_end",
        "app_pointer",
        "ui_manager",
        "race_tip_find",
        "interface_id",
        "list_nth",
        "name_bit",
        "race_results_take",
        "tip_popup_arm",
    ]) {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let calls: Rc<RefCell<Vec<EndCall>>> = Rc::default();
    let lookups = Rc::new(Cell::new(0u64));
    let cue_ends = Rc::new(Cell::new(false));
    let l = lookups.clone();
    ee.hooks.insert(find, Box::new(move |ee| {
        ee.set(2, l.get());
        l.set(l.get() + 1);
    }));
    ee.hooks.insert(id, Box::new(|_| {}));
    ee.hooks.insert(bit, Box::new(|ee| ee.set(2, 0)));
    ee.hooks.insert(nth, Box::new(|ee| ee.set(2, u64::from(E_RIDERS + ee.r(5) as u32 * 0x1000))));
    let c = calls.clone();
    ee.hooks.insert(take, Box::new(move |ee| c.borrow_mut().push(EndCall::Results(ee.r(4) as u32, ee.r(5) as u32))));
    let c = calls.clone();
    ee.hooks.insert(arm, Box::new(move |ee| c.borrow_mut().push(EndCall::Popup(ee.r(4) as u32, ee.r(5) as u32))));
    ee.hooks.insert(E_HOOK_STATS, Box::new(|ee| {
        let k = (ee.r(4) as u32 - E_ITEMS) / 0x1000;
        ee.set(2, u64::from(E_STATS + k * 0x100));
    }));
    ee.hooks.insert(E_HOOK_RIDER, Box::new(|ee| {
        let k = (ee.r(4) as u32 - E_RIDERS) / 0x1000;
        ee.set(2, u64::from(E_RIDERS + k * 0x1000 + 0x800 + 0x400));
    }));
    let c = calls.clone();
    ee.hooks.insert(E_HOOK_TOLD, Box::new(move |ee| c.borrow_mut().push(EndCall::RiderTold((ee.r(4) as u32 - E_TOLD) / 0x100))));
    let c = calls.clone();
    let ends = cue_ends.clone();
    ee.hooks.insert(E_HOOK_CUE, Box::new(move |ee| {
        c.borrow_mut().push(EndCall::Cue((ee.r(4) as u32 - E_CUES) / 0x100));
        ee.set(2, u64::from(!ends.get()));
    }));
    let c = calls.clone();
    ee.hooks.insert(E_HOOK_FADE, Box::new(move |ee| c.borrow_mut().push(EndCall::Fade(ee.f(12).to_bits()))));

    ee.set_u32(app, R_APP);
    ee.set_u32(R_APP + 0x3d4, R_FE);
    ee.set_u32(R_FE + 0xc, R_EVENT);
    ee.set_u32(ui, E_UI);
    ee.set_u32(E_UI + 0x30, E_MENUS);
    for (k, menu) in [E_RESULTS, E_POPUP].into_iter().enumerate() {
        // The lookup's entry k: +4 a pointer to a pointer to the thing whose +4 is the menu.
        let entry = E_MENUS + 12 * k as u32;
        ee.set_u32(entry + 4, E_UI + 0x800 + 0x10 * k as u32);
        ee.set_u32(E_UI + 0x800 + 0x10 * k as u32, E_UI + 0x900 + 0x10 * k as u32);
        ee.set_u32(E_UI + 0x900 + 0x10 * k as u32 + 4, menu);
    }
    ee.set_u32(R_ACT + 0x160, E_MGR);
    ee.set_u32(R_APP + 0x400, E_FADER - 0x100);
    ee.set_u32(E_FADER - 0x100 + 0x44, E_FADER);
    ee.set_u32(E_FADER + 8, E_VT + 0x400);
    ee.set_u32(E_VT + 0x400 + 0x94, E_HOOK_FADE);
    ee.set_u32(E_VT + 0x48 + 0x40, E_HOOK_STATS);
    ee.set_u32(E_VT + 0x100 + 0xa0, E_HOOK_RIDER);
    ee.set_u32(E_VT + 0x200 + 0x9c, E_HOOK_TOLD);
    ee.set_u32(E_VT + 0x300 + 0x1c, E_HOOK_CUE);

    let mut p = Pick(0x51c8_e2a7_09d3_6b4f);
    let (mut picked, mut tipped, mut cued, mut armed) = ([0usize; 7], 0, 0, 0);
    for case in 0..20_000 {
        calls.borrow_mut().clear();
        lookups.set(0);
        let event = race::ResultEvent { activity: if p.below(5) == 0 { [0, 1, 3][p.below(3) as usize] } else { 2 }, alt_outro: p.below(2) == 0, ..Default::default() };
        ee.set_u32(R_EVENT + 4, event.activity as u32);
        ee.write(R_EVENT + 0x116, 1, if event.alt_outro { 1 + p.below(255) as u128 } else { 0 });
        let fade = p.below(3) == 0;
        ee.set_u32(R_APP + 0x570, u32::from(fade));
        cue_ends.set(p.below(2) == 0);

        let mut act = Supercross::new();
        act.hud_stage = if p.below(3) == 0 { 0 } else { p.below(7) as i32 };
        act.hud_armed = u32::from(p.below(4) == 0);
        act.hud_fade = F(p.range(0.0, 10.0));
        ee.set_u32(R_ACT + 0x470, act.hud_stage as u32);
        ee.set_u32(R_ACT + 0x474, act.hud_armed);
        put_f(&mut ee, R_ACT + 0x410, act.hud_fade);
        for k in 0..7 {
            ee.set_u32(R_ACT + 0x44c + 4 * k, E_CUES + 0x100 * k);
            ee.set_u32(E_CUES + 0x100 * k + 8, E_VT + 0x300);
        }
        let items = 1 + p.below(3) as u32;
        ee.set_u32(R_ACT + 0x14, items);
        for k in 0..items {
            let item = E_RIDERS + k * 0x1000;
            ee.set_u32(item + 8, E_VT + 0x100);
            ee.set_u32(item + 0x800 + 0xa74, E_TOLD + k * 0x100);
            ee.set_u32(E_TOLD + k * 0x100 + 8, E_VT + 0x200);
        }

        let n = 1 + p.below(6) as u32;
        let mut cards = Vec::new();
        for k in 0..n {
            let card = E_CARDS + k * 0x200;
            let place = if p.below(8) == 0 { p.below(3) as i32 - 2 } else { 1 + p.below(6) as i32 };
            ee.set_u32(card + 0x2c, place as u32);
            ee.set_u32(card + 0x1c8, if k + 1 < n { card + 0x200 } else { 0 });
            ee.set_u32(card + 4, E_OWNERS + k * 0x100);
            ee.set_u32(E_OWNERS + k * 0x100 + 0x24, E_ITEMS + k * 0x1000);
            ee.set_u32(E_ITEMS + k * 0x1000 + 0x20, E_SEATS + k * 0x1000);
            ee.set_u32(E_ITEMS + k * 0x1000 + 0x48, E_VT + 0x48);
            let player = p.below(3) != 0;
            let seat = p.below(2) as i32;
            ee.set_u32(E_SEATS + k * 0x1000 + 0x34, if player { 1 } else { [0, 2, 0xffff_ffff][p.below(3) as usize] });
            ee.set_u32(E_SEATS + k * 0x1000 + 0x6b0, seat as u32);
            let stats = race::EndSeat { seat, preloads: if p.below(2) == 0 { 0 } else { p.below(40) as u32 }, clutches: if p.below(2) == 0 { 0 } else { p.below(40) as u32 } };
            ee.set_u32(E_STATS + k * 0x100 + 0x38, stats.preloads);
            ee.set_u32(E_STATS + k * 0x100 + 0x3c, stats.clutches);
            cards.push((place, player.then_some(stats)));
        }
        ee.set_u32(E_MGR + 0x34, E_CARDS);
        let mut popup = [0u32; 4];
        for (k, w) in popup.iter_mut().enumerate() {
            *w = p.next() as u32;
            ee.set_u32(E_POPUP + 0xc0 + 4 * k as u32, *w);
        }

        ee.call(end, &[u64::from(R_ACT)], &[], BUDGET);
        let got = act.race_end(&cards, &event, cue_ends.get());

        let mut want = Vec::new();
        if got.results {
            want.push(EndCall::Results(E_RESULTS, E_MGR));
            want.push(EndCall::Popup(E_POPUP, 1));
        }
        if let Some(scene) = got.cue {
            want.extend((0..items).map(EndCall::RiderTold));
            want.push(EndCall::Cue(scene as u32));
            if fade {
                want.push(EndCall::Fade(0.75f32.to_bits()));
            }
        }
        assert_eq!(*calls.borrow(), want, "case {case}: {event:?} {cards:?} → {got:?}");
        for t in &got.tips {
            popup[t.seat as usize] = u32::from(t.no_preload);
            popup[2 + t.seat as usize] = u32::from(t.no_clutch);
        }
        let words: Vec<u32> = (0..4).map(|k| ee.u32_at(E_POPUP + 0xc0 + 4 * k)).collect();
        assert_eq!(words, popup, "case {case}: the tip popup's words, {cards:?}");
        assert_eq!(
            (ee.u32_at(R_ACT + 0x470), ee.u32_at(R_ACT + 0x474), ee.u32_at(R_ACT + 0x410)),
            (act.hud_stage as u32, act.hud_armed, act.hud_fade.0.to_bits()),
            "case {case}: scene, armed, clock"
        );
        picked[act.hud_stage as usize] += usize::from(got.results);
        tipped += got.tips.len();
        cued += usize::from(got.cue.is_some());
        armed += usize::from(got.cue.is_some() && cue_ends.get());
    }
    assert!(picked[1..].iter().all(|&n| n > 300) && tipped > 5000 && cued > 8000 && armed > 4000, "scenes {picked:?}, tips {tipped}, cues {cued}, armed {armed}");
}

// --- the AI's path look-ahead (S308) -----------------------------------------------------------------

const T_HINT: u32 = 0x01b5_0060;
const T_NEAR: u32 = 0x01b5_0070;
const T_DRAWS: u32 = 0x01b5_0080;
const T_LIST: u32 = 0x01b5_0200;

/// Fork words for a generated course: each path's `+0xc` speed, now and then a stray word, and the
/// track's link-pick mode (3 most often, as every loader sets it).
fn t_forks(p: &mut Pick, t: &mut TrackPaths) {
    for path in &mut t.paths {
        let v = p.range(0.0, 150.0);
        path.word_0c = p.stray(v).to_bits();
    }
    t.pick = match p.below(6) {
        0 => 1,
        1 => 2,
        2 => p.next() as u32 & 7,
        _ => 3,
    };
}

/// A limit for the locates: none (−1), 0, a length, or a stray word.
fn t_limit(p: &mut Pick) -> F {
    match p.below(6) {
        0 => F(-1.0),
        1 => F(0.0),
        _ => {
            let x = p.range(-20.0, 300.0);
            F(p.stray(x))
        }
    }
}

/// A point near some node of the course, or anywhere about it.
fn t_point(p: &mut Pick, t: &TrackPaths) -> Vf {
    let i = p.below(t.paths.len() as u64) as usize;
    let n = p.below(t.paths[i].nodes.len() as u64) as usize;
    let q = t.paths[i].nodes[n].points[0];
    let point = match p.below(8) {
        0 => Vf([p.range(-150.0, 150.0), p.range(-5.0, 5.0), p.range(-150.0, 150.0), 1.0]),
        1 => q,
        _ => Vf([q.x() + p.range(-25.0, 25.0), p.range(-5.0, 5.0), q.z() + p.range(-25.0, 25.0), 1.0]),
    };
    Vf([p.stray(point.x()), point.y(), p.stray(point.z()), point.w()])
}

/// The words below the oracle's stack pointer zeroed: the route locate hands back stack words it never
/// set, and the port starts them at zero (see `route_locate`).
fn t_clear_stack(ee: &mut Ee) {
    for k in 0..0x800u32 {
        ee.set_u32(ee.stack - 4 * (k + 1), 0);
    }
}

/// The path locate with a start hint against SLUS-21104's (`path_locate`), over 20,000 generated courses:
/// no hint, a hint on the path (now and then naming no node), a hint on another path (the walk then
/// starts at the way along the paths from it, through the route search), limits, refused keep-off paths
/// and holding-only walks. The answer, the out spot, the distance, and the paths' scratch words and pools.
#[test]
fn the_hinted_path_locate_matches_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let locate = match addr("path_locate") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x5308_10ca_7e0f_beef);
    let mut seen = [0usize; 8];
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        t_forks(&mut p, &mut t);
        put_paths(&mut ee, &mut p, &t);
        let path = p.below(t.paths.len() as u64) as usize;
        let hint = match p.below(4) {
            0 => None,
            1 => {
                let mut h = t_spot_on(&mut p, &t);
                h.path = path as u32 + 1;
                h.piece = if p.below(25) == 0 { 0 } else { 1 + p.below(t.paths[path].nodes.len() as u64) as u32 };
                Some(h)
            }
            _ => Some(t_spot_on(&mut p, &t)),
        };
        let point = t_point(&mut p, &t);
        let limit = t_limit(&mut p);
        let refuse = p.below(3) == 0;
        let holding = p.below(3) == 0;
        let with_distance = p.below(4) != 0;
        let fill = t_spot_on(&mut p, &t);
        let d_fill = F(p.unit());
        put_quad(&mut ee, T_POINT, point);
        put_t_spot(&mut ee, T_OUT, fill);
        put_f(&mut ee, T_D, d_fill);
        if let Some(h) = hint {
            put_t_spot(&mut ee, T_HINT, h);
        }
        let what = format!("case {case}: path {path}, hint {hint:?}, {point:?}, limit {limit:?}, refuse {refuse}, holding {holding}");
        let args = [
            u64::from(T_TRACK),
            if hint.is_some() { u64::from(T_HINT) } else { 0 },
            u64::from(T_POINT),
            u64::from(t_rec(path)),
            u64::from(T_OUT),
            if with_distance { u64::from(T_D) } else { 0 },
            u64::from(refuse) * (1 + (p.next() & 0xff)),
            u64::from(holding),
        ];
        ee.call(locate, &args, &[limit.0], BUDGET * 10);
        let mut out = fill;
        let mut d = d_fill;
        let got = game_sim::ai::follow::locate_on(&mut t, limit, path, hint, point, refuse, holding, &mut out, with_distance.then_some(&mut d));
        assert_eq!(ee.v0() as u32, u32::from(got), "{what}: the answer");
        assert_eq!(t_bits(t_spot_at(&ee, T_OUT)), t_bits(out), "{what}: the spot");
        assert_eq!(f32_at(&ee, T_D).0.to_bits(), d.0.to_bits(), "{what}: the distance");
        check_paths(&ee, &t, &what);
        seen[usize::from(got)] += 1;
        let on = hint.is_some_and(|h| h.path == path as u32 + 1);
        seen[2] += usize::from(got && on);
        seen[3] += usize::from(got && hint.is_some() && !on);
        seen[4] += usize::from(got && on && t.paths[path].links.contains(&path));
        seen[5] += usize::from(got && with_distance && d == F(-1.0));
        seen[6] += usize::from(got && limit >= F(0.0) && out.piece != fill.piece);
        seen[7] += usize::from(got && holding && (out.piece, out.along) != (fill.piece, fill.along));
    }
    eprintln!(
        "hinted locate: refused {}, found {}, hint on the path {}, on another {}, looping {}, none taken {}, limited {}, holding {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5], seen[6], seen[7]
    );
    assert!(
        seen[0] > 800 && seen[2] > 3000 && seen[3] > 6000 && seen[4] > 300 && seen[5] > 300 && seen[6] > 3000 && seen[7] > 1000,
        "{seen:?}"
    );
}

/// The route locate against SLUS-21104's (`path_route_locate`), over 20,000 routes of one to five paths
/// (repeats allowed, the list now and then closed into a ring): the nearest place on any of them, with a
/// hint, limit, refusal and holding-only as above. The answer, the out spot, the distance, the scratch
/// words and pools.
#[test]
fn the_route_locate_matches_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let locate = match addr("path_route_locate") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    let mut p = Pick(0x5308_7041_e10c_a7e5);
    let mut seen = [0usize; 6];
    for case in 0..20_000 {
        let mut t = t_paths(&mut p);
        t_forks(&mut p, &mut t);
        put_paths(&mut ee, &mut p, &t);
        let len = if p.below(30) == 0 { 0 } else { 1 + p.below(5) as usize };
        let route: Vec<usize> = (0..len).map(|_| p.below(t.paths.len() as u64) as usize).collect();
        let ring = p.below(4) == 0;
        for (k, &r) in route.iter().enumerate() {
            let e = T_LIST + 0x10 * k as u32;
            ee.set_u32(e, p.next() as u32 & !0xff);
            ee.set_u32(e + 4, t_rec(r));
            ee.set_u32(e + 8, p.next() as u32);
            let next = if k + 1 < len { e + 0x10 } else if ring { T_LIST } else { 0 };
            ee.set_u32(e + 0xc, next);
        }
        let hint = if p.below(5) == 0 { None } else { Some(t_spot_on(&mut p, &t)) };
        let point = t_point(&mut p, &t);
        let limit = t_limit(&mut p);
        let refuse = p.below(3) == 0;
        let holding = p.below(4) == 0;
        let with_out = p.below(8) != 0;
        let with_distance = p.below(4) != 0;
        let fill = t_spot_on(&mut p, &t);
        let d_fill = F(p.unit());
        put_quad(&mut ee, T_POINT, point);
        put_t_spot(&mut ee, T_OUT, fill);
        put_f(&mut ee, T_D, d_fill);
        if let Some(h) = hint {
            put_t_spot(&mut ee, T_HINT, h);
        }
        t_clear_stack(&mut ee);
        let what = format!("case {case}: route {route:?}, hint {hint:?}, {point:?}, limit {limit:?}, refuse {refuse}, holding {holding}");
        let args = [
            u64::from(T_TRACK),
            if len == 0 { 0 } else { u64::from(T_LIST) },
            if hint.is_some() { u64::from(T_HINT) } else { 0 },
            u64::from(T_POINT),
            if with_out { u64::from(T_OUT) } else { 0 },
            if with_distance { u64::from(T_D) } else { 0 },
            u64::from(refuse) * (1 + (p.next() & 0xff)),
            u64::from(holding),
        ];
        ee.call(locate, &args, &[limit.0], BUDGET * 40);
        let mut out = fill;
        let mut d = d_fill;
        let got = game_sim::ai::follow::route_locate(
            &mut t, limit, &route, hint, point, refuse, holding, with_out.then_some(&mut out), with_distance.then_some(&mut d),
        );
        assert_eq!(ee.v0() as u32, u32::from(got), "{what}: the answer");
        assert_eq!(t_bits(t_spot_at(&ee, T_OUT)), t_bits(out), "{what}: the spot");
        assert_eq!(f32_at(&ee, T_D).0.to_bits(), d.0.to_bits(), "{what}: the distance");
        check_paths(&ee, &t, &what);
        seen[usize::from(got)] += 1;
        seen[2] += usize::from(got && len > 1 && with_out && out.path != route[0] as u32 + 1);
        seen[3] += usize::from(got && with_distance && d == F(-1.0));
        seen[4] += usize::from(got && ring);
        seen[5] += usize::from(got && refuse && route.iter().any(|&r| t.paths[r].flags & 0x100 != 0));
    }
    eprintln!(
        "route locate: refused {}, found {}, a later path nearer {}, none placed {}, ring {}, keep-off passed over {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]
    );
    assert!(seen[0] > 300 && seen[2] > 3000 && seen[3] > 300 && seen[4] > 3000 && seen[5] > 1000, "{seen:?}");
}

/// The vehicle brain's path look-ahead against SLUS-21104's, over 30,000 generated courses: half through
/// the brain's vt+0x88 (`ai_path_look_ahead`, its route word 0 on the stack) and half straight into the
/// look-ahead (`path_look_ahead`). Reaches that stay on the spot's path, run on over picked forks (speed
/// words, draws and the four pick modes), run off the paths' end or the pool, with and without the nearest
/// place on the route. The answer, both out spots, the scratch words and pools.
///
/// Left out, because the original never answers: a spot on a one-node path, and a refused drawn link.
#[test]
fn the_path_look_ahead_matches_the_track_objects() {
    let Some(mut ee) = game_or_skip() else { return };
    let (forward, look) = match (addr("ai_path_look_ahead"), addr("path_look_ahead")) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(why), _) | (_, Err(why)) => return skip(&why),
    };
    let mut p = Pick(0x5308_100c_a4ea_d000);
    let mut seen = [0usize; 10];
    let mut case = 0;
    while case < 30_000 {
        let mut t = t_paths(&mut p);
        t_forks(&mut p, &mut t);
        let long: Vec<usize> = (0..t.paths.len()).filter(|&i| t.paths[i].nodes.len() > 1).collect();
        if long.is_empty() {
            continue;
        }
        let refuse = p.below(3) == 0;
        if refuse {
            for i in 0..t.paths.len() {
                if t.paths[i].links.iter().any(|&l| t.paths[l].flags & 0x100 != 0) {
                    t.paths[i].flags &= !0x20;
                }
            }
        }
        put_paths(&mut ee, &mut p, &t);
        let first = long[p.below(long.len() as u64) as usize];
        let mut spot = t_spot_on(&mut p, &t);
        spot.path = first as u32 + 1;
        spot.piece = 1 + p.below(t.paths[first].nodes.len() as u64) as u32;
        let reach = match p.below(10) {
            0 => F(0.0),
            1 => F(-p.range(0.0, 10.0)),
            2 => {
                let x = p.range(0.0, 400.0);
                F(p.stray(x))
            }
            _ => F(p.range(0.0, 400.0)),
        };
        let speed = F(p.range(0.0, 150.0));
        let limit = t_limit(&mut p);
        let draws: [F; 10] = std::array::from_fn(|_| match p.below(12) {
            0 => F(0.0),
            1 => F(f32::from_bits(0x3f7f_ffff)),
            _ => F(p.unit()),
        });
        let point = t_point(&mut p, &t);
        let holding = p.below(4) == 0;
        let with_near = p.below(5) != 0;
        let (fill, near_fill) = (t_spot_on(&mut p, &t), t_spot_on(&mut p, &t));
        put_t_spot(&mut ee, T_HINT, spot);
        put_t_spot(&mut ee, T_OUT, fill);
        put_t_spot(&mut ee, T_NEAR, near_fill);
        put_quad(&mut ee, T_POINT, point);
        for (k, d) in draws.iter().enumerate() {
            put_f(&mut ee, T_DRAWS + 4 * k as u32, *d);
        }
        t_clear_stack(&mut ee);
        let refuse_word = u64::from(refuse) * (1 + (p.next() & 0xff));
        let near_at = if with_near { u64::from(T_NEAR) } else { 0 };
        let by_brain = p.below(2) == 0;
        let what = format!(
            "case {case}: spot {spot:?}, reach {reach:?}, speed {speed:?}, limit {limit:?}, refuse {refuse}, holding {holding}, near {with_near}, pick {}",
            t.pick
        );
        if by_brain {
            ee.set_u32(ee.stack, u32::from(holding));
            ee.set_u32(ee.stack + 4, 0);
            ee.set_u32(ee.stack + 8, 0);
            ee.set_u32(ee.stack + 12, 0);
            let brain = 0x01b6_0000 + 0x10 * p.below(0x100) as u32;
            let args = [
                u64::from(brain),
                u64::from(T_TRACK),
                u64::from(T_HINT),
                u64::from(T_OUT),
                u64::from(T_POINT),
                near_at,
                u64::from(T_DRAWS),
                refuse_word,
            ];
            ee.call(forward, &args, &[reach.0, speed.0, limit.0], BUDGET * 60);
        } else {
            let args = [
                u64::from(T_TRACK),
                u64::from(T_HINT),
                u64::from(T_OUT),
                u64::from(T_POINT),
                near_at,
                u64::from(T_DRAWS),
                refuse_word,
                u64::from(holding),
            ];
            ee.call(look, &args, &[reach.0, speed.0, limit.0], BUDGET * 60);
        }
        let (mut out, mut near) = (fill, near_fill);
        let before = t.free_routes;
        let got = game_sim::ai::follow::look_ahead(
            &mut t, reach, speed, limit, spot, point, &draws, refuse, holding, &mut out, with_near.then_some(&mut near),
        );
        assert_eq!(ee.v0() as u32, u32::from(got), "{what}: the answer");
        assert_eq!(t_bits(t_spot_at(&ee, T_OUT)), t_bits(out), "{what}: the spot ahead");
        assert_eq!(t_bits(t_spot_at(&ee, T_NEAR)), t_bits(near), "{what}: the nearest spot");
        check_paths(&ee, &t, &what);
        seen[usize::from(got)] += 1;
        seen[2] += usize::from(got && out.path == spot.path);
        seen[3] += usize::from(got && out.path != spot.path);
        seen[4] += usize::from(!got && before > 0 && out == fill && !(refuse && t.paths[first].flags & 0x100 != 0));
        seen[5] += usize::from(before == 0);
        seen[6] += usize::from(got && with_near && near != near_fill);
        seen[7] += usize::from(got && out.path != spot.path && t.paths[first].flags & 0x8 != 0);
        seen[8] += usize::from(got && out.path != spot.path && t.paths[first].flags & 0x28 == 0x20);
        seen[9] += usize::from(by_brain && got);
        case += 1;
    }
    eprintln!(
        "look-ahead: no {}, yes {}, on the spot's path {}, on past it {}, ran out {}, dry pool {}, nearest {}, by speed {}, by draw {}, through vt+0x88 {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5], seen[6], seen[7], seen[8], seen[9]
    );
    assert!(
        seen[2] > 3000 && seen[3] > 6000 && seen[4] > 1000 && seen[5] > 100 && seen[6] > 6000 && seen[7] > 500 && seen[8] > 500 && seen[9] > 6000,
        "{seen:?}"
    );
}

// --- the AI's path follower (S309) -------------------------------------------------------------------

const F_AI: u32 = 0x01b6_0000;
const F_JUMP: u32 = 0x01b6_1000;
const F_RACE: u32 = 0x01b6_2000;
const PF_VT: u32 = 0x01b6_3000;
/// Not code: the brain's other vtable slots land here and fail the case.
const PF_HOOKS: u32 = 0x01b6_4000;
const PF_SHIM: u32 = 0x01b6_5000;
/// Not code: the brain's vt+0x8c lands here (see the path branch's oracle).
const PF_FOLLOW: u32 = 0x01b6_6000;

/// The follower's part of the brain into RAM.
fn put_follower(ee: &mut Ee, f: &game_sim::ai::follow::Follower) {
    ee.write(F_AI + 0x30, 8, u128::from(f.flags));
    ee.set_u32(F_AI + 0x354, f.state as u32);
    put_quad(ee, F_AI + 0xc0, f.position);
    put_quad(ee, F_AI + 0xb0, f.forward);
    put_f(ee, F_AI + 0x80, f.own_speed);
    put_t_spot(ee, F_AI + 0x220, f.spot);
    for (k, d) in f.draws.iter().enumerate() {
        put_f(ee, F_AI + 0x22c + 4 * k as u32, *d);
    }
    put_quad(ee, F_AI + 0x1b0, f.aim);
    put_quad(ee, F_AI + 0x1d0, f.heading);
    put_f(ee, F_AI + 0x1e0, f.speed);
    put_quad(ee, F_AI + 0x1f0, f.angles);
    let words = [
        f.drift, f.kick, f.kick_time, f.kick_decay, f.kick_clock, f.drift_target, f.drift_low, f.drift_high, f.drift_rate, f.drift_max,
        f.drift_clock, f.period_low, f.period_high, f.period,
    ];
    for (k, w) in words.iter().enumerate() {
        put_f(ee, F_AI + 0x164 + 4 * k as u32, *w);
    }
    put_f(ee, F_AI + 0x210, f.reach.base);
    put_f(ee, F_AI + 0x214, f.reach.max);
    put_f(ee, F_AI + 0x218, f.reach.gain);
    put_f(ee, F_AI + 0x2e0, f.cruise);
    put_f(ee, F_AI + 0x2f0, f.look);
    put_f(ee, F_AI + 0x2f4, f.look_cone);
}

/// The follower's part of the brain read back.
fn follower_at(ee: &Ee) -> game_sim::ai::follow::Follower {
    let w = |o: u32| f32_at(ee, F_AI + o);
    game_sim::ai::follow::Follower {
        flags: ee.read(F_AI + 0x30, 8) as u64,
        state: ee.u32_at(F_AI + 0x354) as i32,
        position: quad(ee, F_AI + 0xc0),
        forward: quad(ee, F_AI + 0xb0),
        own_speed: w(0x80),
        spot: t_spot_at(ee, F_AI + 0x220),
        draws: std::array::from_fn(|k| w(0x22c + 4 * k as u32)),
        aim: quad(ee, F_AI + 0x1b0),
        heading: quad(ee, F_AI + 0x1d0),
        speed: w(0x1e0),
        angles: quad(ee, F_AI + 0x1f0),
        drift: w(0x164),
        kick: w(0x168),
        kick_time: w(0x16c),
        kick_decay: w(0x170),
        kick_clock: w(0x174),
        drift_target: w(0x178),
        drift_low: w(0x17c),
        drift_high: w(0x180),
        drift_rate: w(0x184),
        drift_max: w(0x188),
        drift_clock: w(0x18c),
        period_low: w(0x190),
        period_high: w(0x194),
        period: w(0x198),
        reach: game_sim::ai::seek::Reach { base: w(0x210), max: w(0x214), gain: w(0x218) },
        cruise: w(0x2e0),
        look: w(0x2f0),
        look_cone: w(0x2f4),
    }
}

/// Everything in a follower as bits, for a comparison that tells `-0` from `0`.
fn follower_bits(f: &game_sim::ai::follow::Follower) -> Vec<u32> {
    let mut v = vec![f.flags as u32, (f.flags >> 32) as u32, f.state as u32];
    for q in [f.position, f.forward, f.aim, f.heading, f.angles] {
        v.extend(q.to_bits());
    }
    let (a, b, c) = t_bits(f.spot);
    v.extend([a, b, c]);
    v.extend(f.draws.iter().map(|d| d.0.to_bits()));
    let words = [
        f.own_speed, f.speed, f.drift, f.kick, f.kick_time, f.kick_decay, f.kick_clock, f.drift_target, f.drift_low, f.drift_high, f.drift_rate,
        f.drift_max, f.drift_clock, f.period_low, f.period_high, f.period, f.reach.base, f.reach.max, f.reach.gain, f.cruise, f.look, f.look_cone,
    ];
    v.extend(words.iter().map(|w| w.0.to_bits()));
    v
}

/// A level direction, `y` small, `w` any.
fn f_dir(p: &mut Pick) -> Vf {
    let a = p.range(-3.2, 3.2);
    let (s, c) = a.sin_cos();
    let y = if p.below(3) == 0 { p.range(-0.2, 0.2) } else { 0.0 };
    Vf([c, y, s, p.range(-1.0, 1.0)])
}

/// A spot the follower may hold: on a stretch with `along` in 0..1, or at a path's last node with
/// `along` 0 (the places the searches hand out).
fn f_spot(p: &mut Pick, t: &TrackPaths) -> Spot {
    let i = p.below(t.paths.len() as u64) as usize;
    let len = t.paths[i].nodes.len();
    if p.below(10) == 0 {
        return Spot::on(i, len - 1, F(0.0));
    }
    let along = match p.below(10) {
        0 => 0.0,
        1 => 1.0,
        _ => p.unit(),
    };
    Spot::on(i, p.below(len as u64 - 1) as usize, F(along))
}

/// The tick's path branch against SLUS-21104's, over 12,000 generated courses: from
/// `ai_seek_path_branch` to `ai_seek_path_end` with every callee the original's (the brain's vt+0x78
/// reach, vt+0x8c route follower and its vt+0x88 look-ahead, the path queries and `rand`). Riders and
/// goals placed or not yet, the goal's route or the plain look-ahead, the look-ahead cone, drift and its
/// draws, the side step, the node speed words and their blend, the hand-over to the seeker. The brain's
/// follower words, the goal's spot, the paths' scratch words and pools, and the generator.
///
/// Left out, because the original reads words nobody set: a rider or goal the net search cannot place
/// (it then reads its null spot, or points it never wrote on its stack), a course with a one-node path
/// (the measure to its end never returns), a rider spot whose point is not answered, and a look-ahead
/// at a spot with no way on (the cone's way is then stale stack: the flag is cleared for those).
#[test]
fn the_ais_path_branch_matches_the_tick() {
    use game_sim::ai::follow::{self, Follower, Goal};
    let Some(mut ee) = game_or_skip() else { return };
    let [branch, end, reach_fn, look_fn, follow_fn, rand_state] =
        match addrs(["ai_seek_path_branch", "ai_seek_path_end", "ai_path_reach", "ai_path_look_ahead", "ai_route_follow", "rand_state"]) {
            Ok(a) => a,
            Err(why) => return skip(&why),
        };
    // vt+0x88 hands the route search the address of a list head on its stack it never set (128 bytes
    // down, at +124): the shim zeroes it, as the port starts the route empty, then jumps in.
    ee.set_u32(PF_SHIM, 0xafa0_fffc); // sw $zero, -4($sp)
    ee.set_u32(PF_SHIM + 4, 0x0800_0000 | (look_fn >> 2) & 0x03ff_ffff); // j look_fn
    ee.set_u32(PF_SHIM + 8, 0); // nop
    for slot in (0..0x100u32).step_by(4) {
        let target = match slot {
            0x78 => reach_fn,
            0x88 => PF_SHIM,
            0x8c => PF_FOLLOW,
            _ => {
                let h = PF_HOOKS + slot;
                ee.hooks.insert(h, Box::new(move |_| panic!("the path branch called the brain's vtable slot {slot:#x}")));
                h
            }
        };
        ee.set_u32(PF_VT + slot, target);
    }
    ee.hooks.insert(end, Box::new(|ee| ee.set(31, ps2_oracle::DONE as i32 as i64 as u64)));
    // The route follower, run nested (at the stack pointer it would get anyway). A route path that
    // placed nothing hands on the place before it with its own path, so the spot can name another
    // path's node, which the port's spot cannot say; the measure after it then recurses for good.
    // Such a case stops here and is left out.
    let cross = Rc::new(Cell::new(false));
    let seen_cross = cross.clone();
    ee.hooks.insert(PF_FOLLOW, Box::new(move |ee| {
        let ra = ee.r(31);
        let ints: Vec<u64> = (4..10).map(|r| ee.r(r)).collect();
        let floats = [ee.f(12), ee.f(13)];
        ee.call(follow_fn, &ints, &floats, BUDGET * 300);
        let node = ee.u32_at(F_AI + 0x224);
        let other = node != 0 && ee.u32_at(F_AI + 0x220) != t_rec(((node - T_NODES) / 0x60 / 16) as usize);
        seen_cross.set(other);
        ee.set(31, if other { ps2_oracle::DONE as i32 as i64 as u64 } else { ra });
        // The hook goes back under the address it was reached at.
        ee.pc = PF_FOLLOW;
    }));

    let mut p = Pick(0x5309_ba7c_f011_0e5a);
    let mut seen = std::collections::BTreeMap::<&str, u32>::new();
    let mut case = 0;
    let mut tries = 0;
    while case < 12_000 {
        tries += 1;
        assert!(tries < 100_000, "too few usable courses: {case}");
        let mut t = t_paths(&mut p);
        if t.paths.iter().any(|path| path.nodes.len() < 2) {
            continue;
        }
        t_forks(&mut p, &mut t);
        for path in &mut t.paths {
            for node in &mut path.nodes {
                node.word_04 = p.next() as u32 & !follow::NO_DRIFT | if p.below(4) == 0 { follow::NO_DRIFT } else { 0 };
                let speed = p.range(20.0, 150.0);
                node.word_44 = match p.below(3) {
                    0 => (-1.0f32).to_bits(),
                    _ => p.stray(speed).to_bits(),
                };
            }
        }

        let mut flags = p.next() & !(follow::LOOKS_AHEAD | follow::DRIFTS | follow::BLENDS_SPEED | game_sim::ai::seek::CLOSING);
        for (bit, odds) in [(follow::LOOKS_AHEAD, 2), (follow::DRIFTS, 4), (follow::BLENDS_SPEED, 2), (game_sim::ai::seek::CLOSING, 2)] {
            if p.below(odds) != 0 {
                flags |= bit;
            }
        }
        let low = p.range(0.0, 3.0);
        let plo = p.range(0.0, 2.0);
        let mut f = Follower {
            flags,
            state: 0,
            position: t_point(&mut p, &t),
            forward: f_dir(&mut p),
            own_speed: F(p.range(0.0, 150.0)),
            spot: if p.below(4) == 0 { Spot::default() } else { f_spot(&mut p, &t) },
            draws: std::array::from_fn(|_| F(p.unit())),
            aim: Vf([p.range(-100.0, 100.0), p.range(-5.0, 5.0), p.range(-100.0, 100.0), p.range(-1.0, 1.0)]),
            heading: f_dir(&mut p),
            speed: F(p.range(0.0, 150.0)),
            angles: Vf([p.unit(), p.unit(), p.unit(), p.unit()]),
            drift: F(p.range(-6.0, 6.0)),
            kick: F(p.range(-3.0, 3.0)),
            kick_time: F(p.range(0.0, 2.0)),
            kick_decay: F(p.range(0.0, 5.0)),
            kick_clock: F(if p.below(3) == 0 { p.range(0.0, 0.5) } else { p.range(0.0, 3.0) }),
            drift_target: F(p.range(-6.0, 6.0)),
            drift_low: F(low),
            drift_high: F(low + p.range(0.0, 4.0)),
            drift_rate: F(p.range(0.0, 10.0)),
            drift_max: F(p.range(0.0, 6.0)),
            drift_clock: F(p.range(0.0, 3.0)),
            period_low: F(plo),
            period_high: F(plo + p.range(0.0, 3.0)),
            period: F(p.range(0.0, 3.0)),
            reach: game_sim::ai::seek::Reach {
                base: F(p.range(0.0, 20.0)),
                max: F(if p.below(10) == 0 { -1.0 } else { p.range(0.0, 80.0) }),
                gain: F(p.range(0.0, 2.0)),
            },
            cruise: F(p.range(0.0, 150.0)),
            look: F(p.range(0.0, 200.0)),
            look_cone: F(p.range(-1.0, 1.0)),
        };
        let mut goal = (p.below(3) != 0).then(|| Goal {
            point: t_point(&mut p, &t),
            spot: if p.below(2) == 0 { Spot::default() } else { f_spot(&mut p, &t) },
        });
        let race_word = (p.below(3) != 0).then(|| p.next() as u32);
        let dt = F(p.range(0.01, 0.05));
        let state = p.next();

        // The two searches the branch starts with, run on a copy, to leave out the cases the original
        // reads unset words in.
        let mut probe = t.clone();
        if let Some(g) = &mut goal {
            let mut s = g.spot;
            if s.path == 0 {
                follow::spot(&mut probe, g.point, false, &mut s, None);
            }
            if s.at().is_none() {
                continue;
            }
        }
        let mut s = f.spot;
        if s.path == 0 {
            follow::spot(&mut probe, f.position, false, &mut s, None);
        }
        if s.at().is_none() {
            continue;
        }
        let sentinel = Vf([1234.5, 0.0, 0.0, 0.0]);
        let (mut q, mut s2) = (sentinel, s);
        if !probe.spot_point(&mut s2, &mut q) && q == sentinel {
            continue;
        }
        let mut way = sentinel;
        if f.flags & follow::LOOKS_AHEAD != 0 && (!probe.path_ahead(s, &mut way) || way == sentinel) {
            f.flags &= !follow::LOOKS_AHEAD;
            *seen.entry("look-ahead dropped: no way on").or_default() += 1;
        }

        put_paths(&mut ee, &mut p, &t);
        ee.set_u32(T_TRACK + 0x4c, t_rec(0));
        for (i, path) in t.paths.iter().enumerate() {
            for (n, node) in path.nodes.iter().enumerate() {
                ee.set_u32(t_node(i, n) + 4, node.word_04);
                ee.set_u32(t_node(i, n) + 0x44, node.word_44);
            }
        }
        ee.load(F_AI, &[0; 0x400]);
        ee.load(F_JUMP, &[0; 0x200]);
        ee.set_u32(F_AI + 8, PF_VT);
        ee.set_u32(F_AI + 0xdc, T_TRACK);
        put_follower(&mut ee, &f);
        if let Some(g) = &goal {
            ee.set_u32(F_AI + 0x34c, F_JUMP);
            put_quad(&mut ee, F_JUMP + 0xe0, g.point);
            put_t_spot(&mut ee, F_JUMP + 0x110, g.spot);
        }
        if let Some(w) = race_word {
            ee.set_u32(F_AI + 0xd8, F_RACE);
            ee.set_u32(F_RACE + 0x5c, w);
        }
        ee.write(rand_state, 8, u128::from(state));
        ee.set(19, u64::from(F_AI));
        ee.set_f(22, dt.0);
        // The seeker leaves the net in a0 for the goal's search.
        cross.set(false);
        ee.call(branch, &[u64::from(T_TRACK)], &[], BUDGET * 400);
        if cross.get() {
            *seen.entry("left out: another path's node").or_default() += 1;
            continue;
        }
        let before = f;
        let goal_before = goal;
        let free_before = t.free_routes;
        let mut rng = Rng::from_state(state);
        follow::path(&mut f, &mut t, dt, goal.as_mut(), race_word, &mut rng);
        let what = format!("case {case}: {before:?} goal {goal_before:?} race {race_word:?} dt {dt:?} pick {}", t.pick);
        let theirs = follower_at(&ee);
        assert_eq!(follower_bits(&theirs), follower_bits(&f), "{what}: the brain\n original {theirs:?}\n port     {f:?}");
        if let Some(g) = &goal {
            assert_eq!(t_bits(t_spot_at(&ee, F_JUMP + 0x110)), t_bits(g.spot), "{what}: the goal's spot");
        }
        check_paths(&ee, &t, &what);
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator");

        let mut count = |k: &'static str| *seen.entry(k).or_default() += 1;
        if before.spot.path == 0 {
            count("rider placed");
        }
        if goal_before.is_some_and(|g| g.spot.path == 0) {
            count("goal placed");
        }
        if goal.is_some() {
            count("by route");
        }
        let held = f.speed == f.own_speed && f.aim == f.position && f.heading == f.forward;
        if held {
            count("nothing ahead");
        } else {
            count("followed");
            if f.draws != before.draws {
                count("drew again");
            }
            if f.drift_clock == F(0.0) {
                count("new drift target");
            }
            if f.drift == F(0.0) {
                count("no drift");
            } else if f.drift == f.drift_max || f.drift == -f.drift_max {
                count("drift held at its most");
            }
            if f.kick_clock != before.kick_clock {
                count("side step");
            }
            if f.flags & follow::BLENDS_SPEED != 0 && f.speed != before.speed {
                count("speed by the nodes");
            }
            if f.speed == F(150.0) {
                count("default speed");
            }
        }
        if before.flags & follow::LOOKS_AHEAD != 0 {
            count("looked ahead");
        }
        if f.state == 1 {
            count("handed over");
        }
        if t.free_routes < free_before.min(3) {
            count("pool low");
        }
        case += 1;
    }
    eprintln!("path branch over {tries} courses: {seen:?}");
    let at = |k: &str| seen.get(k).copied().unwrap_or(0);
    assert!(
        at("rider placed") > 1500
            && at("goal placed") > 1500
            && at("by route") > 5000
            && at("nothing ahead") > 300
            && at("followed") > 5000
            && at("drew again") > 500
            && at("new drift target") > 1000
            && at("side step") > 500
            && at("speed by the nodes") > 500
            && at("default speed") > 300
            && at("looked ahead") > 2000
            && at("handed over") > 500,
        "{seen:?}"
    );
}
