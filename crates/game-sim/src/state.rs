//! The main actor's state machine: the states the player's bike-and-rider can be in, what moves it
//! between them, how long each state lasts, and which of those moves land on the same frame.
//!
//! The actor has no single idle/move switch. Its state is three axes, and the first two are ported
//! elsewhere:
//!
//! 1. **Where it is** — a wheel on the ground or in the air, decided by the suspension loads:
//!    [`crate::air`].
//! 2. **What it is doing** — the two idle poses, the three rotating moves and the 24 stunts, each an
//!    animation id with its own clocks: [`crate::trick`].
//! 3. **Whether it is riding at all** — riding, down, waiting, put back. That is this module, and it
//!    is the axis the other two hang off: while the rider is down the vehicle's physics step does not
//!    run at all (the base update skips it whenever a reason is set), so a wreck freezes the bike
//!    rather than simulating it on the floor.
//!
//! ## The states
//!
//! One value says which: [`Ride::wreck`], the reason the rider came off, or `None` while riding. The
//! reasons are the original's codes, shared with [`Wreck`] in [`crate::trick`] — the animation, the
//! camera and the front end all switch on them. [`Ride::phase`] folds the reason and the latch into the
//! three states the machine really has:
//!
//! - [`Phase::Riding`] — on the bike. The only state the wreck test can fire its landing clauses in.
//! - [`Phase::Down`] — off the bike with the reset clock not yet armed. The clock still runs, so the
//!   rest of the game can ask how long the rider has been down.
//! - [`Phase::Resetting`] — off the bike with the clock armed: when it passes the reason's limit the
//!   machine asks for the rider to be put back.
//!
//! ## The frame
//!
//! The machine is the last thing in the vehicle's update, after the physics step, and runs once:
//!
//! 1. [`Ride::frame_start`] — the hand-reset latch is dropped (the original clears it at the top of
//!    the player's own update, before anything else).
//! 2. the physics step, *only while the rider is not down*.
//! 3. [`Ride::step`] — the wreck test, the clock, the reset request, and the two bookkeeping writes.
//!
//! The pad's reset chord comes in beside all this, from the input phase: [`Ride::chord_allowed`],
//! [`Ride::take_chord`] and [`Ride::request_reset`].
//!
//! ## The clocks
//!
//! Seconds of `dt`, never frame counts, and both comparisons are "less or equal" — a window ends on
//! the first frame whose clock *exceeds* its limit.
//!
//! - The settle window, [`SETTLE`] (or [`SETTLE_CANNONBALL`] for a cannonball), is how long the rider
//!   is down before the reset clock is armed. It does not start until something tells the machine the
//!   vehicle has settled ([`Sense::settles`]) — for a bike, the rider hitting the ground after the
//!   wreck ([`crate::ride::Crash`]).
//! - The reset limit is the bike's own `MaxWreckTime`, or `CannonballWreckTime` for a cannonball
//!   ([`Params`]).
//!
//! The settle window tests the clock *before* adding to it and the reset tests *after*, so at a steady
//! 1/60 s a 0.5 s settle is 31 frames of adding and the latch on the 32nd, while a reset fires on the
//! first frame whose fresh total is past the limit.
//!
//! ## What lands on the same frame
//!
//! - **Riding → down.** The test's verdict, the entry (reason set, clock back to zero, latch cleared),
//!   the first `dt` of the clock, the wreck counter and the clearing of the race record's riding bit
//!   are all one frame.
//! - **Down → down.** The collision clause can wreck a rider who is already down, which restarts the
//!   clock through a second entry. The per-lap counter does not move again: it only counts the frames
//!   where the reason went from clear to set.
//! - **Down → riding.** A reset that is not shown as a page puts the rider back inside the same frame,
//!   so the frame ends with the reason clear and the riding bit *set* — the machine re-reads the reason
//!   after the respawn, which is why a same-frame reset skips the clear.
//! - **The arming frame.** When the settle window ends, the latch goes on and the clock goes back to
//!   zero on that frame and is not added to, so the reset cannot also fire on it.
//! - **No control.** Losing the driver is an entry plus a clock preloaded to the limit plus the latch,
//!   all on one frame, and no reset on that frame — the branch has none. The reset then lands on the
//!   first frame the rider is driven again, sharing it with that frame's add.
//!
//! `research/journal/2026-10-15-s5-s5-states/` has the addresses, the vtable slots and the proofs.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air;
use crate::hash;
use crate::trick::Wreck;
use ps2_float::F;

/// The two reasons the ride state machine decides for itself. The others are the wreck test's, and are
/// named on [`Wreck`] where that test lives.
impl Wreck {
    /// Thrown into something hard enough to be launched off the bike. It is the one reason with its own
    /// settle window and its own reset limit, and the one the hand reset refuses to cut short.
    pub const CANNONBALL: Wreck = Wreck(13);
    /// Nobody is driving: no AI route, and no local human on the pad either. The rider is put down and
    /// the clock is preloaded to its limit, so the first frame somebody does drive puts it back.
    pub const NO_CONTROL: Wreck = Wreck(14);
}

/// A verdict from the airborne wreck test as a reason code. The two tests that can wreck a rider live
/// in different modules and answer with different types; this machine takes the codes, which is what
/// the original stores.
impl From<air::Wreck> for Wreck {
    fn from(w: air::Wreck) -> Wreck {
        Wreck(w as u8)
    }
}

/// How long a settling vehicle lies there before its reset clock is armed.
pub const SETTLE: F = F(0.5);
/// The same, after a cannonball — five times as long.
pub const SETTLE_CANNONBALL: F = F(2.5);

/// The three states, read off [`Ride`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    /// On the bike.
    Riding,
    /// Off the bike, reset clock not armed. A wreck stays here until it settles ([`Sense::settles`]),
    /// or until the player or the front end's wreck page asks for the reset.
    Down,
    /// Off the bike with the reset clock armed against the reason's limit.
    Resetting,
}

/// Which kind of put-back a reset is. Stored by the request and read by the respawn, so a reset shown
/// as a page still knows which kind it was when the page finally lets it through.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Kind {
    /// Not the rider's fault: out of bounds, a bad lap, the restart. The rider is simply placed again.
    #[default]
    Plain = 0,
    /// After a wreck or a hand reset.
    Wrecked = 1,
}

/// The two numbers from the bike's own parameter file that the machine reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    /// `MaxWreckTime`: how long a down rider waits before being put back.
    pub max_wreck_time: F,
    /// `CannonballWreckTime`: the same for a cannonball, which is a longer show.
    pub cannonball_wreck_time: F,
}

/// The player record a rider hangs off. `None` for a rider with none, which is what the original's
/// three separate null checks mean: such a rider is never driven by a human, never blocks a reset on a
/// page, and cannot be reset at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Owner {
    /// A local human is driving. The original's control source is one of three values and this is the
    /// local-pad one; the other two are an AI and a remote player.
    pub human: bool,
    /// The owner has a screen of its own, with pages. A rider without one is put back on the spot
    /// instead of being shown the wreck page.
    pub screen: bool,
}

/// What the machine reads of the rest of the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sense {
    /// The rider has an AI route to follow. Either an AI or a local human counts as driving it; with
    /// neither, the machine puts the rider down with [`Wreck::NO_CONTROL`].
    pub ai_route: bool,
    /// Who owns the rider, if anybody.
    pub owner: Option<Owner>,
    /// Wrecks are switched on at all. One global in the original, cleared by the modes that must not
    /// wreck anybody.
    pub wrecks_enabled: bool,
    /// The vehicle has settled: the window in front of the reset clock may start. **Not a property of
    /// the class** — S5 read it as one and said so here, and that was wrong. It is a runtime signal,
    /// written during a wreck by the rider's own side of it, and a player bike's copy of it
    /// (`player + 0xd3c`) reads 1 part way through every wreck: 18, 22 and 26 ticks after the entry
    /// across three measured reason-5 wrecks. So the automatic reset is not out of reach for a bike —
    /// it is the normal way back, and the hand chord only cuts it short. Its writer is the rider body
    /// actor (S65): the end of its tick copies its touching flag (a bone of the ragdoll on something)
    /// into the request it shares with the bike, which is that word. The bike updates first, so this
    /// is the flag the previous tick left. The ragdoll sets it in the port
    /// ([`crate::ragdoll_ride::Down::settles`]); a rider down as one point has
    /// [`crate::ride::Crash`]'s reading of it.
    pub settles: bool,
}

/// Which clauses of the wreck test may fire. Both gates sit inside the test in the original and both
/// are the machine's business, so the test is handed them instead of reading them itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Allowed {
    /// Anything at all: [`Sense::wrecks_enabled`]. With this false the test does nothing.
    pub wrecks: bool,
    /// The attitude, g-load and landing clauses. False once the rider is already down: only the
    /// collision clause runs then, and that one only while the reason is not already the cannonball.
    pub landing: bool,
}

/// What the machine asks for when the clock runs out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reset {
    /// Put the rider back now, in this frame. The machine has already done its own half — see
    /// [`Ride::respawn`] for what the caller still owes.
    Now,
    /// Show the wreck page instead; the respawn comes later, when the page asks for it.
    Page,
}

/// What one frame of the machine asks the rest of the player for.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Step {
    /// A wreck entry ran this frame, with this reason. The machine's own half of the entry is done;
    /// the rest of it — placing the rider, stopping a running trick, the camera and the rest of the
    /// show — is the caller's, in the order [`Ride::enter`] lists.
    pub wreck: Option<Wreck>,
    /// The machine asked for the rider to be put back.
    pub reset: Option<Reset>,
    /// The one flag the machine reaches outside itself: a bit in the player's race record, cleared
    /// while the rider is down and set again by a respawn. `None` when this frame left it alone.
    pub riding_bit: Option<bool>,
}

/// The ride state: where the actor is between riding and being put back.
///
/// The original keeps all of it on the vehicle, spread between the physics fields and the player's
/// own; it is one value here so a snapshot is a copy. Nothing in it reads the clock, the pad or the
/// front end — the caller senses those into [`Sense`] and hands the frame time to [`Ride::step`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ride {
    /// Why the rider is off the bike, or `None` while riding.
    pub wreck: Option<Wreck>,
    /// The reset clock is armed: past the reason's limit the machine asks for the reset.
    pub counting: bool,
    /// Seconds on the clock. Restarted by an entry and by the arming frame, and *not* by a respawn —
    /// a rider put back carries the clock it had, which only the next entry clears.
    pub elapsed: F,
    /// The put-back kind the last request stored.
    pub kind: Kind,
    /// A hand reset has been asked for and the update has not run since. It stops the chord being
    /// taken twice in one frame; [`Ride::frame_start`] drops it.
    pub pending: bool,
    /// The rider has not touched down since it was placed. A respawn sets it; the first frame with a
    /// wheel on the ground clears it, which the wheel code does — it is what stops a trick gesture
    /// counting while the bike is still dropping in.
    pub fresh: bool,
    /// The wreck is being shown: set by an entry with a reason, cleared by a respawn.
    pub wrecking: bool,
    /// Wrecks since the stats were last taken. Counts the frames where the reason went from clear to
    /// set, so a re-wreck while already down does not count twice.
    pub wrecks: i32,
    /// Wreck entries since the rider was last restarted. Every entry, including those re-wrecks.
    pub entries: i32,
}

impl Ride {
    /// Which of the three states the rider is in.
    pub fn phase(&self) -> Phase {
        match (self.wreck, self.counting) {
            (None, _) => Phase::Riding,
            (Some(_), false) => Phase::Down,
            (Some(_), true) => Phase::Resetting,
        }
    }

    /// The top of the player's update, before the physics step: the hand-reset latch is dropped.
    pub fn frame_start(&mut self) {
        self.pending = false;
    }

    /// One frame of the machine, last thing in the vehicle's update.
    ///
    /// `test` is the bike's wreck test — the airborne clauses and the trick's landing clause together —
    /// run once, on the frame's driven path only, and handed the two gates it must not decide for
    /// itself ([`Allowed`]). Whatever reason it answers with is entered, even one for a rider already
    /// down. `pages` answers whether the owner's screen already has a page up; it is asked only at the
    /// moment the machine would raise one, and only of an owner that has a screen.
    pub fn step(
        &mut self,
        dt: F,
        sense: Sense,
        p: Params,
        test: impl FnOnce(Allowed) -> Option<Wreck>,
        pages: impl FnOnce() -> bool,
    ) -> Step {
        let prior = self.wreck;
        let mut out = Step::default();
        let driven = sense.ai_route || sense.owner.is_some_and(|o| o.human);
        if driven {
            let allowed = Allowed { wrecks: sense.wrecks_enabled, landing: self.wreck.is_none() };
            if let Some(reason) = test(allowed) {
                self.enter(reason);
                out.wreck = Some(reason);
            }
            if let Some(reason) = self.wreck {
                // The clock. Without a settle window, or once the latch is on, it simply runs.
                if !sense.settles || self.counting {
                    self.elapsed = self.elapsed + dt;
                } else {
                    let window =
                        if reason == Wreck::CANNONBALL { SETTLE_CANNONBALL } else { SETTLE };
                    if self.elapsed <= window {
                        self.elapsed = self.elapsed + dt;
                    } else {
                        // The arming frame: the clock starts again from zero and is not added to.
                        self.counting = true;
                        self.elapsed = F::ZERO;
                    }
                }
                let limit = if reason == Wreck::CANNONBALL {
                    p.cannonball_wreck_time
                } else {
                    p.max_wreck_time
                };
                // The original compares "clock less or equal limit" and skips on true, so the reset
                // wants a clock strictly past the limit.
                if self.counting && !(self.elapsed <= limit) {
                    let screen = sense.owner.is_some_and(|o| o.screen);
                    if !(screen && pages()) {
                        // Losing the driver is put right silently; every other reason is shown.
                        let page = reason != Wreck::NO_CONTROL;
                        out.reset = self.request_reset(Kind::Wrecked, page, sense.owner);
                        if out.reset == Some(Reset::Now) {
                            self.respawn();
                            out.riding_bit = Some(true);
                        }
                    }
                }
            }
        } else if prior.is_none() {
            self.enter(Wreck::NO_CONTROL);
            out.wreck = Some(Wreck::NO_CONTROL);
            // Preloaded to the limit, not past it: the reset waits for a frame with a driver.
            self.elapsed = p.max_wreck_time;
            self.counting = true;
        }
        // Both paths end here, reading the reason as it stands now — which a same-frame respawn has
        // already cleared.
        if self.wreck.is_some() {
            out.riding_bit = Some(false);
        }
        if prior.is_none() && self.wreck.is_some() {
            self.wrecks += 1;
        }
        out
    }

    /// The wreck entry: the rider comes off, for this reason.
    ///
    /// The machine's half is the reason, the clock and the latch, the shown flag and the entry count.
    /// The rest of the original's entry, in its order, is the caller's and is not ported here: a
    /// running trick is stopped (hand-hold off, animation reset, the trick queue cleared), the rider
    /// is placed at the bike's rider point with the bike's drop velocity — three draws from the shared
    /// random sequence instead, for a cannonball — the wheel loads are zeroed, and the camera, the
    /// rumble and the announcer are told.
    pub fn enter(&mut self, reason: Wreck) {
        self.wreck = (reason != Wreck(0)).then_some(reason);
        self.elapsed = F::ZERO;
        self.counting = false;
        self.wrecking = reason != Wreck(0);
        self.entries += 1;
    }

    /// Ask for the rider to be put back, storing the kind.
    ///
    /// `page` asks for the wreck page rather than an immediate put-back, and is only honoured for an
    /// owner with a screen — without one there is nothing to show it on. A rider with no owner at all
    /// is not reset and does not even store the kind.
    ///
    /// The original also ends the player's jump record here, before the owner check, and tells the
    /// network and the camera; none of that is this module's.
    pub fn request_reset(&mut self, kind: Kind, page: bool, owner: Option<Owner>) -> Option<Reset> {
        let owner = owner?;
        self.kind = kind;
        if owner.screen && page { Some(Reset::Page) } else { Some(Reset::Now) }
    }

    /// The put-back itself: the rider is on the bike again.
    ///
    /// Returns the kind the request stored, which the rest of the put-back needs. The clock and the
    /// latch are deliberately left where they were — only an entry clears them.
    ///
    /// The caller still owes the rest, in the original's order: the attitude the wreck test reads is
    /// zeroed, a cannonball's damping is put back, the animation is reset to the grounded idle pose
    /// and the trick queue emptied, the bike's placement runs for this kind, and the race record's
    /// riding bit goes back on ([`Step::riding_bit`]).
    pub fn respawn(&mut self) -> Kind {
        self.wreck = None;
        self.wrecking = false;
        self.fresh = true;
        self.kind
    }

    /// The machine's gates on the pad's reset chord: an AI-driven rider is not reset by hand, a reset
    /// already asked for this frame is not asked for twice, and a cannonball is not cut short.
    ///
    /// The caller's own gates — the two buttons, the game mode, whose pad it is, and the network's veto
    /// — come either side of this one: the original takes the latch after its pad tests and before its
    /// network test, which is why taking it ([`Ride::take_chord`]) and asking for the reset
    /// ([`Ride::request_reset`] with [`Kind::Wrecked`] and no page) are separate calls.
    pub fn chord_allowed(&self, ai_route: bool) -> bool {
        !ai_route && !self.pending && self.wreck != Some(Wreck::CANNONBALL)
    }

    /// Take the chord: nothing else asks for a reset until the next update.
    pub fn take_chord(&mut self) {
        self.pending = true;
    }
}

impl hash::State for Ride {
    fn walk(&self, w: &mut hash::Walk<'_>) {
        w.u8("wreck", self.wreck.map_or(0, |r| r.0));
        w.bool("counting", self.counting);
        w.f("elapsed", self.elapsed);
        w.u8("kind", self.kind as u8);
        w.bool("pending", self.pending);
        w.bool("fresh", self.fresh);
        w.bool("wrecking", self.wrecking);
        w.i32("wrecks", self.wrecks);
        w.i32("entries", self.entries);
    }
}
