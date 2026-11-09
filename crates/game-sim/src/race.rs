//! The race activities (S13): the supercross and nationals races, their start gate, laps and results.
//!
//! The first piece is the gate itself (S13a). A race track's gates — the start gate the riders wait
//! behind, and the lines the activity counts crossings of — are all one kind of object: a rectangle
//! standing on the track, with a centre, the direction a rider is meant to pass it in, the direction
//! across it, a width and a height. [`Gate::crossed`] is the test every one of them uses: given where an
//! entity was last tick and where it is now, did that step pass through the rectangle, beside it, or not
//! reach it at all. The original calls it from a dozen places (the race manager's start gate, the score
//! cards' lines, the stunt and checkpoint activities); this is the one function behind all of them.
//!
//! While the start gate is closed, a rider that rolls through it early is thrown back behind it and
//! loses the gas for a moment: [`gate_push_back`] and [`gate_penalty`] are the two draws that decide by
//! how much, in the order the race manager's tick makes them. The penalty is what the score card counts
//! down ([`crate::score::Score::gate_penalty`]).
//!
//! Around the gates stands the activity itself, [`Supercross`]: one class behind both the supercross and
//! the nationals races. It holds the start's timeline (the announcer's start call, picked at random
//! each reset, with the gate-drop moment its clip carries), the eight lanes across the start gate and
//! which rider stands in which, and the counters the race manager's tick runs down. [`Supercross::new`]
//! is its construction, [`Supercross::reset`] the reset slot every restart goes through, and
//! [`Supercross::shuffle_lanes`], [`Supercross::grid_spot`] and [`Supercross::starting_place`] the
//! slot that lines the riders up behind the gate. The score cards hang off it as a list, one per rider;
//! a card's index picks its lane, and its place before the first crossing is that lane's number.
//!
//! Once the race is on, each card's crossings of the finish line count its laps: [`LapRules::line`] is
//! the score card's lap line (S13c) — the lap's time, the best lap, the kept lap records, the finish in
//! a race that ends at the line, and a ghost race's run starting over — with what it tells the
//! presentation as [`LineNotice`]s.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::contact::unit;
use crate::rng::Rng;
use ps2_float::{Dest, F, Vf, fpu};

/// The gate's flag bits (its `+0x30` word).
pub mod flags {
    /// The gate is live. A start gate holds this while it is closed; with it clear, every test answers
    /// [`super::Crossing::Open`].
    pub const CLOSED: u32 = 0x01;
    /// A step that passes through the rectangle counts whichever way it went. Without it, only a step
    /// along the gate's direction does.
    pub const EITHER_WAY: u32 = 0x04;
    /// Only the half of the rectangle on the positive side of the across direction is the gate; a step
    /// through the other half is beside it.
    pub const ONE_SIDE: u32 = 0x08;
    /// The gate is a whole plane: any step that crosses it counts, wherever, with no width or height.
    pub const PLANE: u32 = 0x10;
}

/// One gate, as the crossing test reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    /// [`flags`].
    pub flags: u32,
    /// The point in the middle of the gate's bottom edge (`+0x40`).
    pub centre: Vf,
    /// The direction a rider passes the gate in (`+0x50`); a unit vector on every gate seen.
    pub normal: Vf,
    /// The direction across the gate (`+0x60`), the width measured along it.
    pub across: Vf,
    /// The gate's whole width (`+0x70`). Zero or less: nothing but a plane gate ever crosses it.
    pub width: F,
    /// How high above its centre the gate reaches (`+0x74`).
    pub height: F,
    /// The squared distance from the centre within which an entity is tested at all (`+0x7c`); zero or
    /// less tests at any distance.
    pub reach_sq: F,
}

/// What [`Gate::crossed`] answers, as the original's numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crossing {
    /// The step did not cross the gate's plane, was too far away, or did not move (0).
    Not = 0,
    /// The step passed through the gate (1).
    Through = 1,
    /// The step crossed the gate's plane outside the rectangle — beside it, or above it (2).
    Beside = 2,
    /// The gate is not live ([`flags::CLOSED`] clear) (4).
    Open = 4,
}

/// The FPU's three-term dot product: `mula` x, `madda` y, `madd` z.
fn dot3(a: Vf, b: Vf) -> F {
    F(fpu::madd(fpu::madda(fpu::mula(a.x(), b.x()), a.y(), b.y()), a.z(), b.z()))
}

/// A word as the FPU reads it out of memory (denormals flushed), for a field compared straight from the
/// gate object.
fn read(x: F) -> F {
    F(fpu::operand(x.0.to_bits()))
}

impl Gate {
    /// Did the step from `from` to `to` cross the gate?
    ///
    /// In order: a gate that is not live is [`Crossing::Open`]; an entity starting further from the
    /// centre than the reach is [`Crossing::Not`]; so is one that did not move. The step's crossing of
    /// the gate's plane is the fraction `t` of the way back from `to` to `from` at which it meets it,
    /// and a step that does not cross (`t` outside 0..1) is [`Crossing::Not`]. A [`flags::PLANE`] gate
    /// stops there. Otherwise the meeting point is placed across the gate, as a fraction of its half
    /// width, and above it: outside either is [`Crossing::Beside`]. Through the rectangle, a step
    /// going the gate's way — or either way with [`flags::EITHER_WAY`] — is [`Crossing::Through`], and
    /// one going the other way is [`Crossing::Not`].
    pub fn crossed(&self, from: Vf, to: Vf) -> Crossing {
        if self.flags & flags::CLOSED == 0 {
            return Crossing::Open;
        }
        let reach_sq = read(self.reach_sq);
        if F::ZERO < reach_sq {
            let off = self.centre.sub(from);
            if !(dot3(off, off) <= reach_sq) {
                return Crossing::Not;
            }
        }
        let step = from.sub(to);
        let length = F(fpu::sqrt(dot3(step, step).0));
        if length <= F::ZERO {
            return Crossing::Not;
        }
        let back = self.centre.sub(to);
        let t = F(fpu::div(dot3(back, self.normal).0, length.0));
        if t < F::ZERO || !(t <= F::ONE) {
            return Crossing::Not;
        }
        if self.flags & flags::PLANE != 0 {
            return Crossing::Through;
        }
        let width = read(self.width);
        if width <= F::ZERO {
            return Crossing::Not;
        }
        let at = to.add(step.mul_q(t.0)).sub(self.centre);
        let side = F(fpu::div(fpu::mul(2.0, dot3(at, self.across).0), width.0));
        if self.flags & flags::ONE_SIDE != 0 && side < F::ZERO {
            return Crossing::Beside;
        }
        if !(side <= F::ONE) || side < F(-1.0) {
            return Crossing::Beside;
        }
        let up = F(fpu::sub(at.y(), fpu::mul(read(F(self.across.y())).0, side.0)));
        if !(up <= read(self.height)) {
            return Crossing::Beside;
        }
        if self.flags & flags::EITHER_WAY != 0 || F::ZERO < dot3(step, self.normal) {
            Crossing::Through
        } else {
            Crossing::Not
        }
    }
}

/// Where a rider's score card last saw it, as the start-gate watch keeps it: `now` (`+0x60`) and the
/// tick before (`+0x70`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub now: Vf,
    pub last: Vf,
}

/// A rider the closed start gate caught: the vector it is moved by, and its seconds of no gas (the
/// score card's [`crate::score::Score::gate_penalty`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caught {
    pub push: Vf,
    pub penalty: F,
}

impl Gate {
    /// The race manager's start-gate watch, for one of a card's entities, once a tick while the gate is
    /// live: the card's `now` moves to `last` and the entity's position `at` becomes `now`; a step
    /// from `now` back to `last` that passes [`Gate::crossed`] throws the entity back along its
    /// `facing` by [`gate_push_back`] (the facing's w kept) and gives it [`gate_penalty`]. A gate that
    /// is not live leaves the card alone and draws nothing.
    pub fn watch(&self, seen: &mut Seen, at: Vf, facing: Vf, rng: &mut Rng) -> Option<Caught> {
        if self.flags & flags::CLOSED == 0 {
            return None;
        }
        seen.last = seen.now;
        seen.now = at;
        if self.crossed(seen.now, seen.last) != Crossing::Through {
            return None;
        }
        let push = facing.merge(Dest::XYZ, facing.mul_q(gate_push_back(rng).0));
        Some(Caught { push, penalty: gate_penalty(rng) })
    }
}

/// How far a rider caught through the closed start gate is thrown back: its facing is scaled by this,
/// `-5 - 5 × unit`, so between five and ten feet the other way. One `rand()`.
pub fn gate_push_back(rng: &mut Rng) -> F {
    F(-5.0) - F(5.0) * rng.unit()
}

/// The seconds of no gas a rider caught through the closed start gate gets: `1 + unit`, so one to two.
/// One `rand()`, drawn after [`gate_push_back`]'s.
pub fn gate_penalty(rng: &mut Rng) -> F {
    F::ONE + rng.unit()
}

// --- the activity ----------------------------------------------------------------------------------

/// How many riders a start gate has lanes for.
pub const LANES: usize = 8;

/// The lanes' spacing across the gate, as a fraction of its half width (a global in the original's
/// data, read by its loader).
const LANE_STEP: F = F(0.19);
/// Where lane 0 sits: this far towards the negative end of the across direction, in half widths.
const LANE_EDGE: F = F(0.871);

/// How far behind the start gate the grid stands, along its direction.
const GRID_BACK: f32 = 8.0;
/// The lane spacing, in feet, on a track with no start gate (lane by card index).
const GRID_STEP_NO_GATE: F = F(4.0);

/// The start call is heard this long after the lights go on, and the gate drops at the clip's own
/// drop time after that; the warning lights come on this long before that drop time.
const CALL_AFTER_LIGHTS: F = F(1.5);
/// How long each of the start's light windows lasts.
const WINDOW: F = F(0.1);
/// How long the track intro, when it plays, puts in front of the whole start.
const INTRO: F = F(6.0);

/// The supercross / nationals race activity: the state its construction, reset and grid slots keep.
///
/// The two clip lists (the track intros, the start calls) are the activity's sound objects; the port
/// holds only what the simulation reads from them: how many there are, and each start call's gate-drop
/// time (its `SoundNGateDropTime` key in the track's race file).
#[derive(Clone, Debug, PartialEq)]
pub struct Supercross {
    /// The base activity's race start delay (`+0x10c`, `RaceStartDelay` in the activity's settings):
    /// the seconds from the reset to the gate drop. The reset rewrites it from the start call.
    pub start_delay: F,
    /// `+0x110`: 0.1 from construction and every reset; the start's timeline sets it to −1 the moment
    /// the start call plays.
    pub start_cue: F,
    /// `+0x128`: 1 from construction; nothing in the activity's own code writes it again.
    pub weight: F,
    /// The lanes across the gate (`+0x3bc`), in half widths from the gate's centre: `k × 0.19 − 0.871`,
    /// set by the loader.
    pub lane_offsets: [F; LANES],
    /// Which lane each score card starts in (`+0x418`), by the card's index.
    pub lanes: [i32; LANES],
    /// The race manager's second countdown (`+0x410`, the HUD fader's).
    pub hud_fade: F,
    /// The overlay the race manager arms after the start (`+0x470`), 0 for none, and whether it is
    /// armed (`+0x474`).
    pub hud_stage: i32,
    pub hud_armed: u32,
    /// How many track intros there are (`+0x438`) and the one last played (`+0x478`).
    pub intro_clips: i32,
    pub intro: i32,
    /// Each start call's gate-drop time, in seconds into its clip (`+0x49c`, count `+0x47c`).
    pub drop_times: Vec<F>,
    /// The start call last picked (`+0x4ac`).
    pub call: i32,
    /// The start's two light windows: the lights from `lights_on` to `lights_off` (`+0x4b0`/`+0x4b4`),
    /// the start call 1.5 s after each edge; the warning lights from `warn_on` to `warn_off`
    /// (`+0x4b8`/`+0x4bc`).
    pub lights_on: F,
    pub lights_off: F,
    pub warn_on: F,
    pub warn_off: F,
    /// The race manager's first countdown (`+0x4c0`).
    pub countdown: F,
    /// Set once the track intro has had its turn (`+0x4c4`): a reset then skips it.
    pub intro_seen: u32,
    /// Asks the start's timeline to finish the intro (`+0x4c8`); the intro's start clears it.
    pub intro_queued: u32,
    /// Set by every reset, cleared by the race manager's first countdown (`+0x4cc`).
    pub fresh: u32,
    /// One flag per seat (`+0x4d0`, `+0x4d4`), read by the score cards' announcements.
    pub seat_flags: [u32; 2],
    /// Set at construction, cleared by the race manager's first countdown (`+0x4d8`).
    pub first_start: u32,
}

impl Supercross {
    /// The activity as its constructor leaves it, before the loader has filled the lanes and the clip
    /// lists. The constructor's timeline words are the defaults: lights at 0 and 0.1, warning at 4 and
    /// 4.1.
    pub fn new() -> Supercross {
        Supercross {
            start_delay: F::ZERO,
            start_cue: WINDOW,
            weight: F::ONE,
            lane_offsets: [F::ZERO; LANES],
            lanes: [0; LANES],
            hud_fade: F::ZERO,
            hud_stage: 0,
            hud_armed: 0,
            intro_clips: 0,
            intro: 0,
            drop_times: Vec::new(),
            call: 0,
            lights_on: F::ZERO,
            lights_off: WINDOW,
            warn_on: F(4.0),
            warn_off: F(4.1),
            countdown: F::ZERO,
            intro_seen: 0,
            intro_queued: 0,
            fresh: 0,
            seat_flags: [0; 2],
            first_start: 1,
        }
    }

    /// The loader's lane table: lane `k` at `k × 0.19 − 0.871` half widths, so the eight span the gate
    /// from −0.871 to +0.459 (not centred on it).
    pub fn lane_table() -> [F; LANES] {
        core::array::from_fn(|k| F::from(k as i32) * LANE_STEP - LANE_EDGE)
    }

    /// The activity's reset slot, from the moment it has reset its gates and lines: the start's
    /// timeline set up again, the start gate closed.
    ///
    /// In order: the cue back to 0.1; the start gate's [`flags::CLOSED`] set; a start call picked at
    /// random (never the same one twice running); the gate drop 1.5 s after it plus the clip's drop
    /// time, the lights at 0, the warning 1.5 s before the clip's drop time. If the track intro has not
    /// been seen, one intro is picked the same way and the whole start moves 6 s later. Then the race
    /// manager's counters go back to zero and [`Supercross::fresh`] is set.
    ///
    /// Two `rand()`s with an unseen intro, one without. (The original seeds the generator from the
    /// console's clock first if nothing has seeded it yet; a running game always has.)
    pub fn reset(&mut self, start_gate: &mut Gate, rng: &mut Rng) {
        self.start_cue = WINDOW;
        start_gate.flags |= flags::CLOSED;
        self.call = pick_other(rng, self.drop_times.len() as i32, self.call);
        let drop = self.drop_times[self.call as usize];
        self.start_delay = CALL_AFTER_LIGHTS + drop;
        self.lights_on = F::ZERO;
        self.lights_off = WINDOW;
        self.warn_on = drop - CALL_AFTER_LIGHTS;
        self.warn_off = WINDOW + self.warn_on;
        if self.intro_seen == 0 {
            self.play_intro(rng);
        }
        self.countdown = F::ZERO;
        self.hud_fade = F::ZERO;
        self.hud_stage = 0;
        self.hud_armed = 0;
        self.fresh = 1;
        self.seat_flags = [0; 2];
    }

    /// The track intro: one picked at random, never the last one, and the whole start 6 s later.
    fn play_intro(&mut self, rng: &mut Rng) {
        self.intro = pick_other(rng, self.intro_clips, self.intro);
        self.start_delay = self.start_delay + INTRO;
        self.lights_on = self.lights_on + INTRO;
        self.lights_off = WINDOW + self.lights_on;
        self.warn_on = self.warn_on + INTRO;
        self.warn_off = WINDOW + self.warn_on;
        self.intro_queued = 0;
    }

    /// The grid slot's lanes: card `i` in lane `i`, then — unless every rider is AI (the `AllAI` knob,
    /// off on retail) — `riders` passes of a bubble along the lanes, each neighbouring pair swapped on
    /// a coin toss (`unit < 0.5`). `riders × (riders − 1)` `rand()`s.
    pub fn shuffle_lanes(&mut self, riders: usize, all_ai: bool, rng: &mut Rng) {
        for (i, lane) in self.lanes.iter_mut().enumerate().take(riders) {
            *lane = i as i32;
        }
        if all_ai {
            return;
        }
        for _ in 0..riders {
            for j in 0..riders.saturating_sub(1) {
                if rng.unit() < F(0.5) {
                    self.lanes.swap(j, j + 1);
                }
            }
        }
    }

    /// Where the grid slot puts card `card` of `riders`, and the way it faces, before the ground snaps
    /// the point onto the track.
    ///
    /// The facing is the gate's direction laid flat (y zeroed, `w` kept) and made unit length. The
    /// riders' lanes are centred on the eight: the card's lane plus half of the lanes left over
    /// (rounded towards zero). The spot is 8 ft behind the gate's centre, then out along the flat
    /// across direction (the facing turned a quarter right) by the lane's offset times half the gate's
    /// width. With no start gate it is the track's start point, out by 4 ft per card index.
    pub fn grid_spot(&self, start: GridStart, card: i32, riders: i32) -> (Vf, Vf) {
        match start {
            GridStart::Gate { centre, normal, width } => {
                let facing = flat_unit(normal);
                let left_over = LANES as i32 - riders;
                let lane = self.lanes[card as usize] + left_over / 2;
                let out = F(0.5) * (self.lane_offsets[lane as usize] * read(width));
                let back = facing.merge(Dest::XYZ, facing.mul_q(GRID_BACK));
                let at = centre.merge(Dest::XYZ, centre.sub(back));
                (beside(at, facing, out), facing)
            }
            GridStart::Point { at, facing } => {
                let out = GRID_STEP_NO_GATE * F::from(card);
                let facing = flat_unit(facing);
                (beside(at, facing, out), facing)
            }
        }
    }

    /// The place a card holds on the grid, before anyone has crossed a line: its lane, counted from 1.
    pub fn starting_place(&self, card: i32) -> i32 {
        self.lanes[card as usize] + 1
    }
}

impl Default for Supercross {
    fn default() -> Supercross {
        Supercross::new()
    }
}

/// What the grid stands behind: the start gate (its centre, direction and width), or on a track with
/// none, the track's start point and its facing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridStart {
    Gate { centre: Vf, normal: Vf, width: F },
    Point { at: Vf, facing: Vf },
}

/// A clip picked at random from `count`, stepping past the one played last time (wrapping to 0).
fn pick_other(rng: &mut Rng, count: i32, last: i32) -> i32 {
    let i = rng.index(count);
    if i != last {
        return i;
    }
    if i + 1 < count { i + 1 } else { 0 }
}

/// A direction laid flat and made unit: y zeroed, then the VU0's normalise (`w` kept).
fn flat_unit(v: Vf) -> Vf {
    unit(Vf([v.x(), 0.0, v.z(), v.w()]))
}

/// `at` moved `out` along the flat right of `facing`: x by `facing.z × out`, z by `−facing.x × out`.
fn beside(at: Vf, facing: Vf, out: F) -> Vf {
    let x = F(at.x()) + F(facing.z()) * out;
    let z = F(at.z()) - F(facing.x()) * out;
    Vf([x.0, at.y(), z.0, at.w()])
}

// --- the lap line (S13c) ---------------------------------------------------------------------------

/// A score card's flag bits (its `+8` doubleword) as the lap line reads them.
pub mod card_flags {
    /// The card is done: its race is over, by the line or by its time running out.
    pub const OUT: u64 = 0x01;
    /// The card takes part in the race; the race kind's handler skips a card without it. A ghost race
    /// sets it on the ghost's card once the ghost has a lap to show.
    pub const TAKING_PART: u64 = 0x02;
    /// The card takes no line test: [`super::LapRules::line`] answers yes without looking (what sets
    /// it was not traced).
    pub const NO_LINE: u64 = 0x10;
    /// Cleared by every lap a fast-lap attack counts (what it means there was not traced).
    pub const FAST_LAP: u64 = 0x100;
    /// The rider is behind the line it starts from: its first crossing starts the race and counts no
    /// lap.
    pub const BEHIND_LINE: u64 = 0x1_0000;
    /// This lap cannot be the best lap; every crossing that passes the line clears it.
    pub const NO_BEST: u64 = 0x2_0000;
}

/// The score manager's flag bits (its `+0x5c` word) as the lap line reads and writes them.
pub mod race_flags {
    /// A card has finished by the line.
    pub const FINISHED: u32 = 0x10;
    /// Every counted lap is kept in the card's list of lap records.
    pub const KEEP_LAPS: u32 = 0x400;
}

/// The score manager's race kind (its `+0x60`). The numbers are the event's activity types; the sx01
/// single race holds 2.
pub mod race_kind {
    pub const PRACTICE: i32 = 1;
    pub const SINGLE_RACE: i32 = 2;
    /// A ghost race: every lap starts the rider's run over, and the card's owner carries the ghost's
    /// card.
    pub const GHOST_RACE: i32 = 3;
    pub const FAST_LAP: i32 = 4;
}

/// A place on the track's path, as the track's path queries hand it back (three words: two that pick
/// the stretch of path, raw, and the distance along it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spot {
    pub path: u32,
    pub piece: u32,
    pub along: F,
}

/// One kept lap (a 0x18-byte record on the card's list, newest first).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LapRecord {
    /// The lap's time and the race clock at its start (raw copies of the card's).
    pub time: F,
    pub start: F,
    /// A copy of the card's `+0x108` word (not identified).
    pub word: u32,
    /// Which lap this was: the card's lap count once it was counted.
    pub lap: i32,
    /// The card's place when it crossed.
    pub place: i32,
}

/// A score card's laps, as the race's line tests keep them. The offsets are the card's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LapCard {
    /// [`card_flags`].
    pub flags: u64,
    /// The path distance to the line from [`LapCard::kept_spot`] (`+0x10`) and from
    /// [`LapCard::spot`] (`+0x14`).
    pub to_line_kept: F,
    pub to_line: F,
    /// How far into the race the rider is, along the path (`+0x1c`).
    pub progress: F,
    /// The path length of the laps already behind it (`+0x24`) and of this one (`+0x28`).
    pub laps_length: F,
    pub lap_length: F,
    /// The card's place (`+0x2c`).
    pub place: i32,
    /// Laps counted (`+0x30`), and in a ghost race the runs started (`+0x34`).
    pub laps: i32,
    pub runs: i32,
    /// Whether the rider is on the track, by the path (`+0x40`; 0 off).
    pub on_track: i32,
    /// The last lap's time (`+0x48`), the race clock at this lap's start (`+0x4c`), the best lap
    /// (`+0x50`, 0 for none yet) and which lap it was (`+0x54`, counted from 0).
    pub lap_time: F,
    pub lap_start: F,
    pub best_lap: F,
    pub best_lap_no: i32,
    /// Where the card saw its rider, now and the tick before (`+0x60`, `+0x70`).
    pub seen: Seen,
    /// Where on the path the rider is (`+0x88`), and the spot the path tests keep (`+0x94`).
    pub spot: Spot,
    pub kept_spot: Spot,
    /// The race clock (`+0xb4`) and the lap's own clock (`+0xb8`).
    pub race_time: F,
    pub lap_clock: F,
    /// `+0x108`, copied into each lap record.
    pub record_word: u32,
    /// One word per line on the track (`+0xe4`, the manager's line count long), cleared by every
    /// counted lap. Empty: the card has none.
    pub lines_passed: Vec<u32>,
    /// The kept laps (`+0x1bc`), newest first.
    pub records: Vec<LapRecord>,
}

/// The track as the lap line reads it: the line, the spot on the path where it stands, and the
/// path's length round one lap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Course {
    pub line: Gate,
    pub line_spot: Spot,
    pub lap_length: F,
}

/// The score manager's words the lap line reads and writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LapRules {
    /// [`race_flags`] (`+0x5c`).
    pub flags: u32,
    /// [`race_kind`] (`+0x60`).
    pub kind: i32,
    /// The laps a race runs (`+0x94`).
    pub laps: i32,
    /// The runs a ghost race needs before its ghost counts (`+0xf0`).
    pub ghost_runs: i32,
    /// How many lap records there are in all (`+0x4c`), and how many are still free (the pool at
    /// `+0x58`).
    pub record_limit: i32,
    pub free_records: u32,
}

/// What the lap line tells the card's listeners, in the order it tells them. The race's presentation
/// (the lap board, the flags, the announcer) hangs off these; the line's own work is in the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineNotice {
    /// The rider went past the line beside it.
    Beside,
    /// The rider went through the line.
    Through,
    /// A ghost race's first crossing, from behind the line.
    Started,
    /// The lap just counted is the card's best.
    BestLap,
    /// The card has run its laps.
    Finished,
    /// A ghost race's run starts over; the ghost's card is handed over with it.
    RunOver,
    /// The run just over is the ghost's best.
    GhostBest,
    /// The ghost has run enough to race against.
    GhostReady,
    /// A lap was counted.
    Lap,
}

/// A real-valued word compared as the FPU reads it.
fn is_zero(x: F) -> bool {
    read(x) == F::ZERO
}

impl LapRules {
    /// The score card's lap line, once a tick for a card the race kind's handler finds near it: did
    /// the rider's step from `card.seen.now` back to `card.seen.last` pass `course.line`, and what
    /// that counts. True when the step went through the line (or the card takes no line test, or the
    /// lap could not be counted because the clock ran backwards: false then); the handler moves the
    /// card's spot onto the line on true.
    ///
    /// A step beside the line only tells the listeners. Through it, a card [`card_flags::BEHIND_LINE`]
    /// starts its race; otherwise a lap is counted: its time is the race clock less the lap's start
    /// (a negative time is held at 0 and counts nothing more), a best lap if the card has none or it
    /// is quicker (and the lap is not [`card_flags::NO_BEST`]), the lap's start moved to now, a lap
    /// record kept if the race keeps them, and — in a race that ends at the line — the card
    /// [`card_flags::OUT`] once it has its laps. A ghost race instead starts the rider's run over and
    /// hands its time to the ghost's card (`ghost`, the card's owner's). The card's line words are
    /// cleared and [`card_flags::NO_BEST`] with them.
    pub fn line(&mut self, card: &mut LapCard, ghost: Option<&mut LapCard>, course: &Course) -> (bool, Vec<LineNotice>) {
        let mut said = Vec::new();
        if card.flags & card_flags::NO_LINE != 0 {
            return (true, said);
        }
        match course.line.crossed(card.seen.now, card.seen.last) {
            Crossing::Through => {}
            Crossing::Beside => {
                said.push(LineNotice::Beside);
                return (false, said);
            }
            _ => return (false, said),
        }
        if card.flags & card_flags::BEHIND_LINE != 0 {
            card.flags &= !card_flags::BEHIND_LINE;
            if self.kind == race_kind::GHOST_RACE {
                said.push(LineNotice::Started);
            }
            card.flags &= !card_flags::NO_BEST;
            return (true, said);
        }
        said.push(LineNotice::Through);
        card.laps = card.laps.wrapping_add(1);
        card.lap_clock = F::ZERO;
        let time = F(fpu::sub(card.race_time.0, card.lap_start.0));
        card.lap_time = time;
        if time < F::ZERO {
            card.lap_time = F::ZERO;
            return (false, said);
        }
        if card.flags & card_flags::NO_BEST == 0 && (is_zero(card.best_lap) || time < read(card.best_lap)) {
            card.best_lap = time;
            card.best_lap_no = if self.kind == race_kind::GHOST_RACE { card.runs } else { card.laps.wrapping_sub(1) };
            said.push(LineNotice::BestLap);
        }
        card.lap_start = card.race_time;
        if self.flags & race_flags::KEEP_LAPS != 0 {
            self.keep_lap(card);
        }
        if self.kind != race_kind::GHOST_RACE && self.kind != race_kind::SINGLE_RACE && self.laps <= card.laps {
            card.flags |= card_flags::OUT;
            self.flags |= race_flags::FINISHED;
            said.push(LineNotice::Finished);
        }
        if self.kind == race_kind::GHOST_RACE {
            if let Some(g) = ghost {
                self.run_over(card, g, course, &mut said);
            }
        } else if self.kind == race_kind::FAST_LAP {
            card.flags &= !card_flags::FAST_LAP;
        }
        card.lines_passed.fill(0);
        said.push(LineNotice::Lap);
        card.flags &= !card_flags::NO_BEST;
        (true, said)
    }

    /// A counted lap kept on the card's list: a free record while the lap is under the pool's size,
    /// else the card's oldest one taken back. (Past the pool with no records of its own the original
    /// writes the record through a stale register; no race reaches it, and the port keeps nothing.)
    fn keep_lap(&mut self, card: &mut LapCard) {
        let lap = if self.kind == race_kind::GHOST_RACE { card.runs.wrapping_add(1) } else { card.laps };
        let had = if lap < self.record_limit {
            if self.free_records == 0 {
                return;
            }
            self.free_records -= 1;
            true
        } else {
            card.records.pop().is_some()
        };
        if had {
            let r = LapRecord {
                time: card.lap_time,
                start: card.lap_start,
                word: card.record_word,
                lap,
                place: card.place,
            };
            card.records.insert(0, r);
        }
    }

    /// A ghost race's lap: the rider's run starts over, the ghost's card takes its best time, and is
    /// set back on the line ready to go round with it.
    fn run_over(&mut self, card: &mut LapCard, g: &mut LapCard, course: &Course, said: &mut Vec<LineNotice>) {
        card.lap_clock = F::ZERO;
        card.lap_start = F::ZERO;
        card.race_time = F::ZERO;
        card.progress = F::ZERO;
        card.laps_length = F::ZERO;
        card.laps = 0;
        card.runs = card.runs.wrapping_add(1);
        said.push(LineNotice::RunOver);
        let best = read(card.best_lap);
        if (best < read(g.best_lap) || is_zero(g.best_lap)) && !is_zero(card.best_lap) {
            g.best_lap = card.best_lap;
            said.push(LineNotice::GhostBest);
        }
        g.lap_time = F::ZERO;
        g.lap_clock = F::ZERO;
        g.lap_start = F::ZERO;
        g.race_time = F::ZERO;
        g.progress = F::ZERO;
        g.laps_length = F::ZERO;
        g.lap_length = course.lap_length;
        g.laps = 0;
        g.kept_spot = Spot { along: F(fpu::add(course.line_spot.along.0, LINE_NUDGE.0)), ..course.line_spot };
        g.spot = g.kept_spot;
        g.to_line = course.lap_length;
        g.on_track = 1;
        if card.runs >= self.ghost_runs && !is_zero(g.best_lap) {
            g.flags |= card_flags::TAKING_PART;
            said.push(LineNotice::GhostReady);
        }
    }
}

/// How far past the line's own spot a card set back on the line is put, so that it stands beyond it.
const LINE_NUDGE: F = F(1e-5);
