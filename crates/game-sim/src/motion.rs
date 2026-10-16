//! Animation driving: which motion plays for the state the rider is in, how far into it the frame is,
//! and how one motion blends into the next.
//!
//! This is the half of the rider's per-frame update that [`crate::trick`] stops short of. The original
//! runs both in one function: the clock section first (what a trick's elapsed and progress are this
//! frame), then — past the join the trick task's proofs end their calls at — the animation section,
//! which is this module. It ends in VU0 skinning, which is the renderer's; everything in front of the
//! skinning is here, because its timing is not cosmetic:
//!
//! - the **phase** a stunt's clip is played at is the trick's `progress`, so a clock that is one frame
//!   out plays the clip one frame out, and every action frame in it fires one frame out with it
//!   ([`Clip::fired`]);
//! - the pose the airborne blend lands on is written straight into the field the landing test judges
//!   ([`Driver::shown_crossup`], which is `air::Loads::landing_crossup`), so the animation decides
//!   whether a jump is survivable;
//! - the cross-fade [`Driver::ramp`] is what makes a restarted motion start from the pose the rider
//!   was already in, and it is a fixed number of frames long ([`Params::blend_rate`]).
//!
//! ## The four parts
//!
//! 1. **The frame's header** — [`Driver::frame`] decays the cross-fade ramp, decides whether the
//!    motion starts over, and latches the requested action id into the one the selection runs on.
//! 2. **The selection** — a switch over that id, one arm per kind of motion, which answers with a
//!    [`Blend`]: which clips, at which phase, mixed how, and faded in by how much. This is the "which
//!    motion plays per state" of the task, and the states are the action ids
//!    ([`crate::trick::Trick`]): the 24 gesture slots each play their own clip, the two idles blend
//!    the three airborne poses, and the rotating moves are left to run on their own.
//! 3. **The call** — [`Player::run`], the dispatch the frame ends in: one of four entry points by
//!    [`Blend::mode`], each driving the motion object's clock from the *first* clip it was handed
//!    ([`Blend::driven`]). Which handle a [`Slot`] means is [`Library`]'s, and that table is filled by
//!    name ([`Library::load`]).
//! 4. **The playback** — [`Player`], the motion object's own clock: the dt advance with its loop and
//!    its end, the phase-driven sample the blends use instead, the transition weight, and the window
//!    of clip time each frame fires its action frames from. [`Player::play`] is what puts a clip on,
//!    and the only thing anywhere that starts a transition.
//!
//! ## What drives what
//!
//! ```text
//!   action id        what plays                                     driven by
//!   0..=23           the slot's own clip, "bike_<stunt name>"        the trick's progress
//!   24  backflip     whatever was started, free-running             dt
//!   25  wheelie      bike_air_Fwd / _Mid / _Bck, two at a time      the rider's fore-aft and crossup
//!   26  stoppie      whatever was started, free-running             dt
//!   27               whatever was started, free-running             dt
//!   28  none         bike_air_Mid at its midpoint                   nothing — a fixed pose
//!   29..             whatever was started, free-running             dt
//! ```
//!
//! The names are the strings the rider's loader looks the clips up by — `"<prefix>bike_<Name>"` from
//! the stunt's own `Name` in `stunts.ini`, and the three airborne poses by their literal names — and a
//! gesture slot the rider has no clip *handle* for falls back on the same fixed pose as id 28. Two
//! things make a slot handle-less: the animation set has no clip of that name, or the slot's record is
//! not marked `UsedForBike`, in which case the loader never binds the clip even when it exists
//! ([`Library::load`]).
//!
//! A fifth kind of call exists in the library and no arm of the rider's switch asks for it: the
//! four-clip, three-weight [`Blend::Three`]. The vehicle's own animation step asks for it, from two of
//! its arms, so the variant is modelled; nothing in the rider's path reaches it.
//!
//! ## The frame
//!
//! The original's order inside one function, which [`Driver::frame`] keeps:
//!
//! 1. the ramp decays by [`Params::blend_rate`] × `dt`, clamped to `0..=1`;
//! 2. the restart test ([`Driver::frame`]'s `restart`), which is the same predicate the trick clocks
//!    use to start a trick over — one test, two users;
//! 3. the requested id is latched into [`Driver::action`];
//! 4. on a restart: the rider's current pose is snapshotted so the ramp has something to fade from,
//!    and the ramp is put back to 1;
//! 5. [`Driver::fore_aft`] is taken from the rider's fore-aft lean;
//! 6. the selection, which may also write [`Driver::pose`] and [`Driver::shown_crossup`];
//! 7. the weights are clamped to `0..=1` and the motion library is called.
//!
//! The pose fields are *not* written on every arm of the switch, and the original leaves them alone
//! rather than clearing them — so a rider in the middle of a stunt shows the crossup the last airborne
//! idle frame left behind. That is why they are state here and not outputs.
//!
//! `research/journal/2026-10-15-s6-s6-motion/` and `research/journal/2026-10-16-s20-s20-animation-caller/`
//! have the addresses, the clip names and the proofs.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::hash;
use crate::trick::{SLOTS, Stunt, Trick};
use ps2_float::F;

// ---------------------------------------------------------------------------------------------------
// the clip
// ---------------------------------------------------------------------------------------------------

/// One animation clip, as far as the driving is concerned: how long it is, whether it loops, and the
/// times inside it that fire something.
///
/// The original's clip holds a lot more — the channels, their key streams, the joint map — and all of
/// that is the renderer's. What the timing needs is these three.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clip {
    /// The clip's length in seconds. Also the denominator of nothing: a phase-driven blend multiplies
    /// by it ([`Player::drive`]), the dt advance compares against it.
    pub duration: F,
    /// The clip's own flag: at the end, loop round rather than stop.
    pub looping: bool,
    /// The action frames: times in seconds from the start of the clip, sorted, each with its own list
    /// of things to call. The port keeps the times — what hangs off them is the event system's.
    pub keys: Vec<F>,
}

impl Clip {
    /// A clip with no action frames.
    pub fn new(duration: F, looping: bool) -> Clip {
        Clip { duration, looping, keys: Vec::new() }
    }

    /// Whether a clip time is past the end. The original's test is strict — a time exactly equal to
    /// the duration is still inside the clip, which is what lets the last action frame fire.
    pub fn over(&self, t: F) -> bool {
        self.duration < t
    }

    /// The action frames a frame fires, in the order the original collects them.
    ///
    /// The test is inclusive at both ends, and one frame's end is the next frame's start — so a key
    /// that lands exactly on a sample time fires on *both* of those frames. The original has the same
    /// double, and it is the reason this is a list of indices rather than a count.
    ///
    /// It is not quite "every key in the window", though. The original brackets the scan with two
    /// binary searches before it tests anything, and the low end of that bracket is one place *behind*
    /// the first key past the start of the window — which, when two keys sit on the same clip time,
    /// leaves the earlier one outside the bracket. So a pair of action frames an animator put on the
    /// same frame of a clip fires only the later one. Kept, because it decides what happens.
    pub fn fired(&self, window: Window) -> Vec<usize> {
        let mut out = Vec::new();
        for (lo, hi) in window.spans() {
            let bracket = self.after(lo).saturating_sub(1)..self.after(hi);
            for i in bracket {
                let k = self.keys[i];
                if lo <= k && k <= hi {
                    out.push(i);
                }
            }
        }
        out
    }

    /// The first action frame strictly after a clip time, which is what both of the scan's binary
    /// searches land on: the one above the window's end is where the bracket stops, and the one above
    /// its start, less one, is where the bracket begins.
    fn after(&self, t: F) -> usize {
        self.keys.partition_point(|&k| !(t < k))
    }
}

/// The span of clip time a frame's action frames are taken from.
///
/// Three shapes, because the original makes one, two or no calls into the event collector: a plain
/// span from the last sample to this one, a split span when the clip wrapped round the end between
/// them, and nothing at all when a clip has only just been put on at time zero.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Window {
    /// No action frames this frame.
    #[default]
    None,
    /// From the previous sample to this one.
    Span(F, F),
    /// The clip ended between the two samples: the tail of the clip first, then its head.
    Wrapped { tail: (F, F), head: (F, F) },
}

impl Window {
    /// The window's spans, in the order the original collects them.
    pub fn spans(self) -> Vec<(F, F)> {
        match self {
            Window::None => Vec::new(),
            Window::Span(a, b) => vec![(a, b)],
            Window::Wrapped { tail, head } => vec![tail, head],
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// which clip
// ---------------------------------------------------------------------------------------------------

/// A clip the rider has, by the slot the loader put it in.
///
/// The rider's loader fills these from its animation set by name: one `bike_<stunt name>` per gesture
/// slot, and the three airborne poses `bike_air_Fwd`, `bike_air_Mid` and `bike_air_Bck`. A slot whose
/// name is not in the set is left empty, which the selection notices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// One of the 24 gesture slots: `bike_<stunt name>`.
    Stunt(u8),
    /// `bike_air_Fwd` — the rider forward on the bike.
    AirFwd,
    /// `bike_air_Mid` — the neutral pose, and the one a grounded rider holds.
    AirMid,
    /// `bike_air_Bck` — the rider back over the rear wheel.
    AirBck,
}

/// Which gesture slots the rider's animation set actually has a clip for.
///
/// The three airborne poses are not here: nothing in the original checks them, and a set without them
/// would not get as far as the selection.
#[derive(Clone, Copy, Debug)]
pub struct Clips {
    /// Per gesture slot, whether `bike_<stunt name>` was found.
    pub stunt: [bool; SLOTS],
}

impl Default for Clips {
    fn default() -> Clips {
        Clips { stunt: [true; SLOTS] }
    }
}

/// What the rider's clip table needs of the layer that owns the animation set and the disc.
///
/// The original's loader reaches into the engine's animation library for both of these, and neither is
/// the simulation's: one reads the disc, the other walks a list of loaded clips. The port asks for them
/// so that [`Library::load`] can be the part that *is* the simulation's — which name each state's clip
/// is asked for by, which states ask at all, and which field the answer lands in.
pub trait Animations {
    /// Find the clip of this name in the set, load it if the set has it and it is not loaded, and bind
    /// it to the rider's motion object.
    ///
    /// Called once per gesture slot whose stunt record is marked for the bike, and **never** for one
    /// that is not — which is the whole of the `UsedForBike` flag's effect on the animation: an
    /// unmarked slot is never bound, so [`Animations::bound`] has no handle for it and the slot plays
    /// the fallback pose instead. The three airborne clips are never loaded here; they are expected to
    /// be bound already.
    fn load(&mut self, name: &str);

    /// The handle the rider's motion object has for the clip of this name, if it has one.
    ///
    /// `None` covers both of the original's ways of answering nothing — the set has no clip of that
    /// name, and it has one but this motion object never bound it — because the original stores the
    /// same null for both.
    fn bound(&self, name: &str) -> Option<ClipId>;
}

/// The clip the rider plays per state: the handles the selection's [`Slot`] names.
///
/// Four fields and not one array of 28, even though the original's are four consecutive words and the
/// three airborne ones sit exactly where ids 25, 26 and 27 would: the loader writes them as named
/// fields, and the ids whose arms would index them never index anything. The id-24 word between the
/// two groups is never written at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Library {
    /// Per gesture slot, the handle for `"<prefix>bike_<stunt name>"`.
    pub stunt: [Option<ClipId>; SLOTS],
    /// `bike_air_Fwd`.
    pub air_fwd: Option<ClipId>,
    /// `bike_air_Mid` — the neutral pose, the grounded idle, and the fallback for a gesture slot with
    /// no clip of its own.
    pub air_mid: Option<ClipId>,
    /// `bike_air_Bck`.
    pub air_bck: Option<ClipId>,
}

impl Library {
    /// The name a gesture slot's clip is looked up by: the rider's own prefix, then `bike_`, then the
    /// stunt's `Name` from `stunts.ini`. The original builds it with one format string per slot, and
    /// into a buffer of 32 bytes — so a long enough stunt name is truncated, which the port keeps.
    pub fn stunt_name(prefix: &str, stunt: &str) -> String {
        let whole = format!("{prefix}bike_{stunt}");
        String::from_utf8_lossy(&whole.as_bytes()[..whole.len().min(31)]).into_owned()
    }

    /// The rider's clip table, as its loader fills it.
    ///
    /// Two passes over the 24 gesture slots, in order: the first loads and binds the clip of every slot
    /// marked for the bike, the second asks for the handle of every slot's clip whether it was marked
    /// or not. Then the three airborne poses, which are only asked for — nothing here loads them.
    pub fn load(prefix: &str, stunts: &[Stunt], anim: &mut impl Animations) -> Library {
        for stunt in stunts.iter().take(SLOTS) {
            if stunt.for_bike {
                anim.load(&Library::stunt_name(prefix, &stunt.name));
            }
        }
        let mut lib = Library::default();
        for (slot, stunt) in stunts.iter().take(SLOTS).enumerate() {
            lib.stunt[slot] = anim.bound(&Library::stunt_name(prefix, &stunt.name));
        }
        lib.air_fwd = anim.bound("bike_air_Fwd");
        lib.air_mid = anim.bound("bike_air_Mid");
        lib.air_bck = anim.bound("bike_air_Bck");
        lib
    }

    /// The handle behind one of the selection's clip slots.
    pub fn clip(&self, slot: Slot) -> Option<ClipId> {
        match slot {
            Slot::Stunt(i) => *self.stunt.get(usize::from(i)).unwrap_or(&None),
            Slot::AirFwd => self.air_fwd,
            Slot::AirMid => self.air_mid,
            Slot::AirBck => self.air_bck,
        }
    }

    /// Which gesture slots have a clip, which is the one thing the selection asks of the table.
    pub fn present(&self) -> Clips {
        Clips { stunt: self.stunt.map(|c| c.is_some()) }
    }
}

// ---------------------------------------------------------------------------------------------------
// what the library is handed
// ---------------------------------------------------------------------------------------------------

/// What the selection asks the motion library for this frame: one of its entry points, with its
/// arguments.
///
/// The weights are already clamped to `0..=1` here, which is the last thing the original does before
/// the call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Blend {
    /// Nothing is driven: whatever clip is playing advances by `dt` on its own ([`Player::advance`]).
    /// The rotating moves and every id past the switch end up here.
    Free,
    /// One clip, sampled at `phase` of its length, faded in from the snapshotted pose by `ramp`.
    One {
        /// Where in the clip, 0 to 1 of its duration.
        phase: F,
        /// How much of the *previous* pose is still showing: 1 the frame a motion starts, 0 once the
        /// cross-fade is over.
        ramp: F,
        /// The clip.
        clip: Slot,
    },
    /// Two clips, both sampled at `phase` of the *first* one's length, mixed, and faded in by `ramp`.
    Two {
        /// Where in the clips, 0 to 1 of `a`'s duration — `b` is sampled at the same seconds, not at
        /// the same fraction of its own length.
        phase: F,
        /// How far from `b` to `a`: 0 is all `b`, 1 is all `a`.
        mix: F,
        /// The cross-fade from the snapshotted pose, as in [`Blend::One`].
        ramp: F,
        /// The clip `mix` interpolates towards.
        a: Slot,
        /// The clip `mix` interpolates from.
        b: Slot,
    },
    /// Four clips and three weights: two pairs, each mixed by `mix`, and the two results mixed by
    /// `cross` — all of it sampled at `phase` of the *first* clip's length and faded in by `ramp`.
    ///
    /// No arm of the rider's own switch asks for this one. The vehicle's animation step does, from two
    /// of its own arms, which is why the variant is here: the entry point is the fourth case of the
    /// same dispatch, and a port of that step has nothing to call without it. See the journal.
    Three {
        /// Where in the clips, 0 to 1 of `a`'s duration. Every one of the four is sampled at the same
        /// number of seconds, and the action frames come from `a`.
        phase: F,
        /// How far from `b` to `a`, and from `d` to `c`: one weight, used inside both pairs.
        mix: F,
        /// How far from the `c`/`d` pair to the `a`/`b` pair.
        cross: F,
        /// The cross-fade from the snapshotted pose, as in [`Blend::One`].
        ramp: F,
        /// The first pair's near clip.
        a: Slot,
        /// The first pair's far clip.
        b: Slot,
        /// The second pair's near clip.
        c: Slot,
        /// The second pair's far clip.
        d: Slot,
    },
}

impl Blend {
    /// Which of the library's entry points this is, by the number the original's dispatch switches
    /// on: 0 the free-running advance, 1 one clip, 2 two, 4 four clips and three weights.
    ///
    /// **3 is not a mode.** The dispatch is a chain of equality tests with no arm for it, so a 3
    /// falls out of the bottom and the frame makes no call at all — as does anything above 4 in the
    /// rider's copy of the dispatch, and anything above 5 in the vehicle's, which has one more entry.
    pub fn mode(self) -> u8 {
        match self {
            Blend::Free => 0,
            Blend::One { .. } => 1,
            Blend::Two { .. } => 2,
            Blend::Three { .. } => 4,
        }
    }

    /// The clip the entry point drives the motion object's clock from, and the phase it drives it at.
    ///
    /// Every blend entry takes its clip time from its **first** clip — `phase` of *that* one's
    /// duration — and fires *that* one's action frames, however many clips it goes on to mix in. The
    /// free-running advance drives whatever is already playing, so it names no clip.
    pub fn driven(self) -> Option<(F, Slot)> {
        match self {
            Blend::Free => None,
            Blend::One { phase, clip, .. } => Some((phase, clip)),
            Blend::Two { phase, a, .. } => Some((phase, a)),
            Blend::Three { phase, a, .. } => Some((phase, a)),
        }
    }

    /// Whether a clip the entry point has only just been handed fires the empty window `0..0` when its
    /// clip time comes out as exactly zero.
    ///
    /// The one place the three blend entries differ. The one-clip entry tests its sample time against
    /// zero and skips the action frames entirely; the two-clip and four-clip entries do not test it,
    /// so a clip switched to at a phase of zero fires whatever key sits on its first frame. The
    /// free-running advance never takes this path at all.
    pub fn fires_empty(self) -> bool {
        matches!(self, Blend::Two { .. } | Blend::Three { .. })
    }

    /// Every clip the entry point samples, in the order it takes them as arguments.
    pub fn clips(self) -> Vec<Slot> {
        match self {
            Blend::Free => Vec::new(),
            Blend::One { clip, .. } => vec![clip],
            Blend::Two { a, b, .. } => vec![a, b],
            Blend::Three { a, b, c, d, .. } => vec![a, b, c, d],
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// the selection
// ---------------------------------------------------------------------------------------------------

/// The rider's own two numbers that the driving reads.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// How fast the cross-fade ramp falls, per second. The rider's constructor sets 3, so a motion
    /// fades in over a third of a second — ten frames at thirty.
    pub blend_rate: F,
    /// `MaxCrossupAngle` from the rider's own parameter file, in radians: the crossup at which the
    /// airborne pose reaches the end of its sweep. The file gives it in degrees and the loader
    /// multiplies by `pi/180`.
    pub max_crossup: F,
}

impl Default for Params {
    fn default() -> Params {
        Params { blend_rate: F(3.0), max_crossup: F::ZERO }
    }
}

/// Where the rider is this frame, as the selection reads it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pose {
    /// The rider's lean, −1 to 1 (`air::Step::lean`). Only [`Driver::pose`] is taken from it.
    pub lean: F,
    /// The rider's fore-aft position, −1 forward to 1 back: which two of the three airborne clips are
    /// blended, and how far between them.
    pub fore_aft: F,
    /// The crossup angle in radians (`air::Air::crossup`), which the airborne blend turns into the
    /// pose the landing is judged on.
    pub crossup: F,
}

/// What the trick machine asks for this frame, as the animation header reads it.
///
/// Four of the five come from [`crate::trick::Tricks`] — the id it latched, its restart flag, and the
/// request count with the copy from the end of the previous frame. `frozen` is the rider's own "change
/// nothing" state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Request {
    /// The id the rider latched to run this frame ([`crate::trick::Tricks::requested`]).
    pub action: Trick,
    /// The trick machine's "this was asked for again" flag, which forces the motion to start over.
    pub force: bool,
    /// How many tricks have been asked for in this flight. The original keeps the copy the animation
    /// compares as a byte, so it wraps at 256 where [`crate::trick::Tricks::seq`] does not.
    pub seq: u8,
    /// `seq` as it was at the end of the previous frame.
    pub prev_seq: i32,
    /// The rider is frozen: no restart, and no cross-fade either.
    pub frozen: bool,
    /// How far through the running trick, `elapsed / duration`
    /// ([`crate::trick::Tricks::progress`]) — the phase a stunt's clip plays at.
    pub progress: F,
}

/// The animation driving's own state: the id it is playing, the cross-fade, and the three values the
/// selection leaves behind for other things to read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Driver {
    /// The action id the selection runs on, latched from the request at the top of the frame. This is
    /// also what the landing test compares against the requested id to see whether a trick is still
    /// coming up.
    pub action: Trick,
    /// The cross-fade: how much of the pose the rider had when the motion started is still showing.
    /// 1 on the frame of a restart, down to 0 at [`Params::blend_rate`] per second.
    pub ramp: F,
    /// A cross-fade has been started at least once. The original sets it on every restart and never
    /// clears it.
    pub faded: bool,
    /// The rider's fore-aft position mapped to 0..1, which picks the pair of airborne clips and the
    /// mix between them. Written every frame, before the selection.
    pub fore_aft: F,
    /// The pose the rider is showing, 0..1 with 0.5 neutral. Only the two idle arms and the
    /// missing-clip fallback write it; the original latches it into the state a networked rider sends.
    pub pose: F,
    /// The crossup the airborne pose is actually showing, 0..1 with 0.5 straight ahead — written by
    /// the airborne arm only, and read by the landing test as `air::Loads::landing_crossup`. A rider
    /// in a stunt keeps whatever the last airborne idle frame put here.
    pub shown_crossup: F,
}

/// What one frame of driving did, beside the fields it wrote.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// What the motion library was asked for.
    pub blend: Blend,
    /// The motion started over this frame: the rider's current pose was snapshotted for the
    /// cross-fade to come from, and any clip playing free was restarted from its first frame.
    pub restart: bool,
}

impl Driver {
    /// One frame of animation driving: the header, then the selection.
    ///
    /// `dt` is the frame time the rest of the rider update uses. The caller hands the same
    /// [`Request`] to [`crate::trick::Tricks::update`], which is run first — the original computes
    /// the restart test once, at the top, and both halves read it.
    pub fn frame(&mut self, dt: F, p: Params, r: Request, pose: Pose, clips: Clips) -> Frame {
        // The ramp first, and it falls whether or not anything restarts: two roundings, a multiply
        // and a subtract, and then the clamp as the original's two branches rather than a min/max.
        let next = self.ramp - p.blend_rate * dt;
        self.ramp = if next > F::ONE {
            F::ONE
        } else if next < F::ZERO {
            F::ZERO
        } else {
            next
        };

        let mut restart = restarted(r, self.action);
        if r.frozen {
            // Frozen: nothing starts over, and the cross-fade is dropped rather than left to decay.
            self.ramp = F::ZERO;
            restart = false;
        }
        self.action = r.action;
        if restart {
            // The snapshot the ramp fades *from* is the caller's: the original asks the motion object
            // to copy the rider's current joint poses into its two blend buffers.
            self.faded = true;
            self.ramp = F::ONE;
        }
        // The fore-aft lean, mapped to 0..1. Written before the switch, so every arm sees it.
        self.fore_aft = (pose.fore_aft + F::ONE) * F(0.5);

        let blend = self.select(p, r, pose, clips);
        Frame { blend, restart }
    }

    /// The pose a held rider shows: straight on, and centred fore-aft.
    ///
    /// Run from the rider's idle step, not from [`Driver::frame`] — the original's idle writes these
    /// two fields itself, at the end, when the rider is held in place (the gate before a gate drops,
    /// and the pause before a reset puts it back). The next [`Driver::frame`] overwrites
    /// [`Driver::fore_aft`] from the rider's lean again; [`Driver::pose`] survives until an arm of
    /// the switch that writes it comes round, which is why it is worth writing at all.
    pub fn freeze(&mut self) {
        self.pose = F(0.5);
        self.fore_aft = F(0.5);
    }

    /// The switch: which motion this action id plays, and how.
    ///
    /// Kept separate from the header so that a caller which only wants to know what is playing — the
    /// renderer, a test — can ask without stepping the ramp.
    pub fn select(&mut self, p: Params, r: Request, pose: Pose, clips: Clips) -> Blend {
        match self.action {
            // The 24 gesture slots: the slot's own clip, played at the trick's progress. A slot the
            // animation set has no clip for falls back on the neutral pose, exactly as id 28 does.
            a if a.slot().is_some() => {
                let slot = a.slot().unwrap_or(0);
                if clips.stunt[slot] {
                    Blend::One {
                        phase: clamp01(r.progress),
                        ramp: self.ramp,
                        clip: Slot::Stunt(slot as u8),
                    }
                } else {
                    self.pose = F(0.5).madd(F(0.5), pose.lean);
                    Blend::One { phase: F(0.5), ramp: self.ramp, clip: Slot::AirMid }
                }
            }
            // The airborne idle: the crossup sweep of two of the three airborne clips, mixed by how
            // far forward or back the rider is. This is the arm that decides a landing.
            Trick::WHEELIE => {
                let shown = remap(pose.crossup, -p.max_crossup, p.max_crossup, F::ZERO, F::ONE);
                self.pose = shown;
                self.shown_crossup = shown;
                // Which pair, and where between them. The halves overlap on the middle clip, so the
                // mix is continuous across the neutral pose.
                let (a, b, mix) = if F(0.5) <= self.fore_aft {
                    (Slot::AirBck, Slot::AirMid, (self.fore_aft - F(0.5)) * F(2.0))
                } else {
                    (Slot::AirMid, Slot::AirFwd, self.fore_aft * F(2.0))
                };
                Blend::Two { phase: clamp01(shown), mix: clamp01(mix), ramp: self.ramp, a, b }
            }
            // The grounded idle: the middle airborne clip, at its exact midpoint. The lean goes into
            // the pose field for the packed state to pick up, but the clip is held still.
            Trick::NONE => {
                self.pose = F(0.5).madd(F(0.5), pose.lean);
                Blend::One { phase: F(0.5), ramp: self.ramp, clip: Slot::AirMid }
            }
            // The backflip, the stoppie, the id between them and everything past 28: the switch does
            // nothing, so whatever clip was started plays on at its own pace.
            _ => Blend::Free,
        }
    }
}

/// Whether the motion starts over this frame.
///
/// Three ways, and the third is the interesting one: the same trick asked for a second time has the
/// same id, so what tells a repeat from the trick still running is the request count moving. The count
/// has to have reached 2 before it counts — the first request of a flight is the one that set the id.
///
/// The trick clocks compute this same predicate for themselves (it is one test in the original, read
/// by both halves of the function), which is what [`crate::trick::Tricks::update`] transliterates.
pub fn restarted(r: Request, playing: Trick) -> bool {
    if r.frozen {
        return false;
    }
    r.action != playing || r.force || (r.seq >= 2 && i32::from(r.seq) != r.prev_seq)
}

/// `max.s` then `min.s`, which is how the original clamps every weight it hands the library.
fn clamp01(x: F) -> F {
    x.max(F::ZERO).min(F::ONE)
}

/// The engine's shared remap: `x` from one range to another, clamped to the output range, with the
/// degenerate input range answering with the low end.
///
/// The same three-instruction lerp the trick clocks' hold amplitude goes through, so it is spelled the
/// same way here: `adda.s` of the low end, then `madd.s` by the span.
fn remap(x: F, in_lo: F, in_hi: F, out_lo: F, out_hi: F) -> F {
    if in_lo == in_hi {
        return out_lo;
    }
    let t = (x - in_lo) / (in_hi - in_lo);
    let y = out_lo.lerp(out_hi, t);
    y.max(out_lo.min(out_hi)).min(out_lo.max(out_hi))
}

// ---------------------------------------------------------------------------------------------------
// the playback
// ---------------------------------------------------------------------------------------------------

/// A clip the motion object is playing, by whatever handle the app layer names clips with. The
/// original's is the address of the rider's own handle for it, which is why identity — not equality of
/// the clip's contents — is what decides whether the action-frame window carries over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipId(pub u32);

/// The transition between two clips: the original's second blend, which runs inside the motion object
/// rather than being asked for by the selection.
///
/// Nothing in the rider's own driving starts one — it is the "start this clip, easing out of the last"
/// entry point that does — but its clock is stepped by the same advance, so a port that leaves it out
/// gets the advance's field writes wrong.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fade {
    /// A transition is running: the pose comes from two clips, not from the one.
    pub on: bool,
    /// The clip the transition is coming *out* of, which is the one that was playing when
    /// [`Player::play`] put the new one on.
    ///
    /// `None` is the other way out: the rider's pose at that moment, snapshotted joint by joint
    /// ([`Snapshot`]), which is what a transition started on top of another transition fades from
    /// because the clip it would have faded from is itself half-way into a blend. Which of the two it
    /// is picks the sampler, and the sampler is the renderer's — but the choice is made here.
    pub from: Option<ClipId>,
    /// How long the transition has been going. Also the clip time the clip it is coming out of is
    /// sampled at, which is why it starts at `delay` rather than at zero: the old clip keeps playing
    /// through the cross-fade.
    pub clock: F,
    /// How long it waits before it starts moving — the clip time the old clip had reached, so that
    /// `clock - delay` is the time since the transition began.
    pub delay: F,
    /// How long the move itself takes.
    pub length: F,
    /// When the transition is dropped — compared against `clock` after the action frames, so the last
    /// frame of a transition still fires them.
    pub end: F,
}

impl Fade {
    /// The transition's weight this frame: a smooth step over `length`, after `delay`.
    ///
    /// The clock is advanced first and then this frame's `dt` is taken back off the numerator, so the
    /// weight is the one the *previous* frame's clock earns. One frame of lag, in the original.
    fn step(&mut self, dt: F) -> F {
        self.clock = self.clock + dt;
        let t = ((self.clock - self.delay) - dt) / self.length;
        if t < F::ZERO {
            F::ZERO
        } else if t < F::ONE {
            // The engine's smooth step normalises its input over its own range before the cubic, and
            // here that range is 0 to 1 — so the original divides by one. The port keeps the divide:
            // the PS2's divider is an approximation, and `x / 1.0` is not always `x`.
            let u = (t - F::ZERO) / F::ONE;
            // `(3 − 2u) · u²`, grouped the way the original's four multiplies group it: the square
            // first, on its own, and the bracket times it. `((3 − 2u) · u) · u` is a bit out.
            (F(3.0) - F(2.0) * u) * (u * u)
        } else {
            F::ONE
        }
    }
}

/// The motion object's clock: what is playing, how far into it, and where the last frame's sample was.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Player {
    /// The clip playing by itself. A phase-driven blend clears it — the two kinds of driving cannot
    /// both be running, and that is how a free-running clip is stopped.
    pub clip: Option<ClipId>,
    /// The clip the last sample was taken from, which decides whether this frame's action-frame
    /// window starts at the last sample or at the start of the clip.
    pub last: Option<ClipId>,
    /// The clip time this frame samples at.
    pub time: F,
    /// The clip time the previous frame sampled at.
    pub prev: F,
    /// A clip that does not loop has run out. Nothing advances again until something puts a clip on.
    pub finished: bool,
    /// The transition between two clips.
    pub fade: Fade,
}

/// What one dt advance did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Played {
    /// The clip time the action frames come from.
    pub window: Window,
    /// The transition's weight, when one was running this frame.
    pub fade: Option<F>,
    /// The clip reached its end and carried on from the start.
    pub looped: bool,
    /// The clip reached its end and stopped there.
    pub finished: bool,
}

impl Player {
    /// The free-running advance: one frame of a clip playing at its own pace.
    ///
    /// Nothing happens at all once a non-looping clip has finished, or while no clip is on — the
    /// original's one guard, and the reason a finished clip holds its last pose instead of resetting.
    ///
    /// The order is the original's and it matters: the previous sample is kept, `dt` goes on, and only
    /// *then* is the end tested, so the frame that runs out of clip is sampled past the end (a looping
    /// clip, after the wrap) and still fires its action frames.
    pub fn advance(&mut self, dt: F, clip: &Clip) -> Played {
        let Some(id) = self.clip else { return Played::default() };
        if self.finished {
            return Played::default();
        }
        self.last = Some(id);
        self.prev = self.time;
        self.time = self.time + dt;
        let (mut looped, mut done) = (false, false);
        if clip.over(self.time) {
            if clip.looping {
                looped = true;
                self.time = self.time - clip.duration;
            } else {
                done = true;
            }
        }
        let fade = self.fade.on.then(|| self.fade.step(dt));
        let window = if looped {
            Window::Wrapped { tail: (self.prev, clip.duration), head: (F::ZERO, self.time) }
        } else {
            Window::Span(self.prev, self.time)
        };
        // Both resets land after the action frames, so the last frame of a clip is a frame like any
        // other. The transition's is not conditional on one running: the original tests the clock
        // every frame.
        if self.fade.end <= self.fade.clock {
            self.fade.on = false;
            self.fade.clock = F::ZERO;
        }
        if done {
            self.finished = true;
            self.time = F::ZERO;
            self.fade.clock = F::ZERO;
        }
        Played { window, fade, looped, finished: done }
    }

    /// The phase-driven sample the blends use: the clip is read at `phase` of its length rather than
    /// advanced, and the free-running clip is taken off.
    ///
    /// Three things to know. The sample time is `duration × phase`, so the phase is where the frame
    /// is in the clip and nothing accumulates — hand it a progress that is one frame out and the
    /// whole clip is one frame out. The window is the two samples in the order they happened, which a
    /// phase that goes *backwards* turns into a wrap: the original decides "the clip wrapped" by
    /// comparing this sample against the last, so a trick whose progress wobbles backwards during its
    /// hold fires the action frames between the new sample and the end of the clip, and then
    /// everything from the start of the clip up to the new one. And the clip time the dt advance keeps
    /// is left alone: the two clocks are separate, so a free-running clip put back on carries on from
    /// where it was rather than from where the phase-driven one left off.
    ///
    /// On a blend of two clips it is the *first* one that is handed over here: both are sampled at the
    /// same number of seconds, and the action frames come from the first one's list.
    pub fn drive(&mut self, phase: F, clip: &Clip, id: ClipId) -> Window {
        self.drive_at(phase, clip, id, false)
    }

    /// The same, with the one thing the blend entries disagree about: whether a clip they have only
    /// just been handed, at a clip time of exactly zero, fires the empty window or nothing at all.
    /// [`Blend::fires_empty`] is which, and it is not cosmetic — an empty window still fires a key
    /// that sits on the clip's first frame, because the window test is inclusive at both ends.
    pub fn drive_at(&mut self, phase: F, clip: &Clip, id: ClipId, fires_empty: bool) -> Window {
        self.clip = None;
        let t = clip.duration * phase;
        let window = if self.last == Some(id) {
            if t < self.prev {
                Window::Wrapped { tail: (self.prev, clip.duration), head: (F::ZERO, t) }
            } else {
                Window::Span(self.prev, t)
            }
        } else if t != F::ZERO || fires_empty {
            // A clip that has only just been put on fires everything from its start up to here.
            Window::Span(F::ZERO, t)
        } else {
            Window::None
        };
        self.last = Some(id);
        self.prev = t;
        window
    }

    /// The call the selection asked for: the dispatch that ends the rider's animation frame.
    ///
    /// The original clamps the weights, switches on which entry point it wants, and calls it with the
    /// motion object, the cross-fade ramp and the handles — all of which [`Blend`] already carries.
    /// What is left is this: a blend drives the clock from its first clip ([`Blend::driven`]), and
    /// nothing driven means the clip already on advances by `dt` ([`Player::advance`]) instead.
    ///
    /// `clip` is the clip behind the handle the call works on — the blend's first one, or the one
    /// already playing when the blend is [`Blend::Free`]. [`Player::clock_clip`] names it.
    ///
    /// A blend whose first handle is missing is the one case the original does not survive: it hands
    /// the entry point a null handle and the entry dereferences it. The port does nothing instead,
    /// which is a divergence on a frame the original crashes on.
    pub fn run(&mut self, dt: F, blend: Blend, lib: &Library, clip: &Clip) -> Played {
        match blend.driven() {
            None => self.advance(dt, clip),
            Some((phase, slot)) => match lib.clip(slot) {
                None => Played::default(),
                Some(id) => Played {
                    window: self.drive_at(phase, clip, id, blend.fires_empty()),
                    ..Played::default()
                },
            },
        }
    }

    /// The handle whose clip [`Player::run`] needs this frame: the blend's first one, or the clip
    /// already playing when nothing is driven.
    pub fn clock_clip(&self, blend: Blend, lib: &Library) -> Option<ClipId> {
        match blend.driven() {
            None => self.clip,
            Some((_, slot)) => lib.clip(slot),
        }
    }

    /// Put a clip on, crossing out of whatever is playing over `put.length` seconds.
    ///
    /// This is the only thing in the original that sets [`Fade::on`]: the rider's own per-frame driving
    /// never starts a transition, it only ever asks for a phase-driven blend, so every transition in
    /// the game comes through here. The cross-fade [`Driver::ramp`] drives is a different one — that is
    /// the blend entry's own weight against the pose snapshot, and it is not this.
    ///
    /// `playing` is the duration of the clip already on, and only `put.keep_phase` reads it.
    ///
    /// Returns whether the pose snapshot was taken: the original asks for it from here as well as from
    /// a restart, when the transition has no clip to come out of. The caller owns the skeleton, so it
    /// is the caller that has to run [`Snapshot::take`] when this says so.
    pub fn play(&mut self, put: Put, playing: F) -> bool {
        // Worked out before anything is switched, from the old clip and the old clock, and written
        // back at the very end — after the hard cut or the transition has put the clock back to zero.
        let carried = put.keep_phase.then(|| put.duration * (self.time / playing));
        let mut snapshot = false;
        if put.length == F::ZERO {
            // A hard cut: no transition, and nothing to fade from.
            self.finished = false;
            self.clip = Some(put.clip);
            self.fade.on = false;
            self.time = F::ZERO;
        } else {
            // The transition's clock starts at the old clip's time, and its delay is that same time,
            // so the first frame of the cross-fade earns a weight of zero and the clip it is coming
            // out of carries on from where it was.
            self.fade.delay = self.time;
            self.fade.clock = self.time;
            self.fade.end = self.time + put.length;
            self.fade.length = put.length;
            self.fade.from = self.clip;
            if self.fade.from.is_none() || self.fade.on {
                // Nothing was playing, or a transition already is — in which case the clip it would
                // fade from is itself mid-blend. Either way it fades from the rider's pose instead.
                self.fade.from = None;
                snapshot = true;
            }
            self.finished = false;
            self.clip = Some(put.clip);
            self.time = F::ZERO;
            self.fade.on = true;
        }
        if let Some(t) = carried {
            self.time = t;
        }
        snapshot
    }
}

/// What putting a clip on takes: the clip, how long the cross into it is, and where its clock starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Put {
    /// How long the cross out of whatever is playing takes. **Zero is a hard cut** — no transition at
    /// all, the new clip from its first frame.
    pub length: F,
    /// The clip going on.
    pub clip: ClipId,
    /// Its duration. Read only by `keep_phase`.
    pub duration: F,
    /// Start the new clip at the same *fraction* of its own length as the old clip had reached, rather
    /// than at its beginning. The original divides by the old clip's duration to get there, so a caller
    /// that asks for this with nothing playing reads through a null clip.
    pub keep_phase: bool,
}

impl Put {
    /// Put a clip on with the cross-fade length the clip itself carries, which is how the engine's own
    /// one-argument play does it.
    pub fn clip(clip: ClipId, duration: F, length: F) -> Put {
        Put { length, clip, duration, keep_phase: false }
    }
}

// ---------------------------------------------------------------------------------------------------
// the pose a cross-fade fades from
// ---------------------------------------------------------------------------------------------------

/// What [`Snapshot::take`] needs of the rider's skeleton: how many joints it has, and each one's own
/// current local pose.
///
/// The two reads are the engine's own joint accessors — the rotation pulled back out of the joint's
/// local matrix as a quaternion, and the translation as a point. Both are the renderer's side of the
/// motion library, which is why they are asked for rather than done here.
pub trait Joints {
    /// How many joints the rider's skeleton has. The snapshot is this long.
    fn count(&self) -> usize;
    /// The joint's current local rotation, as a quaternion.
    fn rotation(&self, joint: usize) -> [F; 4];
    /// The joint's current local translation, as a point — the original writes 1 into the fourth lane.
    fn translation(&self, joint: usize) -> [F; 4];
}

/// The pose a cross-fade fades *from*: the rider's joints as they stood the moment the motion was
/// restarted or a clip was put on with nothing to come out of.
///
/// This is what the blend entries' `ramp` weight interpolates away from, and the reason a restarted
/// motion appears to start from the pose the rider was already in rather than snapping to the first
/// frame of the new clip. It is not a clip rewind: nothing about the clip's own clock moves.
///
/// Two buffers and not one, because the original keeps rotations and translations apart — the blend
/// entries read them in two separate passes, each one taking its own `ramp` branch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// Per joint, its rotation when the snapshot was taken.
    pub rotation: Vec<[F; 4]>,
    /// Per joint, its translation when the snapshot was taken.
    pub translation: Vec<[F; 4]>,
    /// Per joint, a tag the caller can hand in to be stored alongside the pose, exclusive-ored with a
    /// mask. `None` until something asks for one, and then kept for good — the original never frees
    /// it, and it leaks a four-byte allocation making it. No call site in the game asks.
    pub tag: Option<Vec<u32>>,
    /// Whether the pose has tags alongside it, which is the last caller's answer and not whether the
    /// buffer exists: the original keeps the two apart, so a snapshot taken without tags leaves the
    /// buffer it already had alone and only says it is not using it.
    pub tagged: bool,
}

impl Snapshot {
    /// Take the snapshot: every joint's rotation into one buffer and its translation into the other,
    /// in joint order.
    ///
    /// Both buffers are made on the first call and reused after it, and every call overwrites every
    /// entry — so a restart always fades from *this* frame's pose and never from a stale one. The
    /// original only ever allocates them when they are null, so a skeleton that grew joints between
    /// two snapshots would write past them; the port grows instead, which is the one place it is
    /// deliberately not the original.
    ///
    /// `tags` is the argument no caller in the game passes: with `None` the pose is simply not tagged,
    /// and with `Some` the buffer is made (filled with ones) if there is not one yet and then written
    /// with `mask ^ tags[joint]`, joint by joint, interleaved with the pose reads.
    pub fn take(&mut self, joints: &impl Joints, tags: Option<&[u32]>, mask: u32) {
        let n = joints.count();
        if self.rotation.len() < n {
            self.rotation = vec![[F::ZERO; 4]; n];
        }
        if self.translation.len() < n {
            self.translation = vec![[F::ZERO; 4]; n];
        }
        self.tagged = tags.is_some();
        if tags.is_some() && self.tag.as_ref().is_none_or(|t| t.len() < n) {
            self.tag = Some(vec![1; n]);
        }
        for joint in 0..n {
            if let (Some(tags), Some(tag)) = (tags, self.tag.as_mut()) {
                tag[joint] = mask ^ tags[joint];
            }
            self.rotation[joint] = joints.rotation(joint);
            self.translation[joint] = joints.translation(joint);
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// the hash
// ---------------------------------------------------------------------------------------------------

impl hash::State for Driver {
    fn walk(&self, w: &mut hash::Walk<'_>) {
        w.u8("action", self.action.0);
        w.f("ramp", self.ramp);
        w.bool("faded", self.faded);
        w.f("fore_aft", self.fore_aft);
        w.f("pose", self.pose);
        w.f("shown_crossup", self.shown_crossup);
    }
}

impl hash::State for Player {
    fn walk(&self, w: &mut hash::Walk<'_>) {
        w.u32("clip", self.clip.map_or(u32::MAX, |c| c.0));
        w.u32("last", self.last.map_or(u32::MAX, |c| c.0));
        w.f("time", self.time);
        w.f("prev", self.prev);
        w.bool("finished", self.finished);
        w.group("fade", |w| {
            w.bool("on", self.fade.on);
            w.u32("from", self.fade.from.map_or(u32::MAX, |c| c.0));
            w.f("clock", self.fade.clock);
            w.f("delay", self.fade.delay);
            w.f("length", self.fade.length);
            w.f("end", self.fade.end);
        });
    }
}
