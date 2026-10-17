//! The fixed update: one frame of the game, from the mode mailbox to the simulation pass.
//!
//! This is the top of the port's call graph. Everything else in this crate is something the
//! [`Engine::step`] below eventually calls, directly or through the scene tree, and the order it calls
//! them in is this module's whole subject.
//!
//! # The frame, as the original runs it
//!
//! The original's loop is six instructions: call the game's tick through a dispatch table until it
//! returns false. The tick itself is two functions — the game's, which services one *mode request* and
//! then calls the engine's, which is the frame proper. In order:
//!
//! 1. **Service at most one mode request** ([`Mailbox`]). The request word is a mailbox, not a state: a
//!    menu or a race writes a number into it, the next frame acts on it and clears it back to 0. A
//!    shutdown request returns without running the frame at all; every other request falls through into
//!    the frame once it has been serviced — or, while its guard is unsatisfied, without being serviced.
//! 2. **Sample the wall clock** ([`Timer0`], [`clock_ms`]): EE Timer 0, in integer milliseconds. Not
//!    vsync, and not a frame count.
//! 3. **Derive `dt`** ([`DeltaTime`]): the raw delta in seconds, a 30-entry moving average, then the
//!    30 Hz clamp and the time-scale multiplier. The simulation is handed the *averaged* value.
//! 4. **Read the pads** — the device manager's update, handed the same `dt`. This is the only point in
//!    the frame where input enters, and it is before the simulation: a frame simulates the sample taken
//!    at its own top, never one taken mid-frame.
//! 5. **Frame marker 1** ([`Marker::FrameStart`]): instrumentation, no state.
//! 6. **Unless the frame is frozen** ([`Engine::frozen`]): phase = [`Phase::Simulate`], store `dt` and
//!    `1/dt`, advance the frame counter, then **the simulation** — one call on the scene tree's root,
//!    with `dt` as its only argument. Then phase = 0, marker 2, and the render.
//! 7. The audio side's queued commands go to the IOP whether or not step 6 ran, and this frame's clock
//!    becomes the next frame's previous clock.
//!
//! What is *not* here: the render, the display-list ring and the IOP RPC. They are the app layer's, and
//! this crate does not draw, read a device or touch a file. [`Step`] reports what happened so the app
//! layer can do its half in the right order.
//!
//! # Where the random numbers come from
//!
//! One generator, [`crate::rng::Rng`], the C library's LCG — the game has no other. The fixed update
//! **never draws from it**: there is not a single call to it in either tick function, so the sequence a
//! frame consumes is entirely the subsystems', in the order the scene tree walks them. That is why the
//! generator is a field of [`Engine`] handed to [`Game::simulate`] rather than a global: the number of
//! draws per frame is a property of the simulation, and a replay that diverges in the draw *count*
//! diverges in the next frame's state even when this frame's looks right.
//!
//! # Loading frames and frozen frames
//!
//! Three different things, often confused:
//!
//! * **A guarded request** ([`Serviced::Waiting`]): requests 2 and 7 wait for the UI to finish loading,
//!   requests 4 and 6 wait out a two-frame delay. The frame runs completely normally meanwhile — it
//!   ticks, it reads input, it simulates. The request is simply re-examined next frame.
//! * **A blocking handler**: the handlers behind requests 1, 2 and 6 load a world or tear one down
//!   inside the mode service, and the frame does not advance while they do (measured at ~19 s for a
//!   world load). The port models this as time that passes between two [`Engine::step`]s, which is what
//!   it is: the next frame's raw `dt` is enormous, which is exactly the case
//!   [`DeltaTime::LONG_FRAME`] exists to throw away.
//! * **A frozen frame** ([`Engine::frozen`]): the gate the engine checks before simulating. The clock
//!   is still sampled, `dt` is still averaged, the pads are still read and the top-of-frame marker
//!   still fires — only the simulation, the frame counter, the second marker and the render are
//!   skipped. Nothing found so far sets it, so it is dead in the retail build; it is modelled because
//!   a port that cannot stop its simulation cannot be debugged, and because "paused" in this game is a
//!   UI page over a running frame, not this.
//!
//! [`Engine::suspended`] is none of the three, and is easy to mistake for the first: the end of every
//! frame raises it when [`Engine::suspend_requested`] is set, one frame late. It gates work *outside*
//! the fixed update — several per-object updates elsewhere short-circuit while it is set — and never
//! the simulation. It is here because it is the last thing the frame does, and a port that drops it
//! loses a one-frame delay that other code can see.
//!
//! The pause menu, the results screen and the title are *pages*, not states: the simulation keeps
//! ticking under them. Nothing in the fixed update knows they exist.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{F, fpu};

use crate::hash::{State, Walk};
use crate::rng::Rng;
use crate::{Input, hash};

// ---------------------------------------------------------------------------------------------------
// The wall clock
// ---------------------------------------------------------------------------------------------------

/// The two words the tick reads, with interrupts off, to know what time it is.
///
/// The original programs EE Timer 0 to count the bus clock divided by 256 — 576,000 Hz — and to raise
/// an interrupt every 57,600 counts, which is exactly 100 ms. Its handler bumps a word and takes the
/// counter back to zero, so the pair below is "whole 100 ms periods" and "counts into the current one".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timer0 {
    /// The interrupt handler's count: one per 100 ms.
    pub ticks: u32,
    /// The timer's own counter, 0..57,600 within the current 100 ms.
    pub count: u32,
}

/// Milliseconds per Timer 0 interrupt.
pub const WALL_TICK_MS: i32 = 100;
/// The numerator of the sub-tick conversion, as the original spells it: counts × 256,000 …
pub const TIMER0_NUMERATOR: f32 = 256_000.0;
/// … divided by the bus clock, 147,456,000. The pair is `count / 576` in one multiply and one divide,
/// and the port keeps both constants because the rounding of the intermediate product is part of the
/// answer.
pub const TIMER0_DENOMINATOR: f32 = 147_456_000.0;
/// 2³¹, where the original's float-to-unsigned conversion splits.
const TWO_31: f32 = 2_147_483_648.0;

/// The wall clock in whole milliseconds, exactly as the engine computes it.
///
/// Three things in here are not what a reimplementation would write, and all three are observable:
/// the arithmetic is done in *single precision* (so the answer loses its low bits once the game has
/// been running for about a day), the whole-tick part is converted as **unsigned** and the sub-tick
/// part is added to it rather than the other way round, and the result is turned back into a 32-bit
/// word that the caller then treats as signed.
pub fn clock_ms(t: Timer0) -> i32 {
    // `ticks * 100` in integers first, then converted as an unsigned — the compiler's halve-convert-
    // double idiom, which is what makes this wrap at 2³² rather than saturating at 2³¹.
    let whole = F(fpu::from_u32((t.ticks as i32).wrapping_mul(WALL_TICK_MS) as u32));
    let sub = F(fpu::from_u32(t.count)) * F(TIMER0_NUMERATOR) / F(TIMER0_DENOMINATOR);
    let ms = sub + whole;
    // float → unsigned, the way a compiler emits it: below 2³¹ a plain `cvt.w.s`, above it the
    // difference with the top bit put back by hand.
    if ms >= TWO_31 {
        ((ms - F(TWO_31)).to_i32() as u32 | 0x8000_0000) as i32
    } else {
        ms.to_i32()
    }
}

// ---------------------------------------------------------------------------------------------------
// dt
// ---------------------------------------------------------------------------------------------------

/// The frame-time pipeline: a 30-entry moving average of the wall clock, clamped.
///
/// The simulation is **not** handed 1/60, and not even this frame's measured delta: it is handed the
/// mean of the last thirty frames' deltas, and then, while the 30 fps cap is on, the smaller of that
/// mean and 0.03333. A replay that feeds a fixed step reproduces neither.
#[derive(Clone, Copy, Debug)]
pub struct DeltaTime {
    /// The thirty most recent raw deltas, oldest-to-newest only by accident: the ring is summed from
    /// index 0 every frame, so where the write head is does not affect the mean.
    pub ring: [F; DeltaTime::RING],
    /// Where the *next* delta goes.
    pub index: u32,
    /// Cleared at boot and by any frame longer than [`DeltaTime::LONG_FRAME`]; the next frame then
    /// fills the whole ring with its own delta instead of averaging a gap into the next second of play.
    pub primed: bool,
    /// This frame's delta before averaging — after the long-frame clamp, which is where the original
    /// stores it.
    pub raw: F,
}

impl Default for DeltaTime {
    fn default() -> DeltaTime {
        DeltaTime {
            ring: [F(0.0); DeltaTime::RING],
            index: 0,
            primed: false,
            raw: F(0.0),
        }
    }
}

impl DeltaTime {
    /// How many frames the average spans.
    pub const RING: usize = 30;
    /// A frame longer than this is not a frame, it is a level load: the history is thrown away …
    pub const LONG_FRAME: f32 = 0.1;
    /// … and this stands in for it. Note it is 0.033, while the 30 Hz clamp below is 0.03333 — two
    /// different constants in the original, two different constants here.
    pub const AFTER_LONG_FRAME: f32 = 0.033;
    /// The 30 fps cap, applied *after* the mean. This build of the game sets the cap flag in the game
    /// object's constructor and never clears it, so this is the value the simulation runs at.
    pub const CAP_30: f32 = 0.03333;
    /// The millisecond step that goes with the 30 Hz clamp (33, written as 0x21 in the original).
    pub const CAP_30_MS: i32 = 0x21;
    /// Time scaling cannot take a frame below this.
    pub const FLOOR: f32 = 0.001;
    /// Milliseconds to seconds, as a multiply by a constant that is *not* exact in binary — so the
    /// product depends on the multiply, and the port does it with the same one.
    pub const MS_TO_S: f32 = 0.001;

    /// One frame of the pipeline: push `delta_ms`, return what the simulation gets and the millisecond
    /// step that goes with it.
    ///
    /// `scale` is the time-scale object's factor (slow motion, fast forward), `None` when no such
    /// object exists — which is the normal case, and not the same as `Some(0.0)`: a factor of zero
    /// still rewrites the millisecond step.
    pub fn update(&mut self, delta_ms: i32, cap_30: bool, scale: Option<F>) -> (F, i32) {
        let mut ms_step = delta_ms;
        let mut dt = F(Self::MS_TO_S) * F::from(delta_ms);
        if dt > Self::LONG_FRAME {
            self.primed = false;
            dt = F(Self::AFTER_LONG_FRAME);
        }
        self.raw = dt;
        if !self.primed {
            self.primed = true;
            self.ring = [dt; Self::RING];
        }
        // The write and the index advance happen before the sum, so this frame's delta is one of the
        // thirty being averaged.
        self.ring[self.index as usize] = dt;
        self.index = (self.index + 1) % Self::RING as u32;

        // Summed from the start of the ring, in index order, one add at a time. The original unrolls
        // this by six, but the chain is the same left fold — and the order is the answer: thirty
        // single-precision adds in another order give another number.
        let mut sum = F(0.0);
        for &entry in &self.ring {
            sum += entry;
        }
        let mut dt = sum / F::from(Self::RING as i32);

        if cap_30 && dt > Self::CAP_30 {
            ms_step = Self::CAP_30_MS;
            dt = F(Self::CAP_30);
        }
        if let Some(scale) = scale {
            // Guarded on a positive mean, so the first frame of all (whose mean is zero) is never
            // scaled, and the floor below cannot turn it into 0.001.
            if dt > 0.0 {
                dt += scale * dt;
                if dt < Self::FLOOR {
                    ms_step = 1;
                    dt = F(Self::FLOOR);
                } else {
                    ms_step = ms_step.wrapping_add((scale * F::from(ms_step)).to_i32());
                }
            }
        }
        (dt, ms_step)
    }
}

// ---------------------------------------------------------------------------------------------------
// The mode mailbox
// ---------------------------------------------------------------------------------------------------

/// A value written into the game's mode-request word, asking for one transition.
///
/// It is a mailbox and not a state variable: it reads 0 almost always, a subsystem writes a request
/// into it, and the next frame services exactly one and clears it. "Am I in a race?" is answered by
/// whether a world is loaded, not by this.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Request {
    /// Build the world and its activity. Blocks the frame for as long as the load takes.
    EnterWorld,
    /// Leave the world and go back to the main menu. Guarded by the UI's still-loading pair.
    WorldToMenu,
    /// The boot-time memory-card check, then the main menu. Guarded by a two-frame delay.
    CheckToMenu,
    /// Rebuild the front end's pages in place, without touching the world.
    RebuildFrontEnd,
    /// Main menu into a race. Guarded by a two-frame delay.
    MenuToWorld,
    /// Restart the current event: leave the world, then chain into [`Request::EnterWorld`].
    RestartEvent,
    /// Shut the game down. The only request that does not run the frame it is serviced on.
    Shutdown,
}

impl Request {
    /// The number the original's mode word holds, so a capture of that word reads straight across.
    /// There is deliberately no 3: the switch has no such case and nothing in the game writes one.
    pub fn code(self) -> u32 {
        match self {
            Request::EnterWorld => 1,
            Request::WorldToMenu => 2,
            Request::CheckToMenu => 4,
            Request::RebuildFrontEnd => 5,
            Request::MenuToWorld => 6,
            Request::RestartEvent => 7,
            Request::Shutdown => 8,
        }
    }

    /// The request a mode word holds, or `None` for 0 and for the codes the switch ignores.
    pub fn from_code(code: u32) -> Option<Request> {
        Some(match code {
            1 => Request::EnterWorld,
            2 => Request::WorldToMenu,
            4 => Request::CheckToMenu,
            5 => Request::RebuildFrontEnd,
            6 => Request::MenuToWorld,
            7 => Request::RestartEvent,
            8 => Request::Shutdown,
            _ => return None,
        })
    }

    /// Whether this request waits for the UI manager to finish loading before it is serviced.
    pub fn waits_for_ui(self) -> bool {
        matches!(self, Request::WorldToMenu | Request::RestartEvent)
    }

    /// Whether this request arms the two-frame delay before it is serviced.
    pub fn waits_two_frames(self) -> bool {
        matches!(self, Request::CheckToMenu | Request::MenuToWorld)
    }
}

/// The mode word and the two-frame delay that goes with it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mailbox {
    /// The raw request word. Kept as a number rather than a [`Request`] because the original's is a
    /// number, and a code with no case in the switch has to behave the way the original's does:
    /// nothing happens and the word stays put.
    pub request: u32,
    /// Set on the frame a delayed request is first seen.
    pub armed: bool,
    /// Counts the frames since. The delayed request is serviced on the frame this is already at 1.
    pub delay: i32,
}

impl Mailbox {
    /// Ask for a transition. Overwrites whatever was pending, which is what the original's stores do.
    pub fn post(&mut self, r: Request) {
        self.request = r.code();
    }

    /// Whether anything is pending.
    pub fn pending(&self) -> Option<Request> {
        Request::from_code(self.request)
    }
}

/// What the mode service did this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Serviced {
    /// The mailbox was empty, or held a code the switch has no case for.
    Nothing,
    /// The request's guard is not satisfied yet — the UI is still loading, or the two-frame delay is
    /// still counting. The frame runs anyway and the request stays pending.
    Waiting(Request),
    /// The transition ran. The mailbox is clear, except after [`Request::RestartEvent`], which chains
    /// by posting [`Request::EnterWorld`] for the next frame.
    Performed(Request),
}

// ---------------------------------------------------------------------------------------------------
// Phases and frame markers
// ---------------------------------------------------------------------------------------------------

/// What the engine is doing, in the word subsystems read when they need to know.
///
/// The port keeps the original's numbering because the value is observable in a capture and because
/// subsystems branch on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Between the named phases — which is where the engine leaves it, and where a sample taken from
    /// outside the game almost always finds it.
    #[default]
    Idle,
    /// The one-time initialisation pass.
    Init,
    /// The simulation. The only phase this crate ever sets.
    Simulate,
    /// The render's four scene passes, in the order the renderer runs them.
    ScenePassA,
    ScenePassB,
    ScenePassC,
    ScenePassD,
}

impl Phase {
    /// The number in the engine's phase word.
    pub fn code(self) -> u32 {
        match self {
            Phase::Idle => 0,
            Phase::Init => 1,
            Phase::Simulate => 2,
            Phase::ScenePassA => 3,
            Phase::ScenePassB => 4,
            Phase::ScenePassC => 5,
            Phase::ScenePassD => 6,
        }
    }
}

/// A point in the frame the engine announces to anything listening.
///
/// Instrumentation only — no observer in the retail build changes simulation state — but the ids are
/// part of the frame's shape, and the two the fixed update raises bracket the simulation exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    /// Top of the frame: after `dt` and the pads, before the simulation.
    FrameStart = 1,
    /// The simulation has returned; the render has not started.
    SimulationDone = 2,
    /// Mid-render, after the second scene pass.
    MidRender = 3,
    /// A view has been finished.
    ViewFinished = 4,
    /// Idling in the flip, waiting for the field. Raised once per flip, and only if the wait is longer
    /// than four milliseconds.
    WaitingForField = 8,
}

/// The markers one [`Engine::step`] raised, in order. Fixed-size: a frame raises at most two, and a
/// simulation that allocates per frame is a simulation that stutters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Markers {
    raised: [Option<Marker>; 2],
    len: usize,
}

impl Markers {
    fn push(&mut self, m: Marker) {
        self.raised[self.len] = Some(m);
        self.len += 1;
    }

    /// The markers raised, in the order they were raised.
    pub fn iter(&self) -> impl Iterator<Item = Marker> + '_ {
        self.raised[..self.len].iter().flatten().copied()
    }

    /// Whether a marker was raised this frame.
    pub fn has(&self, m: Marker) -> bool {
        self.iter().any(|x| x == m)
    }
}

// ---------------------------------------------------------------------------------------------------
// The frame
// ---------------------------------------------------------------------------------------------------

/// The part of a frame that is not this crate's: the game's own subsystems, which the fixed update
/// calls in a fixed order.
///
/// Implement it on whatever holds the port's world. The default bodies are the behaviour of a game
/// with no front end and nothing loading, which is what a unit test wants.
pub trait Game {
    /// One simulation pass over the scene tree, with the frame's `dt` and the input sampled at the top
    /// of this frame. Returning `false` is the original's "the simulation wants to stop".
    ///
    /// Everything in the game that uses chance draws from `rng`, in this call, in tree order.
    fn simulate(&mut self, dt: F, input: Input, rng: &mut Rng) -> bool;

    /// Is the UI manager still loading? The guard on [`Request::WorldToMenu`] and
    /// [`Request::RestartEvent`].
    fn ui_loading(&self) -> bool {
        false
    }

    /// Perform a mode transition: load a world, build the front end, tear one down. These are whole
    /// subsystems and none of them is ported yet, so the fixed update calls this at the point the
    /// original calls the handlers — before the frame, inside the same tick — and leaves what they do
    /// to the implementation.
    fn transition(&mut self, _t: Request) {}

    /// Shut down. Called for [`Request::Shutdown`], on the frame that does not run.
    fn shutdown(&mut self) {}
}

/// What one [`Engine::step`] did, for the app layer that has to render and flip in step with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// What the mode service did before the frame.
    pub serviced: Serviced,
    /// What the simulation was handed. Also in [`Engine::dt_s`].
    pub dt: F,
    /// Its reciprocal, as the engine stores it: `1/dt`, or 1.0 when `dt` is not positive.
    pub inv_dt: F,
    /// How many milliseconds this frame added to the engine's accumulated clock. Not the same as the
    /// wall-clock delta once the 30 Hz clamp or a time scale is in play.
    pub ms_step: i32,
    /// Whether the simulation ran. False on a frozen frame and on a shutdown frame.
    pub simulated: bool,
    /// Whether the simulation asked to stop. The original **discards this**: the engine tick returns 0
    /// and the game's tick ignores the return, so the loop keeps going. Reported here so a port can
    /// see it happen without changing what it does.
    pub sim_halted: bool,
    /// The frame counter after this frame.
    pub frame: u32,
    /// The markers the fixed update raised, in order.
    pub markers: Markers,
    /// Whether the loop should run another frame.
    pub outcome: Outcome,
}

/// Whether the game's loop continues after a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Run another frame.
    Continue,
    /// Leave the loop: a serviced [`Request::Shutdown`], or the quit flag set during the frame.
    Quit,
}

/// The fixed update's own state: the clock, the frame-time pipeline, the mailbox and the counters the
/// whole engine reads.
///
/// Every field here is one the original keeps in its application object, and the names say what they
/// are rather than where they lived.
#[derive(Clone, Debug, Default)]
pub struct Engine {
    /// This frame's wall clock, in milliseconds.
    pub clock_ms: i32,
    /// The previous frame's. Zero means "the clock has never been sampled", which is how the original
    /// recognises its first frame.
    pub prev_clock_ms: i32,
    /// Accumulated simulated milliseconds: the sum of every frame's step, which is *not* the wall clock
    /// once the 30 Hz clamp or a time scale has been applied.
    pub sim_ms: i32,
    /// The previous frame's [`Engine::sim_ms`].
    pub prev_sim_ms: i32,
    /// The frame-time pipeline.
    pub dt: DeltaTime,
    /// What the simulation was handed this frame.
    pub dt_s: F,
    /// And its reciprocal.
    pub inv_dt: F,
    /// The engine's own frame number, which wraps at 2³¹−1. Nothing else in the game counts frames;
    /// the vsync-looking counter a probe finds counts *fields*, and runs at twice this rate.
    pub frame: u32,
    /// What the engine is doing. Left at [`Phase::Idle`] between frames.
    pub phase: Phase,
    /// The input sampled at the top of this frame — the one value the whole frame simulates from.
    pub input: Input,
    /// The one random generator the game has. The fixed update never draws from it.
    pub rng: Rng,
    /// The mode mailbox.
    pub mode: Mailbox,
    /// Set when something asks to quit; checked after the frame, which is why a quit still simulates
    /// the frame it was asked on.
    pub quit: bool,
    /// Skip the simulation, the frame counter, the second marker and the render. Nothing found so far
    /// sets it.
    pub frozen: bool,
    /// The deferred flag the end of the frame raises. Not the simulation gate: see the module docs.
    pub suspended: bool,
    /// Ask for [`Engine::suspended`] from the *next* frame on. The engine latches it at the very end of
    /// the frame, after the clocks, so asking mid-frame still lets this frame finish unchanged.
    pub suspend_requested: bool,
    /// The 30 fps cap. Set by the game object's constructor in this build and never cleared.
    pub cap_30: bool,
    /// The time-scale factor, when a time-scale object exists: `dt += scale × dt`. `None` is the normal
    /// case; `Some(0.0)` is not the same thing.
    pub time_scale: Option<F>,
}

impl Engine {
    /// A fresh engine: no clock yet, the 30 fps cap on, and the generator in the state the program
    /// image links it with.
    pub fn new() -> Engine {
        Engine {
            cap_30: true,
            ..Default::default()
        }
    }

    /// One frame.
    ///
    /// `now` is the wall clock this frame was woken at — the app layer's only job besides rendering —
    /// and `pad` is the controller as the device manager would have found it.
    pub fn step(&mut self, now: Timer0, pad: Input, game: &mut impl Game) -> Step {
        self.step_at(clock_ms(now), pad, game)
    }

    /// One frame, from an already-sampled clock.
    ///
    /// The same thing as [`Engine::step`] with the clock conversion done by the caller: what a capture
    /// of a running game holds is the sampled millisecond value, so a replay feeds that rather than
    /// re-deriving it from a timer that is not running any more.
    pub fn step_at(&mut self, sampled_ms: i32, pad: Input, game: &mut impl Game) -> Step {
        // 1. One mode request, at most. A shutdown returns before the frame.
        let serviced = self.service(game);
        if serviced == Serviced::Performed(Request::Shutdown) {
            return Step {
                serviced,
                dt: self.dt_s,
                inv_dt: self.inv_dt,
                ms_step: 0,
                simulated: false,
                sim_halted: false,
                frame: self.frame,
                markers: Markers::default(),
                outcome: Outcome::Quit,
            };
        }

        // 2. The clock. On the very first frame the engine primes both ends of the delta with the same
        //    sample instead of measuring one, so the first frame's dt is exactly zero.
        self.clock_ms = sampled_ms;
        if self.prev_clock_ms == 0 {
            self.prev_clock_ms = sampled_ms;
            self.sim_ms = sampled_ms;
            self.prev_sim_ms = sampled_ms;
        }

        // 3. dt, and the millisecond step that goes with it.
        let delta_ms = self.clock_ms.wrapping_sub(self.prev_clock_ms);
        let (dt, ms_step) = self.dt.update(delta_ms, self.cap_30, self.time_scale);
        self.sim_ms = self.sim_ms.wrapping_add(ms_step);

        // 4. The pads, handed the frame's dt. One sample, before anything simulates.
        self.input = pad;

        // 5. Top-of-frame marker.
        let mut markers = Markers::default();
        markers.push(Marker::FrameStart);

        // 6. The simulation, unless the frame is frozen.
        let mut simulated = false;
        if !self.frozen {
            self.phase = Phase::Simulate;
            self.dt_s = dt;
            self.inv_dt = if dt > 0.0 { F(1.0) / dt } else { F(1.0) };
            self.frame = self.frame.wrapping_add(1);
            if self.frame >= 0x7fff_ffff {
                self.frame = 0;
            }
            simulated = true;
            if game.simulate(dt, self.input, &mut self.rng) {
                self.phase = Phase::Idle;
                markers.push(Marker::SimulationDone);
                // The render goes here, in the app layer: marker 2, then the four scene passes, then
                // the flip. Nothing it does is simulation state.
            } else {
                // The engine tick *returns* here: no second marker, no render, and — the part a port
                // forgets — none of step 7 either. This frame's clock never becomes the previous
                // frame's, so the next frame measures its delta from the frame before this one. The
                // game's tick throws the return value away, so the loop still goes round again.
                return Step {
                    serviced,
                    dt,
                    inv_dt: self.inv_dt,
                    ms_step,
                    simulated: true,
                    sim_halted: true,
                    frame: self.frame,
                    markers,
                    outcome: self.outcome(),
                };
            }
        }

        // 7. This frame's clocks become the previous frame's, and the deferred flag takes effect one
        //    frame after it was asked for. Both happen whether or not the frame simulated.
        self.prev_clock_ms = self.clock_ms;
        self.prev_sim_ms = self.sim_ms;
        if self.suspend_requested {
            self.suspended = true;
            self.suspend_requested = false;
        }

        Step {
            serviced,
            dt,
            inv_dt: self.inv_dt,
            ms_step,
            simulated,
            // The only way out of the frame with the simulation halted is the early return above.
            sim_halted: false,
            frame: self.frame,
            markers,
            outcome: self.outcome(),
        }
    }

    /// The quit flag, checked after the frame — which is why the frame it is raised on still runs.
    fn outcome(&self) -> Outcome {
        if self.quit {
            Outcome::Quit
        } else {
            Outcome::Continue
        }
    }

    /// The mode switch: service at most one request, then clear it.
    ///
    /// Separate from [`Engine::step_at`] because it is a separate function in the original, and because
    /// the guards are the part a port gets wrong: a guarded request does **not** block the frame, and a
    /// delayed one takes three frames (arm, count, act), not two.
    pub fn service(&mut self, game: &mut impl Game) -> Serviced {
        let Some(request) = self.mode.pending() else {
            return Serviced::Nothing;
        };
        if request.waits_for_ui() && game.ui_loading() {
            return Serviced::Waiting(request);
        }
        if request.waits_two_frames() {
            if !self.mode.armed {
                self.mode.armed = true;
                self.mode.delay = 0;
                // Request 4 raises the memory-card page as it arms; request 6 only arms. Both mark the
                // UI as mid-transition, which is [`Game::transition`]'s business, not this one's.
                return Serviced::Waiting(request);
            }
            if self.mode.delay < 1 {
                self.mode.delay += 1;
                return Serviced::Waiting(request);
            }
            self.mode.armed = false;
            self.mode.delay = 0;
        }
        game.transition(request);
        match request {
            // The restart chains: it clears nothing, it posts the world build for the next frame.
            Request::RestartEvent => self.mode.request = Request::EnterWorld.code(),
            Request::Shutdown => {
                game.shutdown();
                self.mode.request = 0;
            }
            _ => self.mode.request = 0,
        }
        Serviced::Performed(request)
    }
}

// ---------------------------------------------------------------------------------------------------
// The state hash. Kept next to the types rather than in `hash.rs`, so that one module describes one
// subsystem; `hash::State` is the only thing that has to be in step.
// ---------------------------------------------------------------------------------------------------

impl State for Engine {
    fn walk(&self, w: &mut Walk<'_>) {
        w.i32("clock_ms", self.clock_ms);
        w.i32("prev_clock_ms", self.prev_clock_ms);
        w.i32("sim_ms", self.sim_ms);
        w.i32("prev_sim_ms", self.prev_sim_ms);
        w.field("dt", &self.dt);
        w.f("dt_s", self.dt_s);
        w.f("inv_dt", self.inv_dt);
        w.u32("frame", self.frame);
        w.u32("phase", self.phase.code());
        w.field("input", &self.input);
        w.field("rng", &self.rng);
        w.field("mode", &self.mode);
        w.bool("quit", self.quit);
        w.bool("frozen", self.frozen);
        w.bool("suspended", self.suspended);
        w.bool("suspend_requested", self.suspend_requested);
        w.bool("cap_30", self.cap_30);
        // A missing time scale and a zero one are different states, and hash differently.
        w.bool("time_scale.on", self.time_scale.is_some());
        w.f("time_scale", self.time_scale.unwrap_or(F(0.0)));
    }
}

impl State for DeltaTime {
    fn walk(&self, w: &mut Walk<'_>) {
        w.group("ring", |w| {
            for entry in &self.ring {
                w.f("dt", *entry);
            }
        });
        w.u32("index", self.index);
        w.bool("primed", self.primed);
        w.f("raw", self.raw);
    }
}

impl State for Mailbox {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u32("request", self.request);
        w.bool("armed", self.armed);
        w.i32("delay", self.delay);
    }
}

impl Engine {
    /// The fixed update's whole state in 64 bits, for a per-frame comparison against a capture.
    pub fn hash(&self) -> u64 {
        hash::of(self)
    }
}

// ---------------------------------------------------------------------------------------------------
// The simulation, wired to the real frame.
// ---------------------------------------------------------------------------------------------------

/// The simulation itself, plugged into the fixed update: one [`Engine::step`] reads the pads and
/// rides the bike, with the frame length the clock measured.
///
/// This is the whole path from the frame to the ride, and it is the same [`crate::Sim`] that
/// `replay::verify` drives — the harness and a live loop differ in where the pad bytes come from and
/// in what is under the wheels, not in what is stepped. A shell with a track loaded implements
/// [`Game`] itself, on a value that holds one of these and a world; see `crates/openmx`.
///
/// The pads: the fixed update's own `input` is one sampled [`Input`], which is what a fixture
/// carries, so that is what this path rebuilds a pad report out of ([`crate::Sim::reports`]). A
/// shell that has the real 32-byte reports hands them over itself rather than through here.
impl Game for crate::Sim {
    fn simulate(&mut self, dt: F, input: Input, _rng: &mut Rng) -> bool {
        self.tick_at(dt, input);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clock is the one the hardware is programmed for: 57,600 counts is exactly 100 ms, so one
    /// whole tick plus half a period is 150 ms.
    #[test]
    fn the_clock_reads_timer_0_in_milliseconds() {
        assert_eq!(clock_ms(Timer0 { ticks: 0, count: 0 }), 0);
        assert_eq!(clock_ms(Timer0 { ticks: 1, count: 0 }), 100);
        assert_eq!(clock_ms(Timer0 { ticks: 1, count: 28_800 }), 150);
        assert_eq!(clock_ms(Timer0 { ticks: 1, count: 57_599 }), 199);
    }

    /// The first frame measures nothing: the engine primes both ends of the delta from the same sample.
    /// A port that starts with `prev = 0` and subtracts hands the first frame half a minute of dt.
    #[test]
    fn the_first_frame_has_no_delta() {
        let mut e = Engine::new();
        let mut sim = crate::Sim::new(1);
        let step = e.step_at(107_233, Input::default(), &mut sim);
        assert_eq!(step.dt, F(0.0));
        assert_eq!(step.inv_dt, F(1.0), "1/dt is 1.0, not an infinity");
        assert_eq!(step.frame, 1);
        assert_eq!(e.sim_ms, 107_233);
    }

    /// The mean of thirty copies of a number is **not** that number here: thirty chopped adds and a
    /// divide land a few ulps below it. It matters — the value below is what a subsystem integrating
    /// with `dt` actually gets, and a port that short-circuits "all thirty are equal" diverges.
    const MEAN_OF_THIRTY_33MS: f32 = 0.032_999_992;

    /// Thirty-three milliseconds, thirty times, is a mean just under 0.033 — and the 30 fps clamp does
    /// not touch it, because that is below 0.03333.
    #[test]
    fn the_mean_is_the_mean_until_the_clamp_bites() {
        let mut dt = DeltaTime::default();
        let (d, ms) = dt.update(33, true, None);
        assert_eq!(
            d,
            F(MEAN_OF_THIRTY_33MS),
            "a primed ring is thirty copies of the first delta, averaged"
        );
        assert_eq!(ms, 33, "the clamp did not fire, so the step is the measured one");
        for _ in 0..40 {
            let (d, ms) = dt.update(34, true, None);
            if d == F(DeltaTime::CAP_30) {
                assert_eq!(ms, DeltaTime::CAP_30_MS);
                return;
            }
            assert_eq!(ms, 34);
        }
        panic!("thirty-four millisecond frames must eventually push the mean past the clamp");
    }

    /// A frame long enough to be a level load is not averaged in: the ring is thrown away and refilled
    /// with the stand-in constant, so the next second of play is not slowed down by it.
    #[test]
    fn a_long_frame_resets_the_history() {
        let mut dt = DeltaTime::default();
        for _ in 0..30 {
            dt.update(16, false, None);
        }
        let (d, _) = dt.update(19_000, false, None);
        assert_eq!(dt.raw, F(DeltaTime::AFTER_LONG_FRAME));
        assert!(
            dt.ring.iter().all(|&e| e == F(DeltaTime::AFTER_LONG_FRAME)),
            "every entry was refilled, not just the newest"
        );
        assert_eq!(d, F(MEAN_OF_THIRTY_33MS), "which averages to just under it");
        assert!(dt.primed);
    }

    /// A guarded request does not stop the frame: it ticks, it simulates, and the request is still
    /// pending afterwards.
    #[test]
    fn a_guarded_request_still_runs_the_frame() {
        struct Loading(bool);
        impl Game for Loading {
            fn simulate(&mut self, _: F, _: Input, _: &mut Rng) -> bool {
                true
            }
            fn ui_loading(&self) -> bool {
                self.0
            }
        }
        let mut e = Engine::new();
        let mut g = Loading(true);
        e.mode.post(Request::WorldToMenu);
        let step = e.step_at(1_000, Input::default(), &mut g);
        assert_eq!(step.serviced, Serviced::Waiting(Request::WorldToMenu));
        assert!(step.simulated);
        assert_eq!(e.mode.request, 2, "still pending");

        g.0 = false;
        let step = e.step_at(1_033, Input::default(), &mut g);
        assert_eq!(step.serviced, Serviced::Performed(Request::WorldToMenu));
        assert_eq!(e.mode.request, 0);
    }
}
