//! The vehicle's animation state machine: which clips the bike plays for the state it is in, and the
//! one entry point of the motion library that nothing else in the game reaches.
//!
//! [`crate::motion`] is the rider's animation section and the library it ends in. This is the *other*
//! caller of that library — the bike's own, which is a bigger machine than the rider's and the only
//! live caller of two things the library has that the rider never asks for: the four-clip blend
//! ([`Blend::Four`], asked for from two of the arms below) and a sixth entry point nothing else in the
//! image touches ([`Blend::Pair`]). A port of the library with no port of this is a library with no
//! caller, which is why this module exists.
//!
//! ## The frame, in order
//!
//! [`Driver::frame`] is one call of the original's step, and it does these things in this order:
//!
//! 1. **the placement** — the bike's node is moved to where the request says, unless the request is
//!    not holding it and the actor is placing itself ([`Frame::place`]);
//! 2. **the restart test** — the state the caller asked for is not the one running, or the request's
//!    own count has moved ([`Frame::restart`]), which snapshots the pose and starts the cross-fade at
//!    one;
//! 3. **the fade rate** — a sixty-entry switch on the *asked-for* state, picking one of five
//!    configured rates (and, for the two reverse states, zeroing the reverse clock);
//! 4. **the state latch** — the asked-for state becomes the running one, the cross-fade ramp decays by
//!    `rate x dt`, and the holding flag is let go of once the ramp runs out;
//! 5. **the two lags** — the mix axis and the phase axis, each eased towards the request's target by
//!    its own time constant ([`Lag`]);
//! 6. **the selection** — a sixty-entry jump table on the running state, one arm per group of states,
//!    which answers with a [`Blend`];
//! 7. **the call** — the weights are clamped and one of the library's five entry points is called
//!    ([`Blend::run`]), or none at all when the state is out of range.
//!
//! ## The arms
//!
//! The states past the 24 gesture slots are exactly the 36 clips the loader looks up by name
//! ([`POSE_CLIPS`]), and the arms are the groups of those names. A state in a group that is not the
//! group's own state asks for nothing: its clip is reached through the group's arm, not through its
//! own.
//!
//! ```text
//!   state                      clip                         arm
//!   0..=23                     the 24 gesture slots         the stunt cross-fade, mode 5 or 1
//!   24..=26 turn_stop_*        \
//!   27..=29 turn_still_*        }  the 3x3 pose grid         mode 4 or 2, from state 30 only
//!   30..=32 turn_riding_*      /
//!   33, 34  reverse_l/r           the reverse pair          mode 2, phase from its own clock
//!   35..=37 air_*                 the airborne row          mode 2, from state 35 only
//!   38      backflip                                        mode 1 at a fixed tenth
//!   39      nohandedlander_*                                mode 1
//!   40..=45 wreck/hold group      played once               a cross into the clip, then mode 0
//!   46..=59 get_ready group       played once               the same
//! ```
//!
//! ## Which state
//!
//! The function that *decides* the state is the animated object's own update, which in the original
//! is the step's caller: it chooses, calls the step, and files two fields away for the next frame.
//! That is [`Update`], and the two fields are what make the restart test and the stunt cross-fade
//! read one frame behind. Nothing here draws: the clips are handles
//! ([`crate::motion::ClipId`]) and the pose they sample is the renderer's, exactly as in
//! [`crate::motion`].

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::chassis::turn;
use crate::hash;
use crate::motion::{Animations, Clip, ClipId, Played, Player, Put, clamp01, remap};
use crate::rng::Rng;
use crate::trick::{SLOTS, Stunt};
use ps2_float::{Dest, F, Vf, libm};

/// How many animation states the machine has: the length of both of its jump tables, and of the clip
/// table they index.
pub const STATES: usize = 60;

/// The 36 clips the loader looks up by bare name, in the table slots `24..60`.
///
/// They are not a second table: they are the slots of the states whose arms never index the table, so
/// a state's clip and a group's row live in the same array. The order is the loader's own, and it is
/// what makes every state's meaning legible — the grid rows, the reverse pair, the airborne row, and
/// the two groups that are played once and then left alone.
pub const POSE_CLIPS: [&str; STATES - SLOTS] = [
    "turn_stop_Fwd",
    "turn_stop_Mid",
    "turn_stop_Bck",
    "turn_still_Fwd",
    "turn_still_Mid",
    "turn_still_Bck",
    "turn_riding_Fwd",
    "turn_riding_Mid",
    "turn_riding_Bck",
    "reverse_l",
    "reverse_r",
    "air_Fwd",
    "air_Mid",
    "air_Bck",
    "backflip",
    "nohandedlander_left_right",
    "wreck_air_flail",
    "hold_head",
    "hold_elbow_left",
    "hold_elbow_right",
    "hold_knee_left",
    "hold_knee_right",
    "get_ready_breathe",
    "get_ready_idle_01",
    "get_ready_idle_02",
    "get_ready_idle_03",
    "get_ready_idle_04",
    "get_ready_idle_05",
    "get_ready_idle_06",
    "get_ready_idle_07",
    "get_ready_idle_08",
    "get_ready_idle_09",
    "get_ready_idle_10",
    "get_ready_start_pos_01",
    "get_ready_start_pos_02",
    "get_ready_start_pos_03",
];

/// The first of the three `turn_stop_*` clips: the pose grid's stopped row.
pub const TURN_STOP: i32 = 24;
/// The first of the three `turn_still_*` clips: the grid's standing-still row.
pub const TURN_STILL: i32 = 27;
/// The first of the three `turn_riding_*` clips, and **the state the whole grid is selected from** —
/// the other eight of its nine states ask for nothing of their own.
pub const TURN_RIDING: i32 = 30;
/// `reverse_l`, the first of the reverse pair's two states. Both states select the same pair.
pub const REVERSE_L: i32 = 33;
/// `reverse_r`.
pub const REVERSE_R: i32 = 34;
/// `air_Fwd`, and the state the airborne row is selected from.
pub const AIR: i32 = 35;
/// `backflip`: the one state that plays its clip at a fixed tenth of its length.
pub const BACKFLIP: i32 = 38;
/// `nohandedlander_left_right`.
pub const NO_HANDER: i32 = 39;
/// `wreck_air_flail`, the first of the wreck-and-hold group.
pub const HOLD: i32 = 40;
/// `get_ready_breathe`, the first of the start-line group.
pub const GET_READY: i32 = 46;
/// `get_ready_start_pos_01`, the first of the three start poses — the tail of the start-line group,
/// which is the one part of it a cut request does not shorten.
pub const START_POS: i32 = 57;
/// `get_ready_idle_01`, the first of the ten idles in the start-line group. The chooser draws from
/// them twice: once for the pose a body waits on the gate in, and once for the fidget a body plays
/// when it has been standing still for long enough.
pub const IDLE: i32 = GET_READY + 1;
/// How many of those idles there are.
pub const IDLES: i32 = 10;
/// How many start poses there are, which is the other list the chooser draws from.
pub const START_POSES: i32 = 3;

// ---------------------------------------------------------------------------------------------------
// which clip
// ---------------------------------------------------------------------------------------------------

/// An animation registered on the vehicle from outside the state machine: a key the caller recognises
/// and up to two clips to mix.
///
/// The engine's own "add an animation" call appends one of these by name, looking each name up and
/// binding it there and then, and skipping a name it already has a record for. An empty name leaves
/// that half of the record empty, which the selection notices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extra {
    /// Whatever the caller identifies the record by. The state machine never reads it; the caller
    /// finds its own record with it and then points [`Driver::extra`] at the index.
    pub key: i32,
    /// The clip the mix interpolates towards.
    pub a: Option<ClipId>,
    /// The clip it interpolates from.
    pub b: Option<ClipId>,
}

/// The clip the vehicle plays per animation state: one sixty-entry table, plus the records added from
/// outside.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Library {
    /// Per state, the handle the loader left. The first [`SLOTS`] are the gesture slots, the rest are
    /// [`POSE_CLIPS`] in order.
    pub clip: [Option<ClipId>; STATES],
    /// The records added from outside, in the order they were added — which is the order
    /// [`Driver::extra`] indexes.
    pub extra: Vec<Extra>,
}

impl Default for Library {
    fn default() -> Library {
        Library { clip: [None; STATES], extra: Vec::new() }
    }
}

impl Library {
    /// The name a gesture slot's clip is looked up by: the vehicle's prefix, then the stunt's `Name`
    /// from `stunts.ini`, into a buffer of 32 bytes — so a long enough name is truncated, which the
    /// port keeps. There is no `bike_` in it: that is the rider's loader, not this one.
    pub fn stunt_name(prefix: &str, stunt: &str) -> String {
        let whole = format!("{prefix}{stunt}");
        String::from_utf8_lossy(&whole.as_bytes()[..whole.len().min(31)]).into_owned()
    }

    /// The vehicle's clip table, as its loader fills it.
    ///
    /// Two passes over the 24 gesture slots, as the rider's loader does it — the first loads and binds,
    /// the second asks for the handle — and then the 36 named clips, which are only asked for.
    ///
    /// The one difference from the rider's, and it is not a small one: **every** slot is loaded, with
    /// no `UsedForBike` gate. The flag that decides which stunts the rider has an animation for has no
    /// say over the bike's.
    pub fn load(prefix: &str, stunts: &[Stunt], anim: &mut impl Animations) -> Library {
        for stunt in stunts.iter().take(SLOTS) {
            anim.load(&Library::stunt_name(prefix, &stunt.name));
        }
        let mut lib = Library::default();
        for (slot, stunt) in stunts.iter().take(SLOTS).enumerate() {
            lib.clip[slot] = anim.bound(&Library::stunt_name(prefix, &stunt.name));
        }
        for (i, name) in POSE_CLIPS.iter().enumerate() {
            lib.clip[SLOTS + i] = anim.bound(name);
        }
        lib
    }

    /// The handle in a state's slot, or `None` for a state outside the table.
    pub fn clip(&self, state: i32) -> Option<ClipId> {
        usize::try_from(state).ok().and_then(|s| self.clip.get(s).copied()).flatten()
    }

    /// The record [`Driver::extra`] points at, or an empty one when it points outside the list.
    ///
    /// The original's accessor does not check: an index past the end reads whatever is behind the
    /// list. The port answers with an empty record, which selects nothing.
    pub fn extra(&self, index: i32) -> Extra {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.extra.get(i).copied())
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------------------------------
// the two lags
// ---------------------------------------------------------------------------------------------------

/// One of the two axes the selection mixes along, eased towards its target instead of jumping to it.
///
/// The vehicle holds two of these by pointer and the step walks both every frame, in order: the mix
/// axis first, then the phase axis. The ratio is not a constant — it is recomputed every frame from
/// `dt` and written back into the record, which is how a frame longer than the time constant becomes
/// a snap rather than an overshoot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lag {
    /// Where the axis is now, and what the selection reads.
    pub value: F,
    /// How long it takes to get there, in seconds.
    pub tau: F,
    /// `min(dt, tau) / tau`, left behind by the last step. The original keeps it in the record, so the
    /// port does too.
    pub ratio: F,
}

impl Lag {
    /// An axis with a time constant and nothing yet in it.
    pub fn new(tau: F) -> Lag {
        Lag { value: F::ZERO, tau, ratio: F::ZERO }
    }

    /// One frame towards `target`, returning where it got to.
    ///
    /// `min(dt, tau)` is the whole of the clamp: a frame at least as long as the time constant earns a
    /// ratio of exactly one and the axis lands on the target. A time constant of zero divides by it.
    pub fn step(&mut self, dt: F, target: F) -> F {
        self.ratio = dt.min(self.tau) / self.tau;
        self.value = self.value + self.ratio * (target - self.value);
        self.value
    }
}

// ---------------------------------------------------------------------------------------------------
// what the step is handed
// ---------------------------------------------------------------------------------------------------

/// The animated object's own numbers: the five fade rates, the two phases the stunt cross-fade is
/// built from, and the pose grid's three thresholds.
///
/// The step only reads them. [`Update::serviced`] writes one, [`Params::start_phase`], because in the
/// original that field lives on the object beside the rest of these and is filed away by the update
/// after every step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    /// The rate a gesture slot's cross-fade falls at, and `nohandedlander`'s.
    pub rate_stunt: F,
    /// The rate everything with no rate of its own falls at — including a state outside the table.
    pub rate_default: F,
    /// The rate the airborne row and the backflip fall at.
    pub rate_pose: F,
    /// The rate the wreck-and-hold group falls at.
    pub rate_hold: F,
    /// The rate the start-line group falls at. A request asking to cut gets zero instead, for every
    /// state of the group bar the three start poses.
    pub rate_ready: F,
    /// The longest the stunt cross-fade may take, in seconds, before the clip's own half-length caps
    /// it. See [`Blend::Pair`].
    pub cross_cap: F,
    /// Where the clip the stunt cross-fade is coming *out of* is sampled from on the frame it starts.
    pub start_phase: F,
    /// The pose grid's three thresholds on [`Request::band`], low to high.
    pub band: [F; 3],
}

/// What the caller asks of the step this frame: where the bike is, which way the two axes want to go,
/// and the two flags that change what the step does with them.
///
/// This is the vehicle's request block, which the rest of the vehicle writes and the step only reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Request {
    /// The caller is holding the bike in place: the placement is forced even when the actor is placing
    /// itself, the fade rate and the ramp are zeroed, and both axes are snapped to their targets
    /// rather than eased.
    pub held: bool,
    /// Take the position straight from [`Request::base`], all four lanes, instead of turning the
    /// offset by the rotation and adding it.
    pub at_base: bool,
    /// Cut the start-line group short: its fade rate becomes zero.
    pub cut: bool,
    /// The offset the rotation turns, when the position is not taken from the base. Its `w` is the
    /// one the placement comes out with.
    pub offset: Vf,
    /// Where the bike is.
    pub base: Vf,
    /// Which way it is facing, as a quaternion.
    pub rotation: Vf,
    /// Where the phase axis wants to be. [`f32::MAX`] means "leave it" to a held request.
    pub phase_axis: F,
    /// Where the mix axis wants to be, same convention.
    pub mix_axis: F,
    /// The number the pose grid's three thresholds cut up, and the reverse clock's own input. Forward
    /// speed, as the grid reads it.
    pub band: F,
    /// How far through the running gesture the bike is, 0 to 1 — the trick clocks' `progress`, which
    /// is what the gesture slots' arm plays its clip at.
    pub progress: F,
    /// How many requests the caller has made. The restart test compares it against the count the step
    /// was last run with, so a repeat of the *same* state still restarts.
    pub seq: i32,
}

/// The id the gesture field holds when nothing is asked for: the slot count, one past the last
/// gesture slot.
pub const NO_GESTURE: i32 = SLOTS as i32;

/// The whole request block: the fields the step reads ([`Request`]), and the ones only the state
/// chooser reads.
///
/// One block in the original, two structs here, because [`Driver::frame`] was ported first and takes
/// the half it reads on its own. [`Update::want`] is handed this, reads both halves, and writes
/// exactly one field of it — the start-line flag, which it clears on the frame the gate opens.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Asked {
    /// What the step itself reads. Its [`Request::cut`] is the start-line flag the chooser branches
    /// on first, and the only field of the block anything in the port writes.
    pub request: Request,
    /// The body is wrecked. Nothing else gets a look in while it is.
    pub wrecked: bool,
    /// Which gesture slot is asked for, or [`NO_GESTURE`] for none.
    pub gesture: i32,
    /// The body is in a flip.
    pub flipping: bool,
    /// The body is in the air.
    pub airborne: bool,
    /// The landing is a no-hander, which takes the place of whatever gesture was asked for.
    pub no_hands: bool,
    /// Not zero while the bike is going backwards; which of the two reverse states runs is then the
    /// phase axis against a half.
    pub reversing: F,
    /// Not zero while the rider is being told to go. This is what opens the start gate.
    pub commanded: F,
    /// The number the start-line breathe is three and a half seconds less, less the start-line fade
    /// rate. What it measures is not known: the writer of this block is not located.
    pub settle: F,
}

// ---------------------------------------------------------------------------------------------------
// what the library is handed
// ---------------------------------------------------------------------------------------------------

/// Where the placement left the bike's node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// The position written to the node. `w` is the offset's, not the base's, unless the position came
    /// straight from the base.
    pub position: Vf,
    /// The rotation written to it, straight from the request.
    pub rotation: Vf,
}

/// Which of the motion library's five entry points the frame ends in, with its arguments.
///
/// The weights are clamped to `0..=1` here, which is the last thing the original does before the call.
/// Three of the five are the ones [`crate::motion::Blend`] names; the two that are not are the reason this
/// module has a blend of its own, and the handles are raw rather than slots because the vehicle's
/// table is indexed by state and has no slot names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Blend {
    /// Nothing is driven: whatever clip is playing advances by `dt` on its own
    /// ([`Player::advance`]). Every state with no arm of its own ends up here, and so does the
    /// wreck-and-hold group once its clip has been put on.
    Free,
    /// One clip at `phase` of its length, faded in from the snapshotted pose by `ramp`.
    One { phase: F, ramp: F, clip: Option<ClipId> },
    /// Two clips, both sampled at `phase` of the *first* one's length, mixed by `mix`, and faded in
    /// by `ramp`.
    Two { phase: F, mix: F, ramp: F, a: Option<ClipId>, b: Option<ClipId> },
    /// Four clips and three weights: two pairs, each mixed by `mix`, and the two results mixed by
    /// `cross`. The pose grid's two interpolating bands are the only callers of this entry in the
    /// whole image; it is [`crate::motion::Blend::Three`] on the library's side.
    Four {
        phase: F,
        mix: F,
        cross: F,
        ramp: F,
        a: Option<ClipId>,
        b: Option<ClipId>,
        c: Option<ClipId>,
        d: Option<ClipId>,
    },
    /// Two clips, **each at its own phase**, mixed by `mix` and faded in by `ramp`: the sixth entry
    /// point, which the gesture slots' arm is the only caller of.
    ///
    /// The difference from [`Blend::Two`] is the whole reason it exists. A two-clip blend samples both
    /// clips at the same number of *seconds*, which only makes sense for clips of the same length;
    /// this one gives each clip its own phase, so a gesture can cross out of the gesture that was
    /// running at whatever point *that* clip had reached. `mix` is 0 for all of `b` and 1 for all of
    /// `a`.
    Pair { phase_a: F, phase_b: F, mix: F, ramp: F, a: Option<ClipId>, b: Option<ClipId> },
}

impl Blend {
    /// Which entry point this is, by the number the original's dispatch switches on: 0 the
    /// free-running advance, 1 one clip, 2 two, 4 four clips and three weights, 5 the pair at two
    /// phases.
    ///
    /// **3 is not a mode**, here as in [`crate::motion::Blend::mode`]: the dispatch is a chain of equality
    /// tests with no arm for it. Nothing in this machine ever asks for one, so there is no variant
    /// for it either.
    pub fn mode(self) -> u8 {
        match self {
            Blend::Free => 0,
            Blend::One { .. } => 1,
            Blend::Two { .. } => 2,
            Blend::Four { .. } => 4,
            Blend::Pair { .. } => 5,
        }
    }

    /// The clip the entry point drives the motion object's clock from, and the phase it drives it at.
    ///
    /// Every blend takes its clip time from its **first** clip and fires that one's action frames,
    /// however many it goes on to mix in — including the pair at two phases, whose second phase
    /// belongs to the clip it is coming out of and is not clocked.
    pub fn driven(self) -> Option<(F, Option<ClipId>)> {
        match self {
            Blend::Free => None,
            Blend::One { phase, clip, .. } => Some((phase, clip)),
            Blend::Two { phase, a, .. } => Some((phase, a)),
            Blend::Four { phase, a, .. } => Some((phase, a)),
            Blend::Pair { phase_a, a, .. } => Some((phase_a, a)),
        }
    }

    /// The clip time the pose comes from, which is the same question as which clock the entry point
    /// drives: a phase-driven blend samples at `duration x phase`, where [`Player::drive_at`] has just
    /// left it, and the free-running advance at the clock its own dt step reached.
    ///
    /// The same rule as [`crate::motion::Player::sample_time`], and the same reason it is named on
    /// this side: the clock is the simulation's and the drawing side has no business recomputing it.
    /// The pair at two phases needs one number more — its second clip's own time, which is
    /// [`Blend::pair_time`].
    pub fn sample_time(self, player: &Player) -> F {
        match self.driven() {
            None => player.time,
            Some(_) => player.prev,
        }
    }

    /// The *second* clip's clip time, for the pair at two phases: that clip's own duration times its
    /// own phase. `None` for every other entry point, none of which samples a second clip anywhere but
    /// at the first one's time.
    pub fn pair_time(self, b: &Clip) -> Option<F> {
        match self {
            Blend::Pair { phase_b, .. } => Some(b.duration * phase_b),
            _ => None,
        }
    }

    /// Whether a clip the entry point has only just been handed fires the empty window `0..0` when its
    /// clip time comes out as exactly zero. Only the one-clip entry tests its sample time against
    /// zero; the other three blends do not, so the pair at two phases fires like the two- and
    /// four-clip entries do.
    pub fn fires_empty(self) -> bool {
        !matches!(self, Blend::Free | Blend::One { .. })
    }

    /// Every clip the entry point samples, in the order it takes them as arguments.
    pub fn clips(self) -> Vec<Option<ClipId>> {
        match self {
            Blend::Free => Vec::new(),
            Blend::One { clip, .. } => vec![clip],
            Blend::Two { a, b, .. } | Blend::Pair { a, b, .. } => vec![a, b],
            Blend::Four { a, b, c, d, .. } => vec![a, b, c, d],
        }
    }

    /// The handle whose clip [`Blend::run`] needs this frame: the blend's first one, or the clip
    /// already playing when nothing is driven.
    pub fn clock_clip(self, player: &Player) -> Option<ClipId> {
        match self.driven() {
            None => player.clip,
            Some((_, clip)) => clip,
        }
    }

    /// The call itself: the clock the entry point drives, which is where a driven clip's action frames
    /// come from.
    ///
    /// `clip` is the clip behind the handle [`Blend::clock_clip`] names. A blend whose first handle is
    /// missing does nothing here — the original hands the entry point a null handle and the entry
    /// dereferences it, so the port diverges on a frame the original crashes on.
    pub fn run(self, player: &mut Player, dt: F, clip: &Clip) -> Played {
        match self.driven() {
            None => player.advance(dt, clip),
            Some((_, None)) => Played::default(),
            Some((phase, Some(id))) => Played {
                window: player.drive_at(phase, clip, id, self.fires_empty()),
                ..Played::default()
            },
        }
    }
}

/// The clip the step put on this frame, and the cross it put it on with.
///
/// `clip` is an option because the original does not look at the table slot before putting it on: a
/// state of the wreck-and-hold group with no clip of its own still calls the engine's play, which
/// stores the null handle — and a null handle in the clock is what "nothing is playing" is. Every
/// field [`Player::play`] writes is the same either way; only the clip differs, so [`Play::put`]
/// answers nothing and the caller clears the clock rather than putting a clip on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    /// The clip, or `None` when this state's slot in the table is empty.
    pub clip: Option<ClipId>,
    /// How long the cross into it takes: the fade rate the state was restarted with. Zero is a hard
    /// cut, which is what a held request's zeroed rate makes of it.
    pub length: F,
    /// The clip's own duration. Only a `keep_phase` play reads it, and this one never asks for one, so
    /// nothing downstream of here can tell what it holds.
    pub duration: F,
}

impl Play {
    /// What [`Player::play`] takes, when there is a clip to put on.
    pub fn put(self) -> Option<Put> {
        self.clip.map(|clip| Put::clip(clip, self.duration, self.length))
    }
}

/// What one frame of the step did, in the order it did it: the placement, the restart, the clip it put
/// on, and the call it ends in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// Where the placement left the node, or `None` when it was left to the actor itself.
    pub place: Option<Place>,
    /// The state started over: the caller owes the motion object a pose snapshot
    /// (`motion::Snapshot::take`, as the rider's restart does) before the call.
    pub restart: bool,
    /// The clip the step put on with a cross out of whatever was playing ([`Player::play`]). Only the
    /// two played-once groups ever ask for one, and only on the frame they start.
    pub play: Option<Play>,
    /// The entry point the frame ends in.
    pub blend: Blend,
}

// ---------------------------------------------------------------------------------------------------
// the machine
// ---------------------------------------------------------------------------------------------------

/// The vehicle's animation state, as the step keeps it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Driver {
    /// The state the caller has asked for. Written by [`Update::want`], the animated object's own
    /// update; the step latches it into [`Driver::state`] and never writes it.
    pub want: i32,
    /// The state running. Out of `0..60` and the frame makes no call at all.
    pub state: i32,
    /// The state that was running before the last restart, and the clip the stunt cross-fade comes out
    /// of. Only a value below [`SLOTS`] gets that cross-fade; anything else plays the new clip alone.
    pub prev: i32,
    /// The request count the restart test compares against. The step only reads it:
    /// [`Update::serviced`] is what stores the count, after the step, which is why a count that
    /// moves restarts the state on exactly one frame.
    pub seq: u8,
    /// The rate the cross-fade ramp falls at, picked out of [`Params`] by the asked-for state on every
    /// restart. Also the length of the cross the played-once groups put their clip on with.
    pub rate: F,
    /// How much of the previous pose is still showing: 1 the frame a state starts, 0 once the
    /// cross-fade is over. Every blend takes it as its last weight.
    pub ramp: F,
    /// A state has been started at least once. Set by every restart and never cleared here.
    pub started: bool,
    /// The vehicle is holding a pose, which the ramp running out lets go of — except on the backflip,
    /// which keeps it.
    pub holding: bool,
    /// This frame's selection came out of the clip table rather than out of a group's row. Cleared
    /// every frame and set by the two arms that index the table directly.
    pub from_table: bool,
    /// Where in the new clip the stunt cross-fade is, 0 to 1 ([`Blend::Pair`]'s first phase).
    pub cross: F,
    /// Where in the clip it is coming out of ([`Blend::Pair`]'s second phase).
    pub cross_prev: F,
    /// Where that clip was when the cross-fade started, which the second phase counts on from.
    pub cross_start: F,
    /// The reverse pair's own phase clock, which runs while the bike is going backwards and is zeroed
    /// the moment it is not.
    pub reverse: F,
    /// Which added record the selection takes instead of the state machine, or negative for none.
    pub extra: i32,
    /// The mix axis: which way along a row the selection is.
    pub mix: Lag,
    /// The phase axis: how far into the row's clips it is.
    pub phase: Lag,
}

/// The within-row weight: below the middle it is the low half stretched over `0..1`, above it the high
/// half.
fn half(mix: F) -> F {
    if mix < F(0.5) { F(2.0) * mix } else { F(2.0) * (mix - F(0.5)) }
}

/// The two clips of a three-clip row the mix axis sits between: `a` is the one it is heading towards.
fn pair(lib: &Library, first: i32, mix: F) -> (Option<ClipId>, Option<ClipId>) {
    if mix < F(0.5) {
        (lib.clip(first + 1), lib.clip(first))
    } else {
        (lib.clip(first + 2), lib.clip(first + 1))
    }
}

impl Driver {
    /// One frame of the vehicle's animation step.
    ///
    /// `self_placed` is the pair of flags the actor sets when it is placing itself: both set and a
    /// request that is not holding, and the placement is skipped. `length` answers how long a clip is,
    /// which only the stunt cross-fade asks — it builds its two phases out of the two clips' durations
    /// and nothing else in the machine reads one.
    pub fn frame(
        &mut self,
        dt: F,
        p: &Params,
        r: &Request,
        lib: &Library,
        self_placed: bool,
        length: impl Fn(ClipId) -> F,
    ) -> Frame {
        // 1 — the placement. The two flags only matter to a request that is not holding the bike.
        let place = (r.held || !self_placed).then(|| {
            let position = if r.at_base {
                r.base
            } else {
                // Ten VU0 operations, the chassis' own quaternion rotate, and then an `xyz` add: the
                // `w` the node is placed with is the offset's.
                let turned = turn(r.rotation, r.offset);
                turned.merge(Dest::XYZ, turned.add(r.base))
            };
            Place { position, rotation: r.rotation }
        });

        // 2 — the restart test. The second half of it is what makes a repeat of the state that is
        // already running start over: both ids are equal, but the request's count has moved.
        let restart = self.want != self.state || (r.seq >= 2 && r.seq != i32::from(self.seq));
        // 3 — the fade rate, by the state that was asked for. Out of range takes the default and no
        // arm of the switch runs at all.
        if restart {
            self.rate = match self.want {
                0..=23 | NO_HANDER => p.rate_stunt,
                REVERSE_L | REVERSE_R => {
                    // The only arm of the rate switch that does anything besides pick a rate.
                    self.reverse = F::ZERO;
                    p.rate_default
                }
                AIR | BACKFLIP => p.rate_pose,
                HOLD..=45 => p.rate_hold,
                GET_READY..=56 => {
                    if r.cut {
                        F::ZERO
                    } else {
                        p.rate_ready
                    }
                }
                START_POS..=59 => p.rate_ready,
                _ => p.rate_default,
            };
        }

        // 4 — the restart itself, the held request's override of it, and the latch.
        let (mut w1, mut w2, mut w3) = (F::ZERO, F::ZERO, F::ZERO);
        if restart {
            self.prev = self.state;
            self.started = true;
            self.ramp = F::ONE;
        }
        if r.held {
            self.rate = F::ZERO;
            self.ramp = F::ZERO;
        }
        self.state = self.want;
        self.ramp = self.ramp - self.rate * dt;
        self.from_table = false;
        // The backflip is the one state a run-out ramp does not let go of the held pose on.
        if self.holding && self.state != BACKFLIP && self.ramp <= F::ZERO {
            self.holding = false;
        }

        // 5 — the two axes. A held request snaps each one whose target is not the "leave it" value,
        // and then both are eased towards their targets anyway, which is what makes the snap stick.
        if r.held {
            if r.mix_axis != F(f32::MAX) {
                self.mix.value = r.mix_axis;
            }
            if r.phase_axis != F(f32::MAX) {
                self.phase.value = r.phase_axis;
            }
        }
        let mix = self.mix.step(dt, r.mix_axis);
        let phase = self.phase.step(dt, r.phase_axis);

        // 6 — the selection. An added record takes the machine's place entirely.
        let (mut a, mut b, mut c, mut d) = (None, None, None, None);
        let mut mode = 0u8;
        let mut play = None;
        if self.extra >= 0 {
            let rec = lib.extra(self.extra);
            match (rec.a, rec.b) {
                (Some(_), Some(_)) => {
                    (a, b, w1, w2, mode) = (rec.a, rec.b, phase, mix, 2);
                }
                (Some(_), None) => (a, w1, mode) = (rec.a, phase, 1),
                // The one asymmetry: with only the far clip, it is the *mix* axis that plays it, not
                // the phase axis the near clip would have been played at.
                (None, Some(_)) => (a, w1, mode) = (rec.b, mix, 1),
                (None, None) => {}
            }
        } else {
            match self.state {
                // The gesture slots: cross out of the gesture that was running, each clip at its own
                // phase, or play the new one alone when there was no gesture to come out of.
                0..=23 => {
                    self.from_table = true;
                    a = lib.clip(self.state);
                    if (0..SLOTS as i32).contains(&self.prev) {
                        b = lib.clip(self.prev);
                        if restart {
                            self.cross = F::ZERO;
                            self.cross_prev = p.start_phase;
                            self.cross_start = p.start_phase;
                        } else {
                            let dur_a = a.map_or(F::ZERO, &length);
                            let dur_b = b.map_or(F::ZERO, &length);
                            // The cross-fade's own weight: the seconds into the new clip over the
                            // shorter of the configured cap and half the clip.
                            let into = dur_a * r.progress;
                            w2 = into / p.cross_cap.min(dur_a / F(2.0));
                            self.cross = into / dur_a;
                            self.cross_prev = self.cross_start + (dur_a * r.progress) / dur_b;
                        }
                        self.ramp = F::ZERO;
                        self.cross = clamp01(self.cross);
                        self.cross_prev = clamp01(self.cross_prev);
                        mode = 5;
                    } else {
                        w1 = r.progress;
                        mode = 1;
                    }
                }
                // The pose grid: four bands of the band value, the two middle ones interpolating
                // between a pair of rows and the two outer ones playing one row.
                TURN_RIDING => {
                    w1 = phase;
                    w2 = half(mix);
                    let x = r.band;
                    if x < p.band[0] {
                        (a, b) = pair(lib, TURN_STOP, mix);
                        (c, d) = pair(lib, TURN_STILL, mix);
                        w3 = F::ONE;
                        if p.band[0] != F::ZERO {
                            w3 = remap(x, F::ZERO, p.band[0], F::ONE, F::ZERO);
                        }
                        mode = 4;
                    } else if x <= p.band[1] {
                        (a, b) = pair(lib, TURN_STILL, mix);
                        mode = 2;
                    } else if x > p.band[2] {
                        (a, b) = pair(lib, TURN_RIDING, mix);
                        mode = 2;
                    } else {
                        (a, b) = pair(lib, TURN_STILL, mix);
                        (c, d) = pair(lib, TURN_RIDING, mix);
                        w3 = F::ONE;
                        if p.band[1] != p.band[2] {
                            w3 = remap(x, p.band[1], p.band[2], F::ONE, F::ZERO);
                        }
                        mode = 4;
                    }
                }
                // The reverse pair, played at its own clock: forwards zeroes it, backwards winds it on
                // at three hundredths of the band value per second and wraps it at one.
                REVERSE_L | REVERSE_R => {
                    if r.band > F::ZERO {
                        self.reverse = F::ZERO;
                    } else {
                        let step = dt * (F(0.03) * r.band);
                        self.reverse = F(libm::fmodf((self.reverse - step).0, 1.0));
                    }
                    (a, b) = (lib.clip(REVERSE_R), lib.clip(REVERSE_L));
                    (w1, w2, mode) = (self.reverse, phase, 2);
                }
                // The airborne row, which is the grid's pick on one row and nothing else.
                AIR => {
                    (a, b) = pair(lib, AIR, mix);
                    (w1, w2, mode) = (phase, half(mix), 2);
                }
                // The backflip, held at a tenth of its clip.
                BACKFLIP => {
                    a = lib.clip(self.want);
                    (w1, mode) = (F(0.1), 1);
                }
                NO_HANDER => {
                    self.from_table = true;
                    a = lib.clip(self.want);
                    (w1, mode) = (phase, 1);
                }
                // The two played-once groups: a cross into the clip on the frame the state starts, and
                // from then on the clip runs on its own.
                HOLD..=59 => {
                    if restart {
                        let clip = lib.clip(self.state);
                        play = Some(Play {
                            clip,
                            length: self.rate,
                            duration: clip.map_or(F::ZERO, &length),
                        });
                    }
                }
                // Every other state, and every state outside the table: nothing is driven.
                _ => {}
            }
        }

        // 7 — the clamps and the call.
        self.ramp = clamp01(self.ramp);
        let (ramp, w1, w2, w3) = (self.ramp, clamp01(w1), clamp01(w2), clamp01(w3));
        let blend = match mode {
            1 => Blend::One { phase: w1, ramp, clip: a },
            2 => Blend::Two { phase: w1, mix: w2, ramp, a, b },
            4 => Blend::Four { phase: w1, mix: w2, cross: w3, ramp, a, b, c, d },
            5 => Blend::Pair {
                phase_a: self.cross,
                phase_b: self.cross_prev,
                mix: w2,
                ramp,
                a,
                b,
            },
            _ => Blend::Free,
        };
        Frame { place, restart, play, blend }
    }
}

// ---------------------------------------------------------------------------------------------------
// which state
// ---------------------------------------------------------------------------------------------------

/// The state the chooser reaches for outside the body it belongs to: the game's generator, and the
/// start poses already handed out.
///
/// Both are globals in the original — the generator is the C library's one word, the list is a file
/// static with its own count — and the list being shared is the point of it: two bodies on the same
/// start line draw from one list, so they cannot be given the same pose. One of these per race, not
/// one per body.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shared {
    /// The generator every draw in the chooser comes out of.
    pub rng: Rng,
    /// The start poses handed out so far. The tenth draw fills it; the next one throws it away and
    /// starts again with the one new pose.
    pub used: Vec<i32>,
}

/// The body's own update: the writer of [`Driver::want`], which is what the machine was missing.
///
/// [`Driver::frame`] is handed a state and answers with a call into the motion library. This is the
/// half that picks the state — and in the original it is the *caller*: one function that chooses,
/// calls the step, and then files two fields away for the next frame. The port is that function in
/// two halves with the step between them, [`Update::want`] and [`Update::serviced`], because nothing
/// else can be done with a function that calls what it is handed.
///
/// Without this, nothing in the port ever asks for the pose grid, and the pose grid is the only
/// caller of the four-clip blend in the image.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Update {
    /// How long until the next standing-still fidget. Counted down on every frame the body is
    /// standing still with nothing commanded, and re-armed to a draw between [`Update::wait`] when it
    /// is not running at all.
    pub idle_wait: F,
    /// A fidget is playing, which is what stops the chooser asking for the pose grid again until the
    /// clip has run out.
    pub idle_playing: bool,
    /// A start-line pose has been drawn for this body.
    pub start_picked: bool,
    /// How long the start-line breathe has left before another pose is drawn.
    pub breathe_wait: F,
    /// How slow counts as standing still. The body's constructor sets a fifth of the pose grid's
    /// first threshold.
    pub still_band: F,
    /// The two bounds the fidget's wait is drawn between, low then high: fifteen and forty-five
    /// seconds in the body's constructor.
    pub wait: [F; 2],
    /// The wrecked body has stopped moving. Written by another method of the same object, which is
    /// not ported: false is the airborne flail, true is one of the five hold poses.
    pub settled: bool,
}

impl Update {
    /// The state to ask for this frame: everything the original's update does *before* it calls the
    /// step.
    ///
    /// Writes [`Driver::want`], the chooser's own four live fields, and — on the one frame the start
    /// gate opens — the request's start-line flag. [`Player::finished`] is read and cleared here
    /// too: a played-once pose running out is how the chooser knows to choose again.
    pub fn want(
        &mut self,
        dt: F,
        d: &mut Driver,
        p: &Params,
        a: &mut Asked,
        player: &mut Player,
        shared: &mut Shared,
    ) {
        if !a.request.cut {
            // Riding, in the original's order of precedence.
            if a.wrecked {
                if !self.settled {
                    d.want = HOLD;
                } else if d.want < HOLD + 1 {
                    // Drawn once: the test is on the state asked for, so the five hold poses do not
                    // get redrawn every frame the body lies there.
                    d.want = HOLD + 1 + shared.rng.index(5);
                }
            } else if a.gesture != NO_GESTURE {
                d.want = if a.no_hands { NO_HANDER } else { a.gesture };
            } else if a.flipping {
                d.want = BACKFLIP;
            } else if a.airborne {
                d.want = AIR;
            } else if a.reversing == F::ZERO || a.request.phase_axis < F(0.5) {
                // The reverse pair, as the original tests it: the flag and the axis against a half,
                // twice and in both directions, which is also what decides a NaN axis.
                if a.reversing == F::ZERO || a.request.phase_axis >= F(0.5) {
                    self.standing(dt, d, a, player, shared);
                } else {
                    d.want = REVERSE_L;
                }
            } else {
                d.want = REVERSE_R;
            }
        } else if self.breathe_wait > F::ZERO {
            // On the gate: breathe while the timer runs.
            d.want = GET_READY;
            self.breathe_wait = self.breathe_wait - dt;
        } else if !self.start_picked {
            d.want = Update::draw_start(shared);
            self.start_picked = true;
        } else if !player.finished {
            if d.want == GET_READY {
                d.want = START_POS + shared.rng.index(START_POSES);
            } else if a.commanded != F::ZERO {
                // The gate opens. The chooser clearing this flag is the only writer of it I found,
                // and it is what hands the body over to the pose grid.
                a.request.cut = false;
                d.want = TURN_RIDING;
            }
        } else {
            d.want = GET_READY;
            self.breathe_wait = (F(3.5) - a.settle) - p.rate_ready;
        }
        // Last, and in both branches: moving disarms the fidget. The threshold is the pose grid's
        // own first one, so a body that is moving at all cannot be waiting to fidget.
        if p.band[0] < a.request.band {
            self.idle_wait = F::ZERO;
            self.idle_playing = false;
        }
    }

    /// Standing still, which is the arm that ends in the pose grid: the fidget's countdown, and then
    /// state [`TURN_RIDING`] unless a fidget is playing.
    ///
    /// The countdown only runs while the body is *actually* standing still — slower than
    /// [`Update::still_band`], both axes at exactly a half, nothing commanded — and the draw that
    /// re-arms it is made on the frame it is found not to be running, not on the frame it runs out.
    fn standing(
        &mut self,
        dt: F,
        d: &mut Driver,
        a: &Asked,
        player: &mut Player,
        shared: &mut Shared,
    ) {
        if a.request.band.abs() < self.still_band
            && a.request.phase_axis == F(0.5)
            && a.request.mix_axis == F(0.5)
            && a.commanded == F::ZERO
        {
            if self.idle_wait > F::ZERO {
                // The step is taken and stored before it is tested, so the frame it goes negative is
                // the frame the fidget starts.
                self.idle_wait = self.idle_wait - dt;
                if self.idle_wait < F::ZERO {
                    d.want = IDLE + shared.rng.index(IDLES);
                    self.idle_playing = true;
                    // A fidget is played once, and the finished flag is what the chooser watches for
                    // the end of it, so it starts the wait cleared.
                    player.finished = false;
                }
            } else {
                self.idle_wait =
                    remap(shared.rng.unit(), F::ZERO, F::ONE, self.wait[0], self.wait[1]);
            }
        }
        if !self.idle_playing {
            d.want = TURN_RIDING;
        } else if player.finished {
            self.idle_playing = false;
        }
    }

    /// A start-line pose no other body on this line has taken.
    ///
    /// The list is redrawn against until the draw is not in it; the tenth draw fills it, and the
    /// next call throws the whole list away and starts again with its own one pose. An empty list is
    /// the same first case, which is how the first body on the line is served.
    fn draw_start(shared: &mut Shared) -> i32 {
        if shared.used.is_empty() || shared.used.len() == IDLES as usize {
            let pick = IDLE + shared.rng.index(IDLES);
            shared.used.clear();
            shared.used.push(pick);
            return pick;
        }
        loop {
            let pick = IDLE + shared.rng.index(IDLES);
            if !shared.used.contains(&pick) {
                shared.used.push(pick);
                return pick;
            }
        }
    }

    /// The two fields the original's update writes *after* the step has run: the phase the stunt
    /// cross-fade is built from, and the request count the restart test compares against.
    ///
    /// Both are last frame's by construction — the step reads them before this writes them — which is
    /// where the one frame of lag in both of them comes from, and it is what makes the restart test
    /// fire on exactly the frame the request's count changes rather than on every frame after it.
    pub fn serviced(d: &mut Driver, p: &mut Params, a: &Asked) {
        p.start_phase = a.request.progress;
        // The original stores a byte, so a count of 256 compares equal to one of 0.
        d.seq = a.request.seq as u8;
    }
}

impl hash::State for Lag {
    fn walk(&self, w: &mut hash::Walk<'_>) {
        w.f("value", self.value);
        w.f("tau", self.tau);
        w.f("ratio", self.ratio);
    }
}

impl hash::State for Driver {
    fn walk(&self, w: &mut hash::Walk<'_>) {
        w.i32("want", self.want);
        w.i32("state", self.state);
        w.i32("prev", self.prev);
        w.u8("seq", self.seq);
        w.f("rate", self.rate);
        w.f("ramp", self.ramp);
        w.bool("started", self.started);
        w.bool("holding", self.holding);
        w.bool("from_table", self.from_table);
        w.f("cross", self.cross);
        w.f("cross_prev", self.cross_prev);
        w.f("cross_start", self.cross_start);
        w.f("reverse", self.reverse);
        w.i32("extra", self.extra);
        w.field("mix", &self.mix);
        w.field("phase", &self.phase);
    }
}
