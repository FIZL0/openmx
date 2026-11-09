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
//! While the countdown runs, [`Supercross::start_timeline`] (S13f) walks the start's windows: the
//! intro's end and the race overlays, the start call, and the warning that lets the held bikes go.
//!
//! Then the start itself (S13b), [`Race`]: the race manager's tick ahead of the riders — its music and
//! tip clocks, the watch over every rider while the gate is closed, and the score manager's countdown,
//! whose last tick drops the gate. A caught rider's bike is stopped dead and sent back along its nose
//! ([`push`]); at the drop every bike is let go ([`release`]). [`crate::Sim::race`] holds one on a race.
//!
//! Once the race is on, each card's crossings of the finish line count its laps: [`LapRules::line`] is
//! the score card's lap line (S13c) — the lap's time, the best lap, the kept lap records, the finish in
//! a race that ends at the line, and a ghost race's run starting over — with what it tells the
//! presentation as [`LineNotice`]s.
//!
//! The race's finish (S294) is the rest of the score manager's race handler around the cards' path
//! work: [`LapRules::check_over`] opens each tick (the race is over once no player is racing),
//! [`LapRules::clocks`] and [`LapRules::time_limit`] run each card's clocks — a card still out on the
//! track when the race ends gets its finish made up from its gap — and [`LapRules::close`] ends it: the
//! finish countdown once a player is done, the finishers' times drawn apart once, then the ranking
//! ([`LapRules::rank`]) and the gaps ([`gaps`], or a ghost race's [`LapRules::ghost_gap`]), with what
//! they tell as [`FinishNotice`]s.
//!
//! Where the gates stand comes off the disc (S13e): every track carries a race file, which
//! [`TrackFile::parse`] reads as the track object's loader does — its paths of nodes with their spans
//! and links, the checkpoint records and their parts, the grid gate, the finish line and a third gate.
//! A supercross race then stands its grid and start gate on the start-drop prop
//! ([`Supercross::place_drop_gate`]) and its finish line on the finish prop
//! ([`Supercross::place_finish_line`]), each handed to it as a [`Prop`].
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
    /// (`+0x4b8`/`+0x4bc`). [`Supercross::start_timeline`] reads them.
    pub lights_on: F,
    pub lights_off: F,
    pub warn_on: F,
    pub warn_off: F,
    /// The race manager's first countdown (`+0x4c0`).
    pub countdown: F,
    /// Set once the track intro has had its turn (`+0x4c4`): a reset then skips it.
    pub intro_seen: u32,
    /// Set once the start's timeline has ended the track intro (`+0x4c8`): the intro's start clears
    /// it, and while it is clear the lights window ends the intro ([`Supercross::start_timeline`]).
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
    /// a coin toss (`unit < 0.5`). `riders × (riders − 1)` `rand()`s. With `AllAI` set there are none:
    /// the grid shows the race overlays instead, and skips the vehicles' and riders' resets too.
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

// --- the start's timeline (S13f) -------------------------------------------------------------------

/// A rider's race interface (an actor of class `MXSupercrossInterface`) as the start's timeline drives
/// it: the body it may carry (`+0x68`; on sx01's image, the human rider's bike chassis) and the four
/// words it points at (`+0x08`, `+0x0c`, `+0x10`, `+0x14`). The drop writes the second and third
/// again (1, 0); what reads the words was not traced.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StartItem {
    /// The body's axis freedoms ([`crate::chassis::Chassis::free`], `ang_free`), when it has a body.
    pub body: Option<Axes>,
    pub words: [i32; 4],
}

/// A body's freedom to move along each world axis and to turn about each of its own (`+0x2a0..`,
/// `+0x2ac..` of a chassis).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Axes {
    pub free: [i32; 3],
    pub ang_free: [i32; 3],
}

/// What one call of [`Supercross::start_timeline`] did beyond the activity and its items: each a cue
/// for the presentation, none read again by a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StartStep {
    /// The clock read zero: every item with a body was held for the start (the start hold: its bike
    /// may move only in height).
    pub armed: bool,
    /// The track intro was ended: the scene's intro cameras stop and the intro clip, if it still
    /// plays, fades out over 750 ms. The score manager's countdown has its interval accumulator set
    /// to 0, and its elapsed time to [`StartStep::elapsed`] when that is set.
    pub intro_ended: bool,
    /// Where the intro's end puts the countdown's elapsed time: 6 s when the intro had not been seen
    /// and the countdown was short of that (the start jumps to the end of the intro), 0 when it had
    /// been seen (the countdown starts over).
    pub elapsed: Option<F>,
    /// The lights window: the race overlays come on (one, or one per half of a split screen).
    pub overlays: bool,
    /// The start call window: the picked start call plays.
    pub call: bool,
    /// The warning window: every item's body was freed on every axis.
    pub warning: bool,
}

impl Supercross {
    /// **The start's timeline**, the activity's listener on the score manager's countdown: `time` is
    /// the countdown's elapsed time, and the manager calls this on the countdown's first tick and each
    /// time its interval accumulator reaches the interval (0.1 s) again. `intro_camera` is whether the
    /// scene's intro camera is the one showing. The countdown itself is the score manager's
    /// ([`Countdown`]): what the intro's end does to it comes back in the [`StartStep`].
    ///
    /// When the clock reads zero (or minus zero), each item with a body is held for the start: the
    /// body free to move only in height and to turn only about x and z, its words 0, 0, 1 and
    /// `intro_camera`. Then the first window the clock is in, of three, each from its start up to (not
    /// including) its end:
    ///
    /// - the lights, `lights_on..lights_off`: the race overlays show, and the intro is ended first if
    ///   it has not been ([`Supercross::intro_queued`] clear);
    /// - the start call, the same window 1.5 s later: the call plays and the cue reads −1;
    /// - the warning, `warn_on..warn_off`: each item with a body is freed on every axis, its first
    ///   word 0 and its second 1.
    ///
    /// No `rand()`.
    pub fn start_timeline(&mut self, time: F, items: &mut [StartItem], intro_camera: bool) -> StartStep {
        let mut step = StartStep::default();
        let t = read(time);
        if t == F::ZERO {
            step.armed = true;
            for item in items.iter_mut() {
                if let Some(body) = &mut item.body {
                    body.free = [0, 1, 0];
                    body.ang_free = [1, 0, 1];
                    item.words[..3].copy_from_slice(&[0, 0, 1]);
                    item.words[3] = intro_camera as i32;
                }
            }
        }
        let on = read(self.lights_on);
        if !(t < on) && t < read(self.lights_off) {
            if self.intro_queued == 0 {
                step.elapsed = self.end_intro(t, items);
                step.intro_ended = true;
            }
            step.overlays = true;
        } else if !(t < F(fpu::add(CALL_AFTER_LIGHTS.0, on.0)))
            && t < F(fpu::add(CALL_AFTER_LIGHTS.0, self.lights_off.0))
        {
            step.call = true;
            self.start_cue = -F::ONE;
        } else if !(t < read(self.warn_on)) && t < read(self.warn_off) {
            step.warning = true;
            for item in items.iter_mut() {
                if let Some(body) = &mut item.body {
                    body.free = [1; 3];
                    body.ang_free = [1; 3];
                    item.words[0] = 0;
                    item.words[1] = 1;
                }
            }
        }
        step
    }

    /// The intro's end, at the countdown's `elapsed`: the countdown's new elapsed time, if it moves
    /// (6 s when the intro had not been seen and it is short of that, 0 when it had been); the intro
    /// marked seen and ended; every item's fourth word 0, body or not.
    fn end_intro(&mut self, elapsed: F, items: &mut [StartItem]) -> Option<F> {
        let to = if self.intro_seen != 0 {
            Some(F::ZERO)
        } else if read(elapsed) < INTRO {
            Some(INTRO)
        } else {
            None
        };
        self.intro_seen = 1;
        self.intro_queued = 1;
        for item in items.iter_mut() {
            item.words[3] = 0;
        }
        to
    }
}

/// The start's countdown as the score manager runs it: how long it lasts (`+0xbc` of the manager)
/// and how long it has run (`+0xc4`). While it runs, the manager adds the tick's `dt` to `elapsed`
/// each tick until `elapsed` is no longer under `length`; that tick ends it and drops the gate
/// ([`Race::tick`]). The beeps and the light it shows on the way (every `+0xc0` seconds) are the
/// front end's and change nothing a tick reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Countdown {
    pub length: F,
    pub elapsed: F,
    /// Still counting (the manager's `0x80` flag); cleared by the tick that drops the gate.
    pub running: bool,
}

/// A race's start (S13b): the race manager's tick as far as a tick reads it — its two clocks, the
/// start gate's watch over every rider while it is closed, and the countdown that opens it.
#[derive(Clone, Debug, PartialEq)]
pub struct Race {
    /// The start gate. [`flags::CLOSED`] until the drop.
    pub gate: Gate,
    /// Where each rider's score card last saw it, in player order: the watch's memory.
    pub cards: Vec<Seen>,
    /// The music's clock (`+0x4c0`): counted down while above zero, and the track starts when it runs
    /// out. The drop sets it to one second.
    pub music_in: F,
    /// The music clock is held: the track is still loading, or the race is paused. The front end's to
    /// say; a tick only reads it.
    pub music_held: bool,
    /// The clutch tip's clock (`+0x410`): counted down while above zero, and the "preload and clutch"
    /// tip shows when it runs out.
    pub tip_in: F,
    /// The length of the race's lesson stage, when it has one (`+0x470` names it, its `+0x24` is the
    /// length): with the tip clock at zero and not yet armed, it arms it to show the tip 2.05 s before
    /// the stage's end, but never sooner than a thousandth of a second from now.
    pub stage: Option<F>,
    /// The tip clock has been armed from the stage (`+0x474`); it is armed once.
    pub tip_armed: bool,
    pub countdown: Countdown,
    /// The word the drop draws for the scene (its `+0xf0`), eight to thirteen seconds.
    pub scene_clock: F,
    /// The race's draws: the watch's two per rider caught and the drop's one.
    pub rng: Rng,
}

/// What one [`Race::tick`] did that is not in the race itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// The music clock ran out: the track starts.
    pub music: bool,
    /// The tip clock ran out: the "preload and clutch" tip shows.
    pub tip: bool,
    /// The riders the closed gate caught, by player, in the order it caught them: each is to be
    /// [`push`]ed and given the penalty on its score card.
    pub caught: Vec<(usize, Caught)>,
    /// The countdown ended and the gate dropped: every rider is to be [`release`]d.
    pub dropped: bool,
}

impl Race {
    /// A race waiting at its start: the gate closed, every rider's card seeing it where it stands
    /// (`riders`, as [`Race::tick`] takes them), and a countdown of `length` seconds to run.
    pub fn new(mut gate: Gate, riders: &[(Vf, Vf)], length: F, rng: Rng) -> Race {
        gate.flags |= flags::CLOSED;
        Race {
            gate,
            cards: riders.iter().map(|&(at, _)| Seen { now: at, last: at }).collect(),
            music_in: F::ZERO,
            music_held: false,
            tip_in: F::ZERO,
            stage: None,
            tip_armed: false,
            countdown: Countdown { length, elapsed: F::ZERO, running: true },
            scene_clock: F::ZERO,
            rng,
        }
    }

    /// **The race manager's tick**, before the riders': each rider in `riders` is where its bike is
    /// and which way it faces (its transform's translation and forward row), in player order.
    ///
    /// In the original's order: the music clock, which only runs while above zero and not held, and
    /// starts the track on the tick it reaches zero or below (then reads zero); the gate's watch over
    /// every card while the gate is closed ([`Gate::watch`]); the tip clock, which runs the same way
    /// — or, at zero, is armed once from the lesson stage; and then the score manager's countdown,
    /// whose last tick drops the gate ([`Race::drop`]). The watch runs before the drop, so a rider
    /// through the gate on the drop's own tick is still caught.
    pub fn tick(&mut self, dt: F, riders: &[(Vf, Vf)]) -> Frame {
        let mut frame = Frame::default();
        if !(read(self.music_in) <= F::ZERO) && !self.music_held {
            self.music_in = F(fpu::sub(self.music_in.0, dt.0));
            if self.music_in <= F::ZERO {
                frame.music = true;
                self.music_in = F::ZERO;
            }
        }
        // A rider who joined since the start is seen where it stands first.
        for &(at, _) in &riders[self.cards.len().min(riders.len())..] {
            self.cards.push(Seen { now: at, last: at });
        }
        if self.gate.flags & flags::CLOSED != 0 {
            for (i, &(at, facing)) in riders.iter().enumerate() {
                if let Some(hit) = self.gate.watch(&mut self.cards[i], at, facing, &mut self.rng) {
                    frame.caught.push((i, hit));
                }
            }
        }
        if !(read(self.tip_in) <= F::ZERO) {
            self.tip_in = F(fpu::sub(self.tip_in.0, dt.0));
            if self.tip_in <= F::ZERO {
                frame.tip = true;
                self.tip_in = F::ZERO;
            }
        } else if let Some(length) = self.stage
            && !self.tip_armed
            && read(length) != F::ZERO
        {
            self.tip_armed = true;
            let lead = F(fpu::sub(fpu::sub(length.0, 0.75), 1.3));
            self.tip_in = F(0.001).max(lead);
        }
        let c = &mut self.countdown;
        if c.running {
            if !(read(c.elapsed) < read(c.length)) {
                c.running = false;
                self.drop();
                frame.dropped = true;
            }
            let c = &mut self.countdown;
            c.elapsed = F(fpu::add(c.elapsed.0, dt.0));
        }
        frame
    }

    /// **The drop**, the countdown's end: the music clock set to a second, the gate opened, and the
    /// scene's word drawn, `8 + 5 × unit`. The riders are released by whoever holds them
    /// ([`release`]).
    pub fn drop(&mut self) {
        self.music_in = F::ONE;
        self.gate.flags &= !flags::CLOSED;
        self.scene_clock = F(8.0) + F(5.0) * self.rng.unit();
    }
}

/// A caught rider's bike thrown back: the bike's message for it stops the chassis dead
/// ([`crate::chassis::stop`]) and then makes the push its velocity, `w` and all. The bike is not
/// moved; it rolls back from where it is.
pub fn push(ch: &mut crate::chassis::Chassis, push: Vf) {
    crate::chassis::stop(ch);
    ch.vel = push;
}

/// A rider's bike let go at the drop, the message every card's bike gets after the gate opens: the
/// chassis may move along and turn about every axis again. (A computer rider's also redraws its
/// start; the port's riders are all players.)
pub fn release(ch: &mut crate::chassis::Chassis) {
    ch.free = [1; 3];
    ch.ang_free = [1; 3];
}

// --- the lap line (S13c) ---------------------------------------------------------------------------

/// A score card's flag bits (its `+8` doubleword) as the lap line reads them.
pub mod card_flags {
    /// The card is done: its race is over, by the line or by its time running out.
    pub const OUT: u64 = 0x01;
    /// The card takes part in the race; the race kind's handler skips a card without it. A ghost race
    /// sets it on the ghost's card once the ghost has a lap to show.
    pub const TAKING_PART: u64 = 0x02;
    /// A player's card (by its uses: a ghost race measures its gap to the ghost, and its finish starts
    /// the race's finish countdown). What sets it was not traced.
    pub const PLAYER: u64 = 0x04;
    /// The card takes no line test: [`super::LapRules::line`] answers yes without looking (what sets
    /// it was not traced).
    pub const NO_LINE: u64 = 0x10;
    /// Cleared by every lap a fast-lap attack counts (what it means there was not traced).
    pub const FAST_LAP: u64 = 0x100;
    /// A fast-lap attack runs the card's attack clock (what sets it was not traced).
    pub const ATTACK_CLOCK: u64 = 0x200;
    /// The card's time limit ran out ([`super::LapRules::time_limit`]); it is out of the race like
    /// [`OUT`].
    pub const TIMED_OUT: u64 = 0x4000;
    /// Counts as a player's card for the finish, like [`PLAYER`] (what sets it was not traced).
    pub const PLAYER_TOO: u64 = 0x8000;
    /// The rider is behind the line it starts from: its first crossing starts the race and counts no
    /// lap.
    pub const BEHIND_LINE: u64 = 0x1_0000;
    /// This lap cannot be the best lap; every crossing that passes the line clears it.
    pub const NO_BEST: u64 = 0x2_0000;
    /// The card's time limit is held: it does not run down.
    pub const CLOCK_HELD: u64 = 0x4_0000;
    /// A player's card that does not keep the race open (what sets it was not traced).
    pub const STANDS_ASIDE: u64 = 0x40_0000;
}

/// The score manager's flag bits (its `+0x5c` word) as the lap line reads and writes them.
pub mod race_flags {
    /// The race is over: no player's card is still racing, or the finish countdown ran out.
    pub const OVER: u32 = 0x02;
    /// A player's card is done: the finish countdown runs.
    pub const PLAYER_DONE: u32 = 0x04;
    /// A card has finished by the line.
    pub const FINISHED: u32 = 0x10;
    /// Every counted lap is kept in the card's list of lap records.
    pub const KEEP_LAPS: u32 = 0x400;
    /// No rider the race waits for is still racing (set by the handler; S293).
    pub const ALL_OUT: u32 = 0x02;
    /// A rider the race waits for has finished by the line (S293).
    pub const PLAYER_OUT: u32 = 0x04;
    /// The race's end has been told: the finishers' times are drawn apart once.
    pub const OVER_TOLD: u32 = 0x2000;
    /// The cards are ranked every tick.
    pub const RANKED: u32 = 0x4000;
    /// The zone list is live: the handlers ask [`super::zone_crossings`] of every rider's step (S292).
    pub const ZONES: u32 = 0x200;
}

/// The score manager's race kind (its `+0x60`). Not the event's activity type: a live sx01 single race
/// (activity type 2) holds 1 and a practice (activity type 1) holds 2 (C31).
pub mod race_kind {
    /// A race that ends at its laps: the lap line finishes each card on its last lap.
    pub const RACE: i32 = 1;
    /// Laps with no end: the lap line never finishes a card, and the race is never over by itself.
    pub const PRACTICE: i32 = 2;
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
    /// How far behind the leader the card is, in seconds (`+0xbc`): the leader's own is less the
    /// second card's.
    pub gap: F,
    /// How long the rider has been off the track (`+0xc0`).
    pub off_track: F,
    /// What is left of the card's time limit (`+0xc4`; 0 for none).
    pub time_left: F,
    /// A fast-lap attack's clock (`+0x168`).
    pub attack_clock: F,
    /// `+0x108`, copied into each lap record.
    pub record_word: u32,
    /// One word per line on the track (`+0xe4`, the manager's line count long), cleared by every
    /// counted lap. Empty: the card has none.
    pub lines_passed: Vec<u32>,
    /// The kept laps (`+0x1bc`), newest first.
    pub records: Vec<LapRecord>,
    /// The gate race's next gate to pass, an index into its chain of gates (`+0x80`; `None` for none),
    /// the gates passed this lap (`+0x84`) and in the whole run (`+0x38`). Only
    /// [`LapRules::gate_line`] reads them.
    pub gate: Option<usize>,
    pub gates_this_lap: i32,
    pub gates_passed: i32,
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
    /// The finish countdown: how long it runs once a player is done (`+0x9c`, 30 s on sx01; 0 none)
    /// and how far it has run (`+0x98`).
    pub finish_wait: F,
    pub finish_clock: F,
    /// Whether the cards' time limits run (`+0xa0` above 0).
    pub time_limit: F,
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
    /// The rider went through the gate race's next gate ([`LapRules::gate_line`] only).
    Gate,
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
        if self.kind != race_kind::GHOST_RACE && self.kind != race_kind::PRACTICE && self.laps <= card.laps {
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

// --- the gate race's line and the zones (S292) ------------------------------------------------------

impl LapRules {
    /// The gate race's line (the score manager's machine-race handler, and four activities of their
    /// own, call it once a tick per card): the course is a chain of gates, `gates` in order, and the
    /// card waits at [`LapCard::gate`]. True unless the card waits at no gate, or the lap could not be
    /// counted because the clock ran backwards.
    ///
    /// Through the gate, the path length of the laps behind the card grows by the straight distance
    /// from that gate to the next one (to the first one from the last), and the card waits at the
    /// next. Through the last gate a lap is counted much as [`LapRules::line`] counts one — its time,
    /// a best lap when it is quicker or the card has none (zero or less), the lap's start, a lap
    /// record — but no flag holds the lap back, and a ghost race starts the run over from the first
    /// gate with the lap's length behind it (see [`LapRules::gate_run_over`]). Through any other gate,
    /// a fast-lap attack clears [`card_flags::FAST_LAP`]. Beside the gate only tells the listeners.
    /// Whatever the step did (but a backwards clock), in a race that ends at the line a card with its
    /// laps is [`card_flags::OUT`] again, and the listeners hear it again, every call.
    pub fn gate_line(&mut self, card: &mut LapCard, ghost: Option<&mut LapCard>, gates: &[Gate]) -> (bool, Vec<LineNotice>) {
        let mut said = Vec::new();
        let Some(at) = card.gate else { return (false, said) };
        // The race clock is read before anything is told.
        let race_time = card.race_time;
        match gates[at].crossed(card.seen.now, card.seen.last) {
            Crossing::Through => {
                said.push(LineNotice::Through);
                let next = (at + 1 < gates.len()).then_some(at + 1);
                let to = gates[next.unwrap_or(0)].centre.sub(gates[at].centre);
                card.laps_length = F(fpu::add(card.laps_length.0, fpu::sqrt(dot3(to, to).0)));
                card.gate = next;
                card.gates_this_lap = card.gates_this_lap.wrapping_add(1);
                card.gates_passed = card.gates_passed.wrapping_add(1);
                said.push(LineNotice::Gate);
                if next.is_none() {
                    card.gate = Some(0);
                    card.gates_this_lap = 0;
                    card.laps = card.laps.wrapping_add(1);
                    card.lap_clock = F::ZERO;
                    let time = F(fpu::sub(race_time.0, card.lap_start.0));
                    card.lap_time = time;
                    if time < F::ZERO {
                        card.lap_time = F::ZERO;
                        return (false, said);
                    }
                    if time < read(card.best_lap) || read(card.best_lap) <= F::ZERO {
                        card.best_lap = time;
                        card.best_lap_no =
                            if self.kind == race_kind::GHOST_RACE { card.runs } else { card.laps.wrapping_sub(1) };
                        said.push(LineNotice::BestLap);
                    }
                    card.lap_start = race_time;
                    if self.flags & race_flags::KEEP_LAPS != 0 {
                        self.keep_lap(card);
                    }
                    if self.kind == race_kind::GHOST_RACE {
                        if let Some(g) = ghost {
                            self.gate_run_over(card, g, &mut said);
                        }
                    }
                    said.push(LineNotice::Lap);
                } else if self.kind == race_kind::FAST_LAP {
                    card.flags &= !card_flags::FAST_LAP;
                }
            }
            Crossing::Beside => said.push(LineNotice::Beside),
            _ => {}
        }
        if self.kind != race_kind::PRACTICE && self.kind != race_kind::GHOST_RACE && self.laps <= card.laps {
            card.flags |= card_flags::OUT;
            self.flags |= race_flags::FINISHED;
            said.push(LineNotice::Finished);
        }
        (true, said)
    }

    /// A ghost race's lap in the gate race. Unlike the line's ([`LapRules::line`]), the card keeps its
    /// last lap's time and starts the run with its lap length already behind it, the ghost's card
    /// takes the rider's lap length as its own laps behind it and is not moved, and the ghost counts
    /// once the runs are in whether or not it has a best time.
    fn gate_run_over(&mut self, card: &mut LapCard, g: &mut LapCard, said: &mut Vec<LineNotice>) {
        card.lap_clock = F::ZERO;
        card.lap_start = F::ZERO;
        card.race_time = F::ZERO;
        card.progress = F::ZERO;
        card.laps_length = card.lap_length;
        card.laps = 0;
        card.runs = card.runs.wrapping_add(1);
        card.gate = Some(0);
        card.gates_passed = 0;
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
        g.laps_length = card.lap_length;
        g.gate = Some(0);
        g.gates_passed = 0;
        g.laps = 0;
        if card.runs >= self.ghost_runs {
            g.flags |= card_flags::TAKING_PART;
            said.push(LineNotice::GhostReady);
        }
    }
}

/// The zones a rider's step went through or past (the score manager's zone list, read with its
/// [`race_flags`] zone bit by the machine-race and the stunt-mode handlers): each live zone in list
/// order, with [`Crossing::Through`] or [`Crossing::Beside`]; the listeners are told each. Nothing
/// else is kept.
///
/// The stunt-mode handler copies the rider's place into the tick before's ([`Seen::last`] =
/// [`Seen::now`]) before it asks, so there the step never moves and no zone is ever crossed.
pub fn zone_crossings(seen: Seen, zones: &[Gate]) -> Vec<(usize, Crossing)> {
    zones
        .iter()
        .enumerate()
        .filter(|(_, z)| z.flags & flags::CLOSED != 0)
        .filter_map(|(k, z)| match z.crossed(seen.now, seen.last) {
            c @ (Crossing::Through | Crossing::Beside) => Some((k, c)),
            _ => None,
        })
        .collect()
}

// --- the track's paths, as the race asks them (S293) ------------------------------------------------

/// The path flag bits the race's path queries read and write (a path's `+0` word).
pub mod path_flags {
    /// The track owns the path. Every path the file makes has it; a route search that gives up would
    /// free a path without it, which the port never does (see [`super::TrackPaths::new`]).
    pub const OWNED: u32 = 0x01;
    /// The route search's mark on a path it has opened, cleared from every path when it succeeds and
    /// left where it was when it gives up.
    pub const OPENED: u32 = 0x04;
    /// A checkpoint path; the race handler clears it once a card reaches the path's last stretch.
    pub const CHECKPOINT: u32 = 0x40;
    /// A path the queries asked to keep off (the race asks so every time) leave out.
    pub const KEEP_OFF: u32 = 0x100;
    /// A path no link reaches from path 0. The file sets it on every path and the load clears it from
    /// the ones path 0 reaches ([`super::TrackPaths::mark_reached`]).
    pub const UNREACHED: u32 = 0x80;
    /// Paths the spot search never looks for a rider on: unreached or kept off.
    pub const NOT_SEARCHED: u32 = 0x180;
}

/// How many entries each of the track's three scratch pools holds per path.
const POOL_PER_PATH: usize = 10;

/// One path of [`TrackPaths`]: a [`TrackPath`] with its links made indices, and the two scratch words
/// the route search leaves in it.
#[derive(Clone, Debug, PartialEq)]
pub struct RacePath {
    /// The flags (`+0`), [`path_flags`].
    pub flags: u32,
    /// The length (`+8`).
    pub length: F,
    /// Its nodes, at least one.
    pub nodes: Vec<PathNode>,
    /// The paths it leads on to, by index.
    pub links: Vec<usize>,
    /// The route search's distance from this path to its goal (`+0x1c`), and the link it found shortest
    /// (`+0x28`). Both stay from one search to the next.
    pub dist: F,
    pub best: Option<usize>,
}

/// The track object as the race's path queries see it: its paths, the reach a rider's spot is looked
/// for in, the lap length, and the three scratch pools the queries take entries from.
///
/// The pools hold `10 ×` the path count each (the original allocates them so when the file is read).
/// An entry a query takes and does not give back is gone for good: the spot search keeps the
/// candidates it took when it runs out halfway, as the original does. Only the counts are kept; the
/// entries carry nothing the queries read before writing.
///
/// A [`Spot`]'s words name a path and a node: in these queries they are 1 + the path's index and 1 +
/// the node's index along it, and 0 is no path (the original's null). [`Spot::on`] makes one.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackPaths {
    /// The file's tag `0x29` word (`+0x34`); bit 2 makes the spot search prefer the spot nearest along
    /// the path over the one nearest on the ground.
    pub word_34: u32,
    /// How far ahead and behind a rider's last spot the search takes a new one (`+0x38`, `+0x3c`):
    /// 200 each when the track is made; a race sets its own.
    pub ahead: F,
    pub behind: F,
    /// The lap's length (`+0x54`). 0 here; the race sets it.
    pub lap_length: F,
    pub paths: Vec<RacePath>,
    /// The entries left in the route pool (`+0x98`), the parent pool (`+0xa0`) and the candidate pool
    /// (`+0xa8`).
    pub free_routes: usize,
    pub free_parents: usize,
    pub free_candidates: usize,
}

impl Spot {
    /// The spot `along` the way from node `node` of path `path` to the next node.
    pub fn on(path: usize, node: usize, along: F) -> Spot {
        Spot { path: path as u32 + 1, piece: node as u32 + 1, along }
    }

    /// The path and node the spot is on, if it names both.
    pub fn at(self) -> Option<(usize, usize)> {
        if self.path == 0 || self.piece == 0 {
            return None;
        }
        Some((self.path as usize - 1, self.piece as usize - 1))
    }

    fn on_path(self) -> usize {
        self.at().expect("a spot the path queries walk from names a path and a node").0
    }
}

/// `x ⋅ x` on the flat plane through the FPU accumulator: `mula` x, `madd` z.
fn flat_sq(x: F, z: F) -> F {
    F(fpu::madd(fpu::mula(x.0, x.0), z.0, z.0))
}

/// Is `point` (x and z) inside the ring the two nodes' side points make? The ring runs from `a`'s
/// second point to `b`'s second, `b`'s first, `b`'s third, `a`'s third, `a`'s first and back, and a
/// ray from the point towards −z counts its crossings.
pub fn ring_holds(point: Vf, a: &PathNode, b: &PathNode) -> bool {
    let ring = [a.points[1], b.points[1], b.points[0], b.points[2], a.points[2], a.points[0], a.points[1]];
    let (px, pz) = (F(point.x()), read(F(point.z())));
    let mut odd = false;
    for w in ring.windows(2) {
        let (u, v) = (w[0], w[1]);
        let e = F(v.x()) - F(u.x());
        let t = e * (px - F(u.x()));
        if t < F::ZERO {
            continue;
        }
        let e2 = e * e;
        if !(t < e2) {
            continue;
        }
        let z = F(u.z()) + (t / e2) * (F(v.z()) - F(u.z()));
        if z < pz {
            odd = !odd;
        }
    }
    odd
}

impl TrackPaths {
    /// The paths of a read race file, as the track object holds them once it is loaded.
    ///
    /// `None` for a file the queries cannot walk without reading memory the original never allocated:
    /// an empty path slot, a path with no nodes, a link that names no path, or a path without
    /// [`path_flags::OWNED`] (the file always sets it).
    pub fn new(file: &TrackFile) -> Option<TrackPaths> {
        let mut paths = Vec::new();
        for p in &file.paths {
            let p = p.as_ref()?;
            if p.nodes.is_empty() || p.flags & path_flags::OWNED == 0 {
                return None;
            }
            let mut links = Vec::new();
            for l in p.links.iter().flatten() {
                match *l {
                    Link::Path(i) if file.paths.get(i).is_some_and(|q| q.is_some()) => links.push(i),
                    _ => return None,
                }
            }
            paths.push(RacePath { flags: p.flags, length: p.length, nodes: p.nodes.clone(), links, dist: F::ZERO, best: None });
        }
        let pool = POOL_PER_PATH * paths.len();
        let mut t = TrackPaths {
            word_34: file.word_34,
            ahead: F(200.0),
            behind: F(200.0),
            lap_length: F::ZERO,
            paths,
            free_routes: pool,
            free_parents: pool,
            free_candidates: pool,
        };
        if !t.paths.is_empty() {
            t.mark_reached(0);
        }
        Some(t)
    }

    /// What the load does once the links are read: walks out from path `from` along the links and
    /// clears [`path_flags::UNREACHED`] from every path it reaches. The walk is depth first, its stack
    /// taken from the route pool; when the pool runs dry the answer is 0 and the walk stops where it
    /// is, its stack entries lost and its [`path_flags::OPENED`] marks left on. Otherwise the marks
    /// come off every path and the answer is 1.
    pub fn mark_reached(&mut self, from: usize) -> i32 {
        if self.free_routes == 0 {
            return 0;
        }
        self.free_routes -= 1;
        let mut stack = vec![from];
        while let Some(&p) = stack.last() {
            if self.paths[p].flags & path_flags::OPENED != 0 {
                stack.pop();
                self.free_routes += 1;
                continue;
            }
            self.paths[p].flags = (self.paths[p].flags | path_flags::OPENED) & !path_flags::UNREACHED;
            for j in 0..self.paths[p].links.len() {
                let l = self.paths[p].links[j];
                if self.paths[l].flags & path_flags::OPENED != 0 {
                    continue;
                }
                if self.free_routes == 0 {
                    return 0;
                }
                self.free_routes -= 1;
                stack.push(l);
            }
        }
        for path in &mut self.paths {
            path.flags &= !path_flags::OPENED;
        }
        1
    }

    fn span(&self, p: usize, n: usize) -> F {
        self.paths[p].nodes[n].span
    }

    fn keep_off(&self, p: usize) -> bool {
        self.paths[p].flags & path_flags::KEEP_OFF != 0
    }

    /// The point a spot stands on: its node's first point, `along` the way to the next node's, with
    /// the node's `w`. An `along` outside 0..1 is clamped, and written back into the spot. On a path's
    /// last node there is no next one: the answer is false, and `out` is the node's point only if
    /// `along` is 0. False, `out` untouched, for a spot that names no node.
    pub fn spot_point(&self, spot: &mut Spot, out: &mut Vf) -> bool {
        let Some((p, n)) = spot.at() else { return false };
        let nodes = &self.paths[p].nodes;
        let here = nodes[n].points[0];
        let Some(next) = nodes.get(n + 1) else {
            if is_zero(spot.along) {
                *out = here;
            }
            return false;
        };
        if read(spot.along) < F::ZERO {
            spot.along = F::ZERO;
        } else if !(read(spot.along) <= F::ONE) {
            spot.along = F::ONE;
        }
        let step = next.points[0].sub(here).mul(Vf::splat(spot.along.0));
        *out = here.merge(Dest::XYZ, here.add(step));
        true
    }

    /// The way the path runs at a spot: the unit step from its node's first point to the next node's,
    /// with the next point's `w`. False, `out` untouched, on a path's last node, a node of no length,
    /// or a spot that names no node.
    pub fn path_ahead(&self, spot: Spot, out: &mut Vf) -> bool {
        let Some((p, n)) = spot.at() else { return false };
        let nodes = &self.paths[p].nodes;
        let Some(next) = nodes.get(n + 1) else { return false };
        if !(F::ZERO < read(nodes[n].span)) {
            return false;
        }
        let ahead = next.points[0];
        *out = unit(ahead.merge(Dest::XYZ, ahead.sub(nodes[n].points[0])));
        true
    }

    /// Is `b` at or after `a` along the same path?
    fn reaches(&self, a: Spot, b: Spot) -> bool {
        if a.path != b.path {
            return false;
        }
        if a.piece == b.piece {
            return read(a.along) <= read(b.along);
        }
        b.piece > a.piece
    }

    /// How far along the paths `b` is from `a`. Along one path, the spans between them; when `b` is
    /// behind `a` or on another path, the route search's shortest way round ([`TrackPaths::route`]),
    /// 0 if it finds none. With `keep_off`, −1 if either spot is on a [`path_flags::KEEP_OFF`] path.
    pub fn distance(&mut self, a: Spot, b: Spot, keep_off: bool) -> F {
        let (pa, na) = a.at().expect("a spot the path queries measure from names a path and a node");
        let (pb, nb) = b.at().expect("a spot the path queries measure to names a path and a node");
        if keep_off && (self.keep_off(pa) || self.keep_off(pb)) {
            return F(-1.0);
        }
        let mut d = F::ZERO;
        if pa != pb {
            self.route(a, b, None, Some(&mut d), keep_off);
            return d;
        }
        if na == nb {
            if read(a.along) <= read(b.along) {
                return self.span(pa, na) * (b.along - a.along);
            }
            self.route(a, b, None, Some(&mut d), keep_off);
            return d;
        }
        let mut n = na;
        loop {
            d += self.span(pa, n);
            n += 1;
            if n == nb {
                let part = fpu::msub(fpu::mula(b.along.0, self.span(pa, nb).0), a.along.0, self.span(pa, na).0);
                return d + F(part);
            }
            if n == self.paths[pa].nodes.len() {
                // Off the path's end before reaching `b`: the search's answer, or the spans so far.
                self.route(a, b, None, Some(&mut d), keep_off);
                return d;
            }
        }
    }

    /// The route search: the shortest way along the paths from `a` to `b`. Its length goes to `d`; the
    /// paths to ride, in order, are pushed onto `route` (which must be empty) — the original's way of
    /// building it leaves out the path just before `b`'s, and starts at `b`'s own when that one is
    /// the first's best link. 1 when it found a way (or `b` is ahead of `a` on one path), 0 when it
    /// found none or ran out of pool entries (the entries go back, the opened marks stay), −1 when
    /// `keep_off` and either spot is on a [`path_flags::KEEP_OFF`] path.
    ///
    /// The search is depth first from `a`'s path over the links: each path's distance is its shortest
    /// link's plus its own length, `b`'s path's is the way from its start to `b`, and the first path's
    /// adds the way from `a` to its end. A path reached again before its own links are done takes
    /// whatever distance it held, from this search or an earlier one.
    pub fn route(&mut self, a: Spot, b: Spot, mut route: Option<&mut Vec<usize>>, d: Option<&mut F>, keep_off: bool) -> i32 {
        let pa = a.on_path();
        let pb = b.on_path();
        if keep_off && (self.keep_off(pa) || self.keep_off(pb)) {
            return -1;
        }
        if pa == pb && self.reaches(a, b) {
            if let Some(r) = route {
                if self.free_routes == 0 {
                    return 0;
                }
                self.free_routes -= 1;
                r.push(pb);
            }
            if let Some(d) = d {
                *d = self.distance(a, b, keep_off);
            }
            return 1;
        }
        let last = self.paths[pa].nodes.len() - 1;
        let tail = self.distance(a, Spot::on(pa, last, F::ONE), keep_off);
        self.paths[pb].dist = self.distance(Spot::on(pb, 0, F::ZERO), b, keep_off);
        if self.free_routes == 0 {
            return 0;
        }
        self.free_routes -= 1;
        let mut stack = vec![pa];
        let mut reached = false;
        let mut d = d;
        while let Some(&p) = stack.last() {
            if self.paths[p].flags & path_flags::OPENED == 0 {
                self.paths[p].flags |= path_flags::OPENED;
                for i in 0..self.paths[p].links.len() {
                    let l = self.paths[p].links[i];
                    if l == pb {
                        reached = true;
                    }
                    if self.paths[l].flags & path_flags::OPENED == 0 && l != pb && !(keep_off && self.keep_off(l)) {
                        if self.free_routes == 0 {
                            self.free_routes += stack.len();
                            return 0;
                        }
                        self.free_routes -= 1;
                        stack.push(l);
                    }
                }
                continue;
            }
            self.paths[p].best = None;
            for i in 0..self.paths[p].links.len() {
                let l = self.paths[p].links[i];
                let shorter = self.paths[p].best.is_none() || read(self.paths[l].dist) < read(self.paths[p].dist);
                if shorter && !(keep_off && self.keep_off(l)) {
                    self.paths[p].dist = self.paths[l].dist;
                    self.paths[p].best = Some(l);
                }
            }
            if stack.len() == 1 {
                self.paths[p].dist += tail;
                if let Some(d) = d.as_deref_mut() {
                    *d = self.paths[p].dist;
                }
                if let Some(r) = route.as_deref_mut()
                    && reached
                    && !self.build_route(p, pb, r)
                {
                    self.free_routes += stack.len() + r.len();
                    r.clear();
                    return 0;
                }
            } else {
                let length = self.paths[p].length;
                self.paths[p].dist += length;
            }
            self.free_routes += 1;
            stack.pop();
        }
        for path in self.paths.iter_mut() {
            path.flags &= !path_flags::OPENED;
        }
        1
    }

    /// The route from `first` to `goal` over the best links, as the original strings it: each path
    /// whose best link is not `goal`, then `goal`. False when the pool runs out, or the chain meets a
    /// path with no best link (where the original reads on from a null path; see the journal).
    fn build_route(&mut self, first: usize, goal: usize, r: &mut Vec<usize>) -> bool {
        let mut cur = first;
        loop {
            let Some(next) = self.paths[cur].best else { return false };
            if next == goal {
                break;
            }
            if self.free_routes == 0 {
                return false;
            }
            self.free_routes -= 1;
            r.push(cur);
            cur = next;
        }
        if self.free_routes == 0 {
            return false;
        }
        self.free_routes -= 1;
        r.push(goal);
        true
    }

    /// Give a route's entries back to the pool.
    pub fn free_route(&mut self, route: &mut Vec<usize>) {
        self.free_routes += route.len();
        route.clear();
    }

    /// The spot `dist` further on from `spot` along `route` (from [`TrackPaths::route`]), into `out`.
    /// At the route's end it stops at the last path's last node. False, `out` untouched, when the
    /// route is empty or does not start on `spot`'s path; `spot` itself when `dist` is not above 0.
    pub fn route_walk(&self, dist: F, spot: Spot, route: &[usize], out: &mut Spot) -> bool {
        let Some(&first) = route.first() else { return false };
        if spot.path != first as u32 + 1 {
            return false;
        }
        if !(F::ZERO < dist) {
            *out = spot;
            return true;
        }
        let (mut p, mut n) = (first, spot.at().map_or(0, |(_, n)| n));
        let mut entry = 0;
        let mut along = spot.along;
        let mut gone = F::ZERO;
        loop {
            if read(along) == F::ONE {
                if n + 1 < self.paths[p].nodes.len() {
                    n += 1;
                    gone += self.span(p, n);
                } else if entry + 1 < route.len() {
                    entry += 1;
                    (p, n) = (route[entry], 0);
                    gone += self.span(p, n);
                } else {
                    gone = dist;
                }
            } else {
                gone += self.span(p, n) * (F::ONE - along);
                along = F::ONE;
            }
            if !(gone <= dist) {
                let span = self.span(p, n);
                if !is_zero(span) {
                    along = F::ONE - (gone - dist) / span;
                }
            }
            if !(gone < dist) {
                break;
            }
        }
        *out = Spot::on(p, n, along);
        true
    }

    /// Where along the paths `point` is: the track object's spot search.
    ///
    /// Every stretch of every searched path whose ring holds the point (within its span plus `ahead +
    /// behind`) gives a candidate, the point's place along the stretch clamped to 0..1. Then, newest
    /// first, a candidate within `ahead` along the paths of `start`, or `behind` before it, is taken —
    /// the nearest on the ground, or with [`TrackPaths::word_34`] bit 2 the one furthest along within
    /// reach; with `ahead` or `behind` negative the first candidate is taken outright. Failing that a
    /// candidate is measured from the start of each `via` path, and failing that from the end of any
    /// path that leads to both its path and `start`'s, within the track's own reach (answer 2).
    ///
    /// `out` is `start` unless a candidate is taken; `dist_out`, when a candidate is, the distance it
    /// was taken at. 0 when none was (or the candidate pool ran out, keeping the ones taken so far).
    #[allow(clippy::too_many_arguments)]
    pub fn find_spot(&mut self, ahead: F, behind: F, point: Vf, start: Spot, via: &[usize], out: &mut Spot, dist_out: Option<&mut F>) -> i32 {
        *out = start;
        let mut best_d = F(fpu::neg(behind.0));
        let mut best_sq = F(1e6);
        let (px, pz) = (F(point.x()), F(point.z()));
        let reach = ahead + behind;
        let mut cands: Vec<Spot> = Vec::new();
        for i in 0..self.paths.len() {
            if self.paths[i].flags & path_flags::NOT_SEARCHED != 0 {
                continue;
            }
            for n in 0..self.paths[i].nodes.len() - 1 {
                let (here, next) = (&self.paths[i].nodes[n], &self.paths[i].nodes[n + 1]);
                let (hx, hz) = (F(here.points[0].x()), F(here.points[0].z()));
                let r = here.span + reach;
                if !(flat_sq(hx - px, hz - pz) < r * r) || !ring_holds(point, here, next) {
                    continue;
                }
                let (dx, dz) = (F(next.points[0].x()) - hx, F(next.points[0].z()) - hz);
                let mut t = F::ZERO;
                if !(dx == F::ZERO && dz == F::ZERO) {
                    let up = fpu::madd(fpu::mula(dx.0, (px - hx).0), dz.0, (pz - hz).0);
                    t = F(up) / flat_sq(dx, dz);
                }
                if !(t <= F::ONE) {
                    t = F::ONE;
                } else if t < F::ZERO {
                    t = F::ZERO;
                }
                if self.free_candidates == 0 {
                    return 0;
                }
                self.free_candidates -= 1;
                cands.push(Spot::on(i, n, t));
            }
        }
        let (ahead_r, behind_r) = (read(ahead), read(behind));
        let mut found = 0;
        for c in cands.iter_mut().rev() {
            if ahead_r < F::ZERO || behind_r < F::ZERO {
                if found == 0 {
                    *out = *c;
                }
                found = 1;
            }
            let d1 = self.distance(start, *c, true);
            let d2 = self.distance(*c, start, true);
            if (!(d1 < F::ZERO) && d1 <= ahead_r) || (!(d2 < F::ZERO) && d2 <= behind_r) {
                found = 1;
                self.take(c, d1, d2, point, out, &mut best_d, &mut best_sq);
            }
            for &lp in via {
                if found != 0 {
                    break;
                }
                let from = Spot::on(lp, 0, F::ZERO);
                let da = self.distance(from, *c, true);
                let db = self.distance(from, start, true);
                let on_lp = |s: Spot| s.at().is_some_and(|(p, _)| p == lp);
                if on_lp(*c) && (!on_lp(start) || (!(da < F::ZERO) && !(db < F::ZERO) && da < db)) {
                    found = 1;
                    self.take(c, da, db, point, out, &mut best_d, &mut best_sq);
                }
                if (!(da < F::ZERO) && da <= ahead_r) || (!(db < F::ZERO) && db <= behind_r) {
                    found = 1;
                    self.take(c, da, db, point, out, &mut best_d, &mut best_sq);
                }
            }
        }
        if found == 0 && !cands.is_empty() {
            let start_path = start.at().map(|(p, _)| p);
            let mut parents = Vec::new();
            for i in 0..self.paths.len() {
                for j in 0..self.paths[i].links.len() {
                    let l = self.paths[i].links[j];
                    if Some(l) != start_path || self.keep_off(l) || self.free_parents == 0 {
                        continue;
                    }
                    self.free_parents -= 1;
                    parents.push(i);
                }
            }
            let (reach_ahead, reach_behind) = (read(self.ahead), F(fpu::neg(read(self.behind).0)));
            for c in cands.iter().rev() {
                if c.path == start.path {
                    continue;
                }
                for &lp in parents.iter().rev() {
                    if found != 0 {
                        break;
                    }
                    for j in 0..self.paths[lp].links.len() {
                        let l = self.paths[lp].links[j];
                        if l as u32 + 1 != c.path || self.keep_off(l) {
                            continue;
                        }
                        let end = Spot::on(lp, self.paths[lp].nodes.len() - 1, F::ZERO);
                        let f = self.distance(end, *c, true) - self.distance(end, start, true);
                        if !(f <= reach_ahead) || f < reach_behind {
                            continue;
                        }
                        best_d = f;
                        found = 2;
                        *out = *c;
                    }
                }
            }
            self.free_parents += parents.len();
        }
        self.free_candidates += cands.len();
        if found != 0
            && let Some(d) = dist_out
        {
            *d = best_d;
        }
        found
    }

    /// The spot search taking candidate `c`, found `da` ahead of (and `db` behind) its measure.
    #[allow(clippy::too_many_arguments)]
    fn take(&self, c: &mut Spot, da: F, db: F, point: Vf, out: &mut Spot, best_d: &mut F, best_sq: &mut F) {
        let back = F(fpu::neg(db.0));
        if self.word_34 & 2 == 0 || (da <= read(*best_d) && back <= read(*best_d)) {
            let mut at = Vf::ZERO;
            self.spot_point(c, &mut at);
            let sq = flat_sq(F(point.x()) - F(at.x()), F(point.z()) - F(at.z()));
            if sq < *best_sq {
                *best_sq = sq;
                *best_d = da;
                *out = *c;
            }
        } else {
            *out = *c;
            *best_d = if da <= read(*best_d) { back } else { da };
        }
    }
}

// --- the race handler, per card (S293) -----------------------------------------------------------

/// More of a score card's flag bits, as the race handler reads them (see also [`card_flags`]).
pub mod race_card_flags {
    /// A rider the race waits for (`0x4`, and `0x8000` beside it; what sets either was not traced).
    /// Such a card clears a checkpoint path once it reaches its last stretch, and finishing it tells
    /// the manager [`super::race_flags::PLAYER_OUT`].
    pub const WAITED_FOR: u64 = 0x04;
    pub const WAITED_FOR_TOO: u64 = 0x8000;
    /// A fast-lap attack's own clock runs (the card's `+0x168`).
    pub const FAST_LAP_CLOCK: u64 = 0x200;
    /// The card's time ran out ([`super::LapCard::time_left`]); with [`super::card_flags::OUT`] it
    /// marks a card the handler no longer races.
    pub const OUT_OF_TIME: u64 = 0x4000;
    /// The card's time is held: it does not count down.
    pub const CLOCK_HELD: u64 = 0x4_0000;
    /// A computer rider's aim: the handler aims it through a helper the port leaves out.
    pub const COMPUTER_AIM: u64 = 0x80_0000;
    /// The off-track clock has a limit ([`super::RaceRules::off_limit`]).
    pub const OFF_TRACK_LIMIT: u64 = 0x100_0000;
    /// The race does not wait for this card even if it is [`WAITED_FOR`].
    pub const NOT_WAITED_FOR: u64 = 0x40_0000;
}

/// A score card as the race handler keeps it: the lap line's words and the rest of the handler's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaceCard {
    pub lap: LapCard,
    /// The spot search's distance to the spot it found (`+0x18`), kept from the last search that
    /// found one.
    pub on_path: F,
    /// How far the rider has gone, step by step (`+0x20`).
    pub odometer: F,
    /// The spot the rider aims at (`+0xa0`): the kept spot, for a player.
    pub aim: Spot,
    /// The rider's look ahead (`+0x120`), told to the card's listeners with its position.
    pub look: Vf,
    /// The path's way ahead at the rider's spot (`+0x140`).
    pub ahead: Vf,
    /// Off the track, the unit step from the rider back to the path (`+0x180`).
    pub to_track: Vf,
}

/// The score manager's words the race handler reads, beside the lap line's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaceRules {
    pub lap: LapRules,
    /// A rider further than this below the path is off the track (`+0xb8`; 0 or less: no limit).
    pub below_limit: F,
    /// The race's time limit (`+0xa0`; 0 or less: none). Only whether there is one is read: the cards
    /// carry their own time left.
    pub time_limit: F,
    /// How long a rider may be off the track (`+0xd4`), for cards with
    /// [`race_card_flags::OFF_TRACK_LIMIT`].
    pub off_limit: F,
    /// Off the track and far from the line, the rider is pointed this many of the track's search
    /// reaches ahead along the path (`+0xd8`).
    pub aim_reach: F,
}

/// What the race handler tells a card's listeners, in the order it tells them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RaceNotice {
    /// The lap line's.
    Line(LineNotice),
    /// Where the rider is and where it looks (the card's owner and its chain).
    Aim { point: Vf, ahead: Vf },
    /// The rider is off the track ([`RaceCard::to_track`] is the way back).
    OffTrack,
    /// The rider has been off the track for [`RaceRules::off_limit`].
    OffTooLong,
    /// The card's time ran out.
    OutOfTime,
}

impl RaceRules {
    /// What the race handler does before its cards, outside a practice or ghost race: the manager's
    /// [`race_flags::ALL_OUT`] is set unless some card (by its flags) is a rider the race waits for
    /// that is still racing.
    pub fn look_for_waited_for(&mut self, cards: impl IntoIterator<Item = u64>) {
        use race_card_flags as rf;
        if self.lap.kind == race_kind::PRACTICE || self.lap.kind == race_kind::GHOST_RACE {
            return;
        }
        self.lap.flags |= race_flags::ALL_OUT;
        for f in cards {
            if f & card_flags::TAKING_PART != 0
                && f & (card_flags::OUT | rf::OUT_OF_TIME) == 0
                && f & (rf::WAITED_FOR | rf::WAITED_FOR_TOO) != 0
                && f & rf::NOT_WAITED_FOR == 0
            {
                self.lap.flags &= !race_flags::ALL_OUT;
                break;
            }
        }
    }

    /// One tick of the race handler for one card, `dt` long, the card's rider already moved to
    /// `card.lap.seen.now`: the odometer, the path's way ahead, the spot search (is the rider on the
    /// track, and where), the distances to the line, the lap line once the card is near it and the card
    /// set back on the line after a counted lap, the race progress, the off-track clock, the clocks,
    /// and then the rider's aim, the way back to the track with the off-track limit, and the card's
    /// time limit.
    ///
    /// A card not taking part, or done ([`card_flags::OUT`], [`race_card_flags::OUT_OF_TIME`]), or in
    /// a fast-lap attack without [`card_flags::FAST_LAP`], only tells its aim. `leader_time` is the
    /// race clock of the manager's first card, which a card still out when every waited-for rider is
    /// done ([`race_flags::ALL_OUT`]) is timed from.
    ///
    /// The card's spots and the line's must name nodes. Left out (see the S293 journal): the line
    /// lists (manager flags `0x40`, `0x200`), the helpers behind manager flags `0x800` / `0x1000` and
    /// card flag `0x8_0000`, and [`race_card_flags::COMPUTER_AIM`]'s aim.
    #[allow(clippy::too_many_arguments)]
    pub fn card(
        &mut self,
        dt: F,
        card: &mut RaceCard,
        ghost: Option<&mut LapCard>,
        track: &mut TrackPaths,
        course: &Course,
        leader_time: F,
    ) -> Vec<RaceNotice> {
        use race_card_flags as rf;
        let mut said = Vec::new();
        let kind = self.lap.kind;
        let fl = card.lap.flags;
        let done = card_flags::OUT | rf::OUT_OF_TIME;
        if fl & card_flags::TAKING_PART == 0 || (kind == race_kind::FAST_LAP && fl & card_flags::FAST_LAP == 0) || fl & done != 0 {
            if kind == race_kind::FAST_LAP {
                card.lap.on_track = 1;
            }
        } else {
            self.race_step(dt, card, ghost, track, course, leader_time, &mut said);
        }
        let fl = card.lap.flags;
        if fl & card_flags::TAKING_PART == 0 || fl & done != 0 {
            if fl & done != 0 {
                said.push(RaceNotice::Aim { point: card.lap.seen.now, ahead: card.look });
            }
            return said;
        }
        if fl & rf::COMPUTER_AIM == 0 {
            said.push(RaceNotice::Aim { point: card.lap.seen.now, ahead: card.look });
            card.aim = card.lap.kept_spot;
        }
        if card.lap.on_track == 0 {
            self.back_to_track(card, track, course, &mut said);
        }
        if read(self.time_limit) > F::ZERO && read(card.lap.time_left) > F::ZERO {
            let fl = card.lap.flags;
            if fl & card_flags::TAKING_PART != 0 && fl & done == 0 {
                if fl & rf::CLOCK_HELD == 0 {
                    card.lap.time_left = card.lap.time_left - dt;
                }
                if read(card.lap.time_left) <= F::ZERO {
                    card.lap.flags |= rf::OUT_OF_TIME;
                    card.lap.time_left = F::ZERO;
                    said.push(RaceNotice::OutOfTime);
                }
            }
        }
        said
    }

    /// The handler's work on a card it races, up to its clocks.
    #[allow(clippy::too_many_arguments)]
    fn race_step(
        &mut self,
        dt: F,
        card: &mut RaceCard,
        ghost: Option<&mut LapCard>,
        track: &mut TrackPaths,
        course: &Course,
        leader_time: F,
        said: &mut Vec<RaceNotice>,
    ) {
        use race_card_flags as rf;
        let kind = self.lap.kind;
        let step = card.lap.seen.now.sub(card.lap.seen.last);
        let len = fpu::madd(fpu::madda(fpu::mula(step.x(), step.x()), step.y(), step.y()), step.z(), step.z());
        card.odometer = card.odometer + F(fpu::sqrt(len));
        track.path_ahead(card.lap.spot, &mut card.ahead);
        let (reach_ahead, reach_behind) = (track.ahead, track.behind);
        let start = card.lap.kept_spot;
        let now = card.lap.seen.now;
        card.lap.on_track = track.find_spot(reach_ahead, reach_behind, now, start, &[], &mut card.lap.spot, Some(&mut card.on_path));
        if read(self.below_limit) > F::ZERO {
            // A spot naming no node reads the word at 0x14 of low memory; the port takes it as 0.
            let path_y = card.lap.spot.at().map_or(F::ZERO, |(p, n)| F(track.paths[p].nodes[n].points[0].y()));
            if !(path_y - F(now.y()) <= read(self.below_limit)) {
                card.lap.on_track = 0;
            }
        }
        if card.lap.flags & rf::WAITED_FOR != 0
            && let Some((p, n)) = card.lap.kept_spot.at()
            && track.paths[p].flags & path_flags::CHECKPOINT != 0
            && n + 2 == track.paths[p].nodes.len()
        {
            track.paths[p].flags &= !path_flags::CHECKPOINT;
        }
        if card.lap.on_track == 0 {
            card.lap.off_track = card.lap.off_track + dt;
        } else {
            self.near_line(card, ghost, track, course, said);
            if card.lap.on_track == 0 {
                card.lap.off_track = card.lap.off_track + dt;
            } else {
                card.lap.off_track = F::ZERO;
            }
        }
        let last = card.lap.seen.last;
        card.lap.seen.last = last.merge(Dest::XYZ, now);
        let fl = card.lap.flags;
        if kind == race_kind::GHOST_RACE || self.lap.flags & race_flags::ALL_OUT == 0 || fl & card_flags::OUT != 0 {
            if fl & card_flags::OUT == 0 {
                if kind != race_kind::GHOST_RACE || fl & card_flags::BEHIND_LINE == 0 {
                    card.lap.race_time = card.lap.race_time + dt;
                }
                if fl & card_flags::BEHIND_LINE == 0 {
                    card.lap.lap_clock = card.lap.lap_clock + dt;
                }
            }
        } else {
            // Everyone the race waits for is done: the card is timed off the first card's clock.
            let c = &mut card.lap;
            c.race_time = leader_time + c.gap;
            let left = F(fpu::from_i32(self.lap.laps.wrapping_sub(c.laps)));
            c.lap_time = (c.gap + c.lap_start) / left;
            if read(c.lap_time) < read(c.best_lap) || read(c.best_lap) == F::ZERO {
                c.best_lap = c.lap_time;
            }
            let laps = F(fpu::from_i32(self.lap.laps));
            if read(c.race_time) < laps * c.best_lap {
                c.best_lap = c.race_time / laps;
            }
            c.laps = self.lap.laps;
            c.flags |= card_flags::OUT;
        }
        if kind == race_kind::FAST_LAP && card.lap.flags & rf::FAST_LAP_CLOCK != 0 {
            card.lap.attack_clock = card.lap.attack_clock + dt;
        }
    }

    /// On the track: the distances to the line, the lap line when the card is close to it (and the
    /// card set back on the line after a counted lap), and the race progress.
    fn near_line(&mut self, card: &mut RaceCard, ghost: Option<&mut LapCard>, track: &mut TrackPaths, course: &Course, said: &mut Vec<RaceNotice>) {
        let (reach_ahead, reach_behind) = (read(track.ahead), read(track.behind));
        let c = &mut card.lap;
        c.to_line_kept = track.distance(c.kept_spot, course.line_spot, true);
        c.to_line = track.distance(c.spot, course.line_spot, true);
        let kept = c.to_line_kept;
        if kept != F::ZERO && kept < reach_ahead && !(c.to_line - kept <= reach_behind) {
            // Ahead of the kept spot by more than the search looks back, close to the line: the
            // rider is taken to be across it, and off the track if it stands on its far side.
            c.to_line = kept;
            let off = c.seen.now.sub(course.line.centre);
            let n = course.line.normal;
            let side = fpu::madd(fpu::madda(fpu::mula(off.x(), n.x()), off.y(), n.y()), off.z(), n.z());
            if !(F(side) <= F::ZERO) {
                c.on_track = 0;
            }
        }
        let near = reach_ahead < F::ZERO || (!(kept < F::ZERO) && kept < reach_ahead);
        if near && !(read(F(c.seen.last.x())) < F::ZERO) {
            let (counted, heard) = self.lap.line(c, ghost, course);
            said.extend(heard.into_iter().map(RaceNotice::Line));
            if counted {
                c.kept_spot = Spot { along: F(fpu::add(course.line_spot.along.0, LINE_NUDGE.0)), ..course.line_spot };
                c.spot = c.kept_spot;
                c.to_line_kept = course.lap_length;
                c.to_line = course.lap_length;
                if self.lap.kind != race_kind::GHOST_RACE {
                    c.laps_length = c.laps_length + c.lap_length;
                }
                c.lap_length = course.lap_length;
                c.on_track = 1;
            }
            let fl = c.flags;
            if self.lap.kind != race_kind::GHOST_RACE
                && fl & card_flags::OUT != 0
                && fl & (race_card_flags::WAITED_FOR | race_card_flags::WAITED_FOR_TOO) != 0
            {
                self.lap.flags |= race_flags::PLAYER_OUT;
            }
        }
        let on_path = read(card.on_path);
        let c = &mut card.lap;
        if ((on_path > F::ZERO && on_path < reach_ahead) || c.on_track == 2) && (on_path < c.to_line_kept || c.to_line_kept == F::ZERO) {
            c.progress = c.laps_length + c.lap_length - c.to_line;
            c.kept_spot = c.spot;
        }
    }

    /// Off the track: the rider is pointed back at the path — at the kept spot, or, far from the
    /// line, [`RaceRules::aim_reach`] search reaches ahead of it along the way to the line — and the
    /// off-track limit.
    fn back_to_track(&mut self, card: &mut RaceCard, track: &mut TrackPaths, course: &Course, said: &mut Vec<RaceNotice>) {
        let reach = read(track.ahead);
        let kept = card.lap.kept_spot;
        let mut target = kept;
        if !(reach <= F::ZERO) && !(read(card.lap.to_line_kept) <= reach) {
            // The original's route list starts as a stale stack word; the port's starts empty.
            let mut route = Vec::new();
            track.route(kept, course.line_spot, Some(&mut route), None, true);
            if !track.route_walk(track.ahead * self.aim_reach, kept, &route, &mut target) {
                target = kept;
            }
            track.free_route(&mut route);
        }
        let mut point = card.to_track;
        track.spot_point(&mut target, &mut point);
        let back = point.merge(Dest::XYZ, point.sub(card.lap.seen.now));
        card.to_track = unit(back);
        said.push(RaceNotice::OffTrack);
        if card.lap.flags & race_card_flags::OFF_TRACK_LIMIT != 0 && read(self.off_limit) <= read(card.lap.off_track) {
            card.lap.off_track = F::ZERO;
            said.push(RaceNotice::OffTooLong);
        }
    }
}

// --- the race's finish (S294) -----------------------------------------------------------------------

/// What the race's finish and its ranking tell the cards' listeners. `card` is the card's index in the
/// list as it stands when told (place order once ranked). The card's owner and its chain hear each,
/// then the manager's listener, as with [`LineNotice`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishNotice {
    /// The race is over (told once, to every card in list order, then to the manager's listener).
    Over { card: usize },
    /// The card's time limit ran out.
    TimeUp { card: usize },
    /// The card lost a place to the card now ahead of it (`None` at the head of the list).
    PlaceLost { card: usize, ahead: Option<usize> },
    /// The card gained a place on the card now behind it (`None` at the tail).
    PlaceGained { card: usize, behind: Option<usize> },
    /// The card took the lead. The manager's listeners hear it only when their own gate lets them
    /// (their `+0x10` not above 0, or under their `+0x14`, which they clear): that state is theirs.
    Lead { card: usize },
}

/// `a < b` as `c.olt.s` reads the two words.
fn lt(a: F, b: F) -> bool {
    read(a).0 < read(b).0
}

/// `a <= b` as `c.ole.s` reads the two words.
fn le(a: F, b: F) -> bool {
    read(a).0 <= read(b).0
}

/// Whether a card is still in the race: taking part, neither done nor out of time.
fn racing(flags: u64) -> bool {
    flags & card_flags::TAKING_PART != 0 && flags & (card_flags::OUT | card_flags::TIMED_OUT) == 0
}

impl LapRules {
    /// The race handler's first look each tick: in a race (not a practice or a ghost race) the race is
    /// [`race_flags::OVER`] unless some player's card is still racing — taking part, not done or out of
    /// time, [`card_flags::PLAYER`] or [`card_flags::PLAYER_TOO`], and not
    /// [`card_flags::STANDS_ASIDE`]. `cards` in list order.
    pub fn check_over(&mut self, cards: &[LapCard]) {
        if self.kind == race_kind::PRACTICE || self.kind == race_kind::GHOST_RACE {
            return;
        }
        self.flags |= race_flags::OVER;
        let keeps_open = |f: u64| {
            racing(f)
                && f & (card_flags::PLAYER | card_flags::PLAYER_TOO) != 0
                && f & card_flags::STANDS_ASIDE == 0
        };
        if cards.iter().any(|c| keeps_open(c.flags)) {
            self.flags &= !race_flags::OVER;
        }
    }

    /// A racing card's clocks, after its path and line tests each tick: where it was seen moves on to
    /// where it is now, and then — once the race is over, for a card that has not finished — its finish
    /// is made up from its gap: its race time the leader's (`leader_time`, the list head's race time)
    /// plus its gap, its lap time the gap and its lap's start spread over the laps it had left, a best
    /// lap if that is quicker (or none yet) and no quicker than the race time over the race's laps
    /// allows, and the card done on the race's laps. A card still racing runs its race clock (not in a
    /// ghost race while behind its line) and its lap clock (not while behind its line); a fast-lap
    /// attack runs the attack clock.
    pub fn clocks(&self, dt: F, card: &mut LapCard, leader_time: F) {
        card.seen.last = card.seen.last.merge(Dest::XYZ, card.seen.now);
        let out = card.flags & card_flags::OUT != 0;
        if self.kind != race_kind::GHOST_RACE && self.flags & race_flags::OVER != 0 && !out {
            card.race_time = F(fpu::add(leader_time.0, card.gap.0));
            let left = F(fpu::from_i32(self.laps.wrapping_sub(card.laps)));
            card.lap_time = F(fpu::div(fpu::add(card.gap.0, card.lap_start.0), left.0));
            if lt(card.lap_time, card.best_lap) || is_zero(card.best_lap) {
                card.best_lap = card.lap_time;
            }
            let laps = F(fpu::from_i32(self.laps));
            if lt(card.race_time, F(fpu::mul(laps.0, card.best_lap.0))) {
                card.best_lap = F(fpu::div(card.race_time.0, laps.0));
            }
            card.laps = self.laps;
            card.flags |= card_flags::OUT;
        } else if !out {
            let behind = card.flags & card_flags::BEHIND_LINE != 0;
            if self.kind != race_kind::GHOST_RACE || !behind {
                card.race_time = F(fpu::add(card.race_time.0, dt.0));
            }
            if !behind {
                card.lap_clock = F(fpu::add(card.lap_clock.0, dt.0));
            }
        }
        if self.kind == race_kind::FAST_LAP && card.flags & card_flags::ATTACK_CLOCK != 0 {
            card.attack_clock = F(fpu::add(card.attack_clock.0, dt.0));
        }
    }

    /// A card's time limit, once a tick: while the race has time limits and the card has time left, a
    /// racing card's runs down (unless [`card_flags::CLOCK_HELD`]); at 0 the card is
    /// [`card_flags::TIMED_OUT`] (true: tell it [`FinishNotice::TimeUp`]).
    pub fn time_limit(&self, dt: F, card: &mut LapCard) -> bool {
        if le(self.time_limit, F::ZERO) || le(card.time_left, F::ZERO) || !racing(card.flags) {
            return false;
        }
        if card.flags & card_flags::CLOCK_HELD == 0 {
            card.time_left = F(fpu::sub(card.time_left.0, dt.0));
        }
        if !le(card.time_left, F::ZERO) {
            return false;
        }
        card.flags |= card_flags::TIMED_OUT;
        card.time_left = F::ZERO;
        true
    }

    /// The race handler's close each tick, after every card's own work. Once a player's card is
    /// [`race_flags::PLAYER_DONE`] the finish countdown runs (not in a ghost race, not once over, and
    /// only with a wait set): when it has run its wait the race is [`race_flags::OVER`] and every card
    /// done. The first tick the race is over its end is told: each card in list order whose next card's
    /// race time is below its own pushes the next one's to its own plus one `rand()` scaled to `[0, 1)`
    /// (so the times run in list order), and every card hears [`FinishNotice::Over`]. Then a ghost race
    /// not yet finished measures each card's gap to its ghost (`ghosts`, the card's owner's ghost card
    /// where it has one; [`LapRules::ghost_gap`]); any other race ranks the cards if it ranks them
    /// ([`race_flags::RANKED`]; [`LapRules::rank`]) and, unless a card has finished, measures the gaps
    /// ([`gaps`]). `cards` in list order, and not empty (with none the handler does nothing at all).
    pub fn close(
        &mut self,
        dt: F,
        cards: &mut Vec<LapCard>,
        ghosts: &[Option<&LapCard>],
        rng: &mut Rng,
    ) -> Vec<FinishNotice> {
        let mut said = Vec::new();
        if self.kind != race_kind::GHOST_RACE
            && self.flags & race_flags::PLAYER_DONE != 0
            && self.flags & race_flags::OVER == 0
            && !le(self.finish_wait, F::ZERO)
        {
            if lt(self.finish_clock, self.finish_wait) {
                self.finish_clock = F(fpu::add(self.finish_clock.0, dt.0));
            } else {
                self.flags |= race_flags::OVER;
                for c in cards.iter_mut() {
                    c.flags |= card_flags::OUT;
                }
            }
        }
        if self.flags & race_flags::OVER != 0 && self.flags & race_flags::OVER_TOLD == 0 {
            self.flags |= race_flags::OVER_TOLD;
            for k in 0..cards.len() {
                if k + 1 < cards.len() && !le(cards[k].race_time, cards[k + 1].race_time) {
                    let draw = rng.unit();
                    cards[k + 1].race_time = F(fpu::add(cards[k].race_time.0, draw.0));
                }
                said.push(FinishNotice::Over { card: k });
            }
        }
        if self.kind == race_kind::GHOST_RACE {
            if self.flags & race_flags::FINISHED == 0 {
                for (c, g) in cards.iter_mut().zip(ghosts) {
                    if let Some(g) = g {
                        Self::ghost_gap(c, g);
                    }
                }
            }
        } else {
            if self.flags & race_flags::RANKED != 0 {
                said.extend(Self::rank(cards));
            }
            if self.flags & race_flags::FINISHED == 0 {
                gaps(cards);
            }
        }
        said
    }

    /// The ranking: the cards sorted by how far into the race they are, furthest first, and each given
    /// its place. The sort is the original's own, a pass per place that pulls any card further on than
    /// the one in the place up into it — stepping over the card after the one it pulled, and leaving a
    /// done card where it stands (with what is behind it). Then each card's place is counted from 1: a
    /// card further back than before hears [`FinishNotice::PlaceLost`] (the card ahead of it), one
    /// further on [`FinishNotice::PlaceGained`] (the card behind it), and a new leader
    /// [`FinishNotice::Lead`]. A card with no place yet (0) is told nothing but the lead.
    pub fn rank(cards: &mut Vec<LapCard>) -> Vec<FinishNotice> {
        let n = cards.len();
        let mut p = 0;
        while p + 1 < n {
            if cards[p].flags & card_flags::OUT == 0 {
                let mut q = p + 1;
                while q < n {
                    if lt(cards[p].progress, cards[q].progress) {
                        let c = cards.remove(q);
                        cards.insert(p, c);
                        q += 2;
                    } else {
                        q += 1;
                    }
                }
            }
            p += 1;
        }
        let mut said = Vec::new();
        for k in 0..n {
            let place = k as i32 + 1;
            let was = cards[k].place;
            cards[k].place = place;
            if was != 0 && place > was {
                said.push(FinishNotice::PlaceLost { card: k, ahead: k.checked_sub(1) });
            } else if place < was {
                said.push(FinishNotice::PlaceGained { card: k, behind: (k + 1 < n).then_some(k + 1) });
            }
            if place == 1 && was != 1 {
                said.push(FinishNotice::Lead { card: k });
            }
        }
        said
    }

    /// A ghost race's gap: a player's card, while its lap is no slower than its ghost's best and the
    /// ghost is taking part, is as far behind the ghost as the race time it took to get where it is,
    /// less the ghost's time to get there at the ghost's pace (0 while the ghost has gone nowhere).
    pub fn ghost_gap(card: &mut LapCard, ghost: &LapCard) {
        if card.flags & card_flags::PLAYER == 0
            || ghost.flags & card_flags::TAKING_PART == 0
            || !le(card.lap_clock, ghost.best_lap)
        {
            return;
        }
        card.gap = if lt(F::ZERO, ghost.progress) {
            behind(card, ghost.race_time, fpu::div(1.0, ghost.progress.0))
        } else {
            F::ZERO
        };
    }
}

/// How far behind the pace a card is: its race time less the time the pace (`per_foot` seconds a
/// foot, over `pace_time`'s whole) takes to its progress.
fn behind(card: &LapCard, pace_time: F, per_foot: f32) -> F {
    F(fpu::sub(card.race_time.0, fpu::mul(per_foot, fpu::mul(pace_time.0, card.progress.0))))
}

/// The gaps, each tick of a race nobody has finished: every card behind the leader not yet done is as
/// far behind as its race time less the leader's time to its progress at the leader's pace (none
/// below 0; with the leader nowhere yet, its race time); the leader's own gap is the second card's,
/// negated (0 alone). `cards` in list order, not empty.
pub fn gaps(cards: &mut [LapCard]) {
    let Some((lead, rest)) = cards.split_first_mut() else { return };
    let per_foot = if lt(F::ZERO, lead.progress) { fpu::div(1.0, lead.progress.0) } else { 0.0 };
    for c in rest.iter_mut() {
        if c.flags & card_flags::OUT == 0 {
            c.gap = behind(c, lead.race_time, per_foot);
            if lt(c.gap, F::ZERO) {
                c.gap = F::ZERO;
            }
        }
    }
    lead.gap = match rest.first() {
        Some(second) => F(fpu::neg(second.gap.0)),
        None => F::ZERO,
    };
}

// --- the track's race file (S13e) -------------------------------------------------------------------

/// What a gate is when the original makes one, before anything is read into it: no flags, its three
/// directions zero with `w` 1, no width or height, and a reach of −1 (test at any distance).
impl Gate {
    pub fn new() -> Gate {
        let zero = Vf([0.0, 0.0, 0.0, 1.0]);
        Gate { flags: 0, centre: zero, normal: zero, across: zero, width: F::ZERO, height: F::ZERO, reach_sq: F(-1.0) }
    }
}

impl Default for Gate {
    fn default() -> Gate {
        Gate::new()
    }
}

/// A track's race file (`tracks\<track>\<track>.tdf`, in the track's second resource,
/// `Tracks\<track>_2.res`), as the track object reads it: the riders' paths round the course, and the
/// gates the race activity starts from.
///
/// The file is a run of records, each a tag byte and its little-endian words; [`TrackFile::parse`]
/// takes them in order, as the original does, so a scale or an offset applies to the points after it.
/// Points are stored `offset + raw × scale` (`y` only scaled); a gate's across direction is
/// normalised and its width scaled, its normal kept as written.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackFile {
    /// The x and z offsets every point is moved by (`+0x80`, `+0x88`). The original never sets them
    /// before reading: a file without them gets whatever the object's memory held. The port starts
    /// them at 0.
    pub offset_x: F,
    pub offset_z: F,
    /// The scale every point and gate width is multiplied by (`+0x90`), 1 until the file sets it.
    pub scale: F,
    /// Tag `0x29`'s word (`+0x34`).
    pub word_34: u32,
    /// The paths, by index (`+0x50`, count `+0x40`); a slot the file names no path for is `None`.
    pub paths: Vec<Option<TrackPath>>,
    /// How many nodes and checkpoints the file made (`+0x44`, `+0x48`): every one, including those a
    /// path read again left behind. Each node and checkpoint's id is the count before it.
    pub nodes_made: i32,
    pub checkpoints_made: i32,
    /// The gates: the grid's (`+0x58`, tag `0x0f`), the finish line's (`+0x5c`, tag `0x10`) and a third
    /// (`+0x60`, tag `0x19`). Each is [`flags`] 3 and 150 high once the file is read.
    pub grid: Option<Gate>,
    pub line: Option<Gate>,
    pub third: Option<Gate>,
    /// Tags `0x13`–`0x16` and `0x28` (`+0x64`..`+0x74`), the object's defaults 100, 0.9, 1.5, 0.5 and
    /// 4 until the file sets them. What reads them is not ported.
    pub tunables: [F; 5],
}

/// One path of a [`TrackFile`]: a record of `0x30` bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackPath {
    /// The flags (`+0`): the file's word with `0xc1` set.
    pub flags: u32,
    /// The length (`+8`): the sum of its nodes' spans, in the order they were read.
    pub length: F,
    /// Tag `0x18`'s word (`+0xc`).
    pub word_0c: u32,
    /// Its nodes, in order (the list at `+0x10`, last `+0x14`).
    pub nodes: Vec<PathNode>,
    /// The paths it leads on to (`+0x2c`, count `+0x18`), or `None` if the file gave it no links.
    pub links: Option<Vec<Link>>,
    /// The checkpoint count the file gives (`+0x20`), and its checkpoints (the list at `+0x24`).
    pub checkpoint_count: i32,
    pub checkpoints: Vec<Checkpoint>,
}

/// One node of a path: a record of `0x60` bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct PathNode {
    /// Its id (`+0`).
    pub id: i32,
    /// Tag `0x17`'s word (`+4`).
    pub word_04: u32,
    /// Its three points (`+0x10`, `+0x20`, `+0x30`; tags 9, `0xa`, `0xb`), `w` 0. The first is the
    /// path's own line.
    pub points: [Vf; 3],
    /// The flat (xz) distance from its first point to the next node's (`+0x40`), 0 on the last.
    pub span: F,
    /// Tags `0x12`, `0x1a`, `0x1b` (`+0x44`, `+0x48`, `+0x4c`); the first is −1 until set.
    pub word_44: u32,
    pub word_48: u32,
    pub word_4c: u32,
}

/// Where a path's link leads once the file is read: a path by its index (whose slot may be empty), or
/// the number as the file wrote it when that is no index (only the path count itself passes the
/// reader's check).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Path(usize),
    Raw(i32),
}

/// One checkpoint of a path: a record of `0x24` bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct Checkpoint {
    /// Its id (`+0`).
    pub id: i32,
    /// Tags `0x21`, `0x20`, `0x1e`, `0x1f` (`+4`, `+8`, `+0xc`, `+0x10`).
    pub word_04: u32,
    pub word_08: u32,
    pub word_0c: u32,
    pub word_10: u32,
    /// Its parts, in order (the list at `+0x14`, last `+0x18`).
    pub parts: Vec<CheckPart>,
}

/// One part of a checkpoint: a record of `0x40` bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckPart {
    /// Tag `0x23`'s word (`+4`).
    pub word_04: u32,
    /// Its point (`+0x10`, tag `0x24`), `w` 0.
    pub point: Vf,
    /// The flat distance from its point to the next part's (`+0x20`), 0 on the last.
    pub span: F,
    /// Tags `0x25`, `0x26` (`+0x24`, `+0x28`).
    pub word_24: u32,
    pub word_28: u32,
}

/// Why [`TrackFile::parse`] gave up. The ones the original answers by failing the load say so; the
/// others are files on which the original reads or writes memory it never allocated, which the port
/// does not imitate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The original fails the load (a count below 1, a path index out of range, a path record with no
    /// path, a negative count, a link out of range).
    Fails,
    /// A record before the one it hangs off: a path record before any path, a checkpoint before any
    /// path, a part's point in a checkpoint with no parts, no path count at all.
    Stray,
    /// A record running past the end of the file.
    Truncated,
    /// A link at the index one past the path's links, which the original's check lets through and
    /// writes past the end of the array.
    PastLinks,
}

/// Where the next record of a list is hooked in: the list's head, or the last record's next.
#[derive(Clone, Copy)]
enum Hook {
    None,
    Head(usize),
    After(usize),
}

struct Record {
    rec: Checkpoint,
    next: Option<usize>,
    parts: Option<usize>,
    last_part: Option<usize>,
}

struct Part {
    part: CheckPart,
    next: Option<usize>,
    before: Option<usize>,
}

struct PathRec {
    path: TrackPath,
    checkpoints: Option<usize>,
}

struct Words<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Words<'_> {
    fn word(&mut self) -> Result<u32, Refused> {
        let w = self.bytes.get(self.at..self.at + 4).ok_or(Refused::Truncated)?;
        self.at += 4;
        Ok(u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
    }
    fn f(&mut self) -> Result<F, Refused> {
        Ok(F(f32::from_bits(self.word()?)))
    }
}

/// The flat length of `a − b`: a VU subtract, then `mula` x, `madd` z and a square root.
fn flat_span(a: Vf, b: Vf) -> F {
    let d = a.sub(b);
    F(fpu::sqrt(fpu::madd(fpu::mula(d.x(), d.x()), d.z(), d.z())))
}

impl TrackFile {
    /// The track object as its constructor leaves it, before a file is read.
    pub fn new() -> TrackFile {
        TrackFile {
            offset_x: F::ZERO,
            offset_z: F::ZERO,
            scale: F::ONE,
            word_34: 0,
            paths: Vec::new(),
            nodes_made: 0,
            checkpoints_made: 0,
            grid: None,
            line: None,
            third: None,
            tunables: [F(100.0), F(0.9), F(1.5), F(0.5), F(4.0)],
        }
    }

    /// A point as the reader stores it: `offset + raw × scale` on x and z, `raw × scale` on y.
    fn place(&self, x: F, y: F, z: F) -> [F; 3] {
        let s = self.scale.0;
        [
            F(fpu::add(self.offset_x.0, fpu::mul(x.0, s))),
            F(fpu::mul(y.0, s)),
            F(fpu::add(self.offset_z.0, fpu::mul(z.0, s))),
        ]
    }

    fn point(&self, w: &mut Words) -> Result<Vf, Refused> {
        let (x, y, z) = (w.f()?, w.f()?, w.f()?);
        let [x, y, z] = self.place(x, y, z);
        Ok(Vf([x.0, y.0, z.0, 0.0]))
    }

    /// A gate record: centre, normal and across, three words each, and the width.
    fn gate(&self, w: &mut Words) -> Result<Gate, Refused> {
        let mut v = [F::ZERO; 10];
        for x in v.iter_mut() {
            *x = w.f()?;
        }
        let mut g = Gate::new();
        g.normal = Vf([v[3].0, v[4].0, v[5].0, 1.0]);
        g.across = crate::air::normalize(Vf([v[6].0, v[7].0, v[8].0, 1.0]));
        g.width = F(fpu::mul(v[9].0, self.scale.0));
        let [x, y, z] = self.place(v[0], v[1], v[2]);
        g.centre = Vf([x.0, y.0, z.0, 1.0]);
        g.flags = 3;
        Ok(g)
    }

    /// Read a race file, record by record, as the track object's loader does.
    ///
    /// The reader stops when a tag byte is the file's last: a record with no words there is never
    /// taken. Unknown tags are skipped (their byte only). Reading a path's index again makes a new,
    /// empty path in its slot; reading the path count again empties every slot. Once the records are
    /// read, each link that names a path index is turned into that path, and every gate is made 150
    /// high.
    pub fn parse(bytes: &[u8]) -> Result<TrackFile, Refused> {
        let mut t = TrackFile::new();
        let mut w = Words { bytes, at: 0 };
        // The path records, the slots naming them, and the current one.
        let mut paths: Vec<PathRec> = Vec::new();
        let mut slots: Option<Vec<Option<usize>>> = None;
        let mut cur: Option<usize> = None;
        let mut records: Vec<Record> = Vec::new();
        let mut parts: Vec<Part> = Vec::new();
        let mut record_hook = Hook::None;
        let mut part_hook = Hook::None;
        let mut record: Option<usize> = None;
        let mut part: Option<usize> = None;
        let mut link_count = 0i32;
        loop {
            let Some(&tag) = bytes.get(w.at) else { break };
            w.at += 1;
            if w.at >= bytes.len() {
                break;
            }
            let count = slots.as_ref().map_or(0, |s| s.len() as i32);
            match tag {
                0x01 => {
                    let n = w.word()? as i32;
                    if n < 1 {
                        return Err(Refused::Fails);
                    }
                    slots = Some(vec![None; n as usize]);
                }
                0x02 => t.offset_x = w.f()?,
                0x03 => t.offset_z = w.f()?,
                0x04 => t.scale = w.f()?,
                0x29 => t.word_34 = w.word()?,
                0x05 => {
                    let k = w.word()? as i32;
                    if k < 0 || k >= count {
                        return Err(Refused::Fails);
                    }
                    paths.push(PathRec {
                        path: TrackPath {
                            flags: 0,
                            length: F::ZERO,
                            word_0c: 0,
                            nodes: Vec::new(),
                            links: None,
                            checkpoint_count: 0,
                            checkpoints: Vec::new(),
                        },
                        checkpoints: None,
                    });
                    let p = paths.len() - 1;
                    slots.as_mut().expect("a count")[k as usize] = Some(p);
                    cur = Some(p);
                    record_hook = Hook::Head(p);
                }
                0x06 | 0x07 | 0x18 | 0x08 | 0x0d | 0x1c => {
                    let p = cur.ok_or(Refused::Stray)?;
                    let path = &mut paths[p].path;
                    match tag {
                        0x06 => path.flags = w.word()? | 0xc1,
                        0x07 => {
                            w.word()?;
                        }
                        0x18 => path.word_0c = w.word()?,
                        0x08 => {
                            path.nodes.push(PathNode {
                                id: t.nodes_made,
                                word_04: 0,
                                points: [Vf::ZERO; 3],
                                span: F::ZERO,
                                word_44: 0xbf80_0000,
                                word_48: 0,
                                word_4c: 0,
                            });
                            t.nodes_made = t.nodes_made.wrapping_add(1);
                        }
                        0x0d => {
                            let n = w.word()? as i32;
                            if n < 0 {
                                return Err(Refused::Fails);
                            }
                            path.links = Some(vec![Link::Raw(0); n as usize]);
                            link_count = n;
                        }
                        _ => {
                            path.checkpoint_count = w.word()? as i32;
                            record_hook = Hook::Head(p);
                            if path.checkpoint_count < 0 {
                                return Err(Refused::Fails);
                            }
                        }
                    }
                }
                0x17 | 0x09 | 0x0a | 0x0b | 0x12 | 0x1a | 0x1b => {
                    let p = cur.ok_or(Refused::Stray)?;
                    if paths[p].path.nodes.is_empty() {
                        return Err(Refused::Fails);
                    }
                    let value = match tag {
                        0x09 | 0x0a | 0x0b => None,
                        _ => Some(w.word()?),
                    };
                    let point = match tag {
                        0x09 | 0x0a | 0x0b => Some(t.point(&mut w)?),
                        _ => None,
                    };
                    let path = &mut paths[p].path;
                    let last = path.nodes.len() - 1;
                    let node = &mut path.nodes[last];
                    match (tag, value, point) {
                        (0x17, Some(v), _) => node.word_04 = v,
                        (0x12, Some(v), _) => node.word_44 = v,
                        (0x1a, Some(v), _) => node.word_48 = v,
                        (0x1b, Some(v), _) => node.word_4c = v,
                        (0x0a, _, Some(at)) => node.points[1] = at,
                        (0x0b, _, Some(at)) => node.points[2] = at,
                        (_, _, Some(at)) => {
                            node.points[0] = at;
                            if last > 0 {
                                let span = flat_span(at, path.nodes[last - 1].points[0]);
                                path.nodes[last - 1].span = span;
                                path.length = F(fpu::add(path.length.0, span.0));
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                0x0c => {
                    w.word()?;
                }
                0x1d => {
                    records.push(Record {
                        rec: Checkpoint {
                            id: t.checkpoints_made,
                            word_04: 0,
                            word_08: 0,
                            word_0c: 0,
                            word_10: 0,
                            parts: Vec::new(),
                        },
                        next: None,
                        parts: None,
                        last_part: None,
                    });
                    let r = records.len() - 1;
                    t.checkpoints_made = t.checkpoints_made.wrapping_add(1);
                    match record_hook {
                        Hook::None => return Err(Refused::Stray),
                        Hook::Head(p) => paths[p].checkpoints = Some(r),
                        Hook::After(q) => records[q].next = Some(r),
                    }
                    record_hook = Hook::After(r);
                    part_hook = Hook::Head(r);
                    record = Some(r);
                }
                0x21 | 0x1e | 0x1f | 0x20 => {
                    let r = record.ok_or(Refused::Fails)?;
                    let v = w.word()?;
                    let rec = &mut records[r].rec;
                    match tag {
                        0x21 => rec.word_04 = v,
                        0x1e => rec.word_0c = v,
                        0x1f => rec.word_10 = v,
                        _ => rec.word_08 = v,
                    }
                }
                0x22 => {
                    let r = record.ok_or(Refused::Fails)?;
                    parts.push(Part {
                        part: CheckPart { word_04: 0, point: Vf::ZERO, span: F::ZERO, word_24: 0, word_28: 0 },
                        next: None,
                        before: records[r].last_part,
                    });
                    let s = parts.len() - 1;
                    match part_hook {
                        Hook::None => return Err(Refused::Stray),
                        Hook::Head(q) => records[q].parts = Some(s),
                        Hook::After(q) => parts[q].next = Some(s),
                    }
                    part_hook = Hook::After(s);
                    records[r].last_part = Some(s);
                    part = Some(s);
                }
                0x23 | 0x25 | 0x26 => {
                    let s = part.ok_or(Refused::Fails)?;
                    let v = w.word()?;
                    let it = &mut parts[s].part;
                    match tag {
                        0x23 => it.word_04 = v,
                        0x25 => it.word_24 = v,
                        _ => it.word_28 = v,
                    }
                }
                0x24 => {
                    let (Some(s), Some(r)) = (part, record) else { return Err(Refused::Fails) };
                    parts[s].part.point = t.point(&mut w)?;
                    let last = records[r].last_part.ok_or(Refused::Stray)?;
                    if let Some(before) = parts[last].before {
                        parts[before].part.span = flat_span(parts[last].part.point, parts[before].part.point);
                    }
                }
                0x0e => {
                    let p = cur.ok_or(Refused::Stray)?;
                    if paths[p].path.links.is_none() {
                        return Err(Refused::Fails);
                    }
                    let (c, v) = (w.word()? as i32, w.word()? as i32);
                    if c < 0 || link_count < c || v < 0 || count < v {
                        return Err(Refused::Fails);
                    }
                    let links = paths[p].path.links.as_mut().expect("links");
                    match links.get_mut(c as usize) {
                        Some(l) => *l = Link::Raw(v),
                        None => return Err(Refused::PastLinks),
                    }
                }
                0x0f => t.grid = Some(t.gate(&mut w)?),
                0x10 => t.line = Some(t.gate(&mut w)?),
                0x19 => t.third = Some(t.gate(&mut w)?),
                0x13 => t.tunables[0] = w.f()?,
                0x14 => t.tunables[1] = w.f()?,
                0x15 => t.tunables[2] = w.f()?,
                0x16 => t.tunables[3] = w.f()?,
                0x28 => t.tunables[4] = w.f()?,
                _ => {}
            }
        }
        let slots = slots.ok_or(Refused::Stray)?;
        let count = slots.len() as i32;
        let mut out: Vec<Option<TrackPath>> = Vec::with_capacity(slots.len());
        for slot in &slots {
            out.push(slot.map(|p| {
                let mut path = paths[p].path.clone();
                if let Some(links) = path.links.as_mut() {
                    for l in links.iter_mut() {
                        if let Link::Raw(v) = *l {
                            if (0..count).contains(&v) {
                                *l = Link::Path(v as usize);
                            }
                        }
                    }
                }
                let mut r = paths[p].checkpoints;
                while let Some(k) = r {
                    let mut rec = records[k].rec.clone();
                    let mut s = records[k].parts;
                    while let Some(j) = s {
                        rec.parts.push(parts[j].part.clone());
                        s = parts[j].next;
                    }
                    path.checkpoints.push(rec);
                    r = records[k].next;
                }
                path
            }));
        }
        t.paths = out;
        for g in [&mut t.grid, &mut t.line, &mut t.third].into_iter().flatten() {
            g.height = F(150.0);
        }
        Ok(t)
    }
}

impl Default for TrackFile {
    fn default() -> TrackFile {
        TrackFile::new()
    }
}

/// A prop of the track's scene the supercross loader stands a gate on (the drop gate, the finish
/// gate): its frame's across and look rows and its position (`+0x80`, `+0xa0`, `+0xb0`), and its
/// model's bound — half its width, and its height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prop {
    pub across: Vf,
    pub look: Vf,
    pub at: Vf,
    pub half_width: F,
    pub height: F,
}

/// The world's up, as the grid gate's normal is crossed with it.
const UP: Vf = Vf([0.0, 1.0, 0.0, 1.0]);

/// `a × b`, keeping `a`'s `w`: the `vopmula`/`vopmsub` pair writes only `xyz` of the register that
/// held `a`.
fn cross(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, crate::air::cross(a, b))
}

impl Supercross {
    /// The supercross loader's gates off the drop-gate prop: the track's grid gate (when the file had
    /// one) stood in it, facing `across × up`; and the start gate, flagged `0x02`, 1000 high, facing
    /// the prop's look and set back 4.5 behind it. Both are the prop's width (twice its half width).
    pub fn place_drop_gate(drop: &Prop, grid: Option<&mut Gate>, start: &mut Gate) {
        let width = F(fpu::mul(2.0, drop.half_width.0));
        if let Some(g) = grid {
            g.centre = drop.at;
            g.width = width;
            g.height = drop.height;
            g.across = drop.across;
            g.normal = cross(drop.across, UP);
        }
        start.flags |= 0x02;
        start.centre = start.centre.merge(Dest::XYZ, drop.at);
        start.width = width;
        start.height = F(1000.0);
        start.across = start.across.merge(Dest::XYZ, drop.across);
        start.normal = start.normal.merge(Dest::XYZ, drop.look);
        let back = start.normal.mul_q(4.5);
        start.centre = start.centre.merge(Dest::XYZ, start.centre.sub(back));
    }

    /// The finish line off the finish-gate prop: stood at the prop, its width and height, across it,
    /// and facing the way the riders go there — `ahead` (the track's path direction at the line's
    /// spot, which the caller finds from the line's new centre) made square to the across direction:
    /// `normalize(across × (ahead × across))`.
    pub fn place_finish_line(finish: &Prop, ahead: Vf, line: &mut Gate) {
        line.centre = line.centre.merge(Dest::XYZ, finish.at);
        line.width = F(fpu::mul(2.0, finish.half_width.0));
        line.height = finish.height;
        line.across = line.across.merge(Dest::XYZ, finish.across);
        let side = cross(ahead, line.across);
        line.normal = crate::air::normalize(cross(line.across, side));
    }
}
