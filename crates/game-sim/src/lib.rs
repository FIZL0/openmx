//! **This is the crate you fill in: the game's own logic, ported.**
//!
//! It starts nearly empty on purpose. What is here is the shape that has survived contact with a real
//! port — the rules that are annoying to adopt later and cheap to adopt now. Delete the examples as you
//! replace them; keep the rules.
//!
//! # The shape
//!
//! ```text
//! crates/game-sim/src/
//!   lib.rs        this: the state type, the tick, the hash
//!   physics.rs    one module per subsystem of the original, named after what it does
//!   player.rs     …not after the file it was found in, and never after an address
//!   rules.rs
//!   replay.rs     the acceptance test: same input, same hash
//! ```
//!
//! Five rules, each of which exists because breaking it costs a week:
//!
//! 1. **No `f32` arithmetic operators anywhere in here.** Every float goes through [`ps2_float`]: `F` for
//!    scalars, `Vf` for VU0 vectors. The PS2's FPU chops where IEEE rounds and saturates where IEEE makes
//!    an infinity, so `a + b * c` in plain Rust is *a different number* from the original's, usually in
//!    the last bit and occasionally not. Every module in this crate starts with
//!    `#![deny(clippy::float_arithmetic)]`, which turns "I forgot" into a build failure.
//! 2. **No `std` transcendentals.** `sinf`, `atan2f`, `powf` and friends come from [`ps2_float::libm`],
//!    which is the algorithm the PS2 SDK shipped. `f32::sin` is accurate to whatever the host's libm
//!    feels like, which differs between x86-64 and aarch64, between glibc versions, and between debug and
//!    release — and each difference is a test that passes on your machine and fails in CI.
//! 3. **No hidden state.** Everything the simulation reads lives in one state value (here [`Sim`]), so a
//!    snapshot is a copy and a replay is a loop. `static mut`, thread locals, lazily-initialised caches
//!    and "I'll just read the clock" all end the same way: a desync nobody can reproduce.
//! 4. **No rendering, no input polling, no files, no time.** This crate is a pure function of (state,
//!    input). The app layer draws it and feeds it. That is what lets a test run ten thousand ticks a
//!    second, and what lets the same code run in a browser or in a netplay rollback.
//! 5. **Every ported function is proved.** `ps2-oracle` calls the original over generated inputs and
//!    compares bit for bit; `replay` proves whole matches. "It looks right" is not a result.
//!
//! # The loop
//!
//! ```
//! use game_sim::{Sim, Input};
//! let mut sim = Sim::new(0x1234_5678);
//! for _ in 0..60 {
//!     sim.tick(Input::default());
//! }
//! assert_eq!(sim.ticks, 60);
//! ```
//!
//! [`Sim::hash`] is the whole state in 64 bits, identical on every platform (see
//! [`state_hash`]). It is the single most useful thing in a port: print it per tick and a divergence
//! from the original — or between two machines — has an exact frame number instead of a vague
//! "sometime in the second rally".

#![forbid(unsafe_code)]
// Float operators are denied per module (see each `mod` below) rather than crate-wide, so that the few
// places that legitimately need host arithmetic — a hash, a test's statistics — can opt out knowingly.

use std::fmt;
use std::sync::Arc;

use ps2_float::{F, Vf};

use crate::input::{AXIS_CENTRE, Manager, PADS, Report};
use crate::phy::Phy;
use crate::ride::Controls;
use crate::rider::{Rider, Tables};
use crate::trick::Set;
use crate::world::{Flat, World};

/// One frame of controller input. Replace with the game's own buttons; keep it `Copy` and small, because
/// a replay fixture is a list of these.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct Input {
    pub buttons: u16,
    /// The left stick's two bytes less the centre, right and down positive, modulo 256: full right
    /// or down (`0xff`, +128) is the one an `i8` cannot hold, and wraps to -128 (S94).
    pub stick: (i8, i8),
}

/// The entire simulation state: the bike, the person on it, and the pads they are holding.
///
/// **This is the value the harness drives and the value the shell drives.** `replay::verify` steps
/// one of these; `crates/openmx` holds one and steps it with the track under it instead of a flat
/// floor. There is no second simulation and no example state beside it: a measurement taken off the
/// shell is a measurement of the thing a fixture compares.
///
/// Three of its fields are the run's *parameters* rather than its state, and are not in the walk
/// ([`crate::hash`]): [`Sim::tables`], which is read-only arithmetic built once, and [`Sim::phy`],
/// which is the vehicle's numbers as the disc carries them. Two runs of different bikes diverge in
/// the first tick's `bike.pos` regardless, which is the failure a reader wants to see.
pub struct Sim {
    /// How many simulation passes this state has been through. Tick 0 is the state a run starts
    /// from.
    pub ticks: u64,
    /// What the run was seeded with. The original's generator is [`rng::Rng`] and the ride draws
    /// nothing from it (see [`tick`]'s own docs); this is the word a fixture carries, kept in the
    /// state so that two runs from two seeds differ from tick 0 even where nothing has drawn yet.
    pub rng: u32,
    /// The tables the ride and the trick decoder read: built once, never written, and shared by
    /// every clone of this state rather than rebuilt per snapshot. Thirty thousand entries of arc
    /// cosine is not something a per-tick snapshot should copy, and a shared *immutable* table is
    /// not the hidden mutable state rule 3 is about.
    pub tables: Arc<Tables>,
    /// The bike's own parameters, off its `.phy`: shared by every clone of this state like the
    /// tables, because nothing in a ride writes them ([`snapshot`]).
    pub phy: Arc<Phy>,
    /// The binding table the bike's commands are read through (P73, P79): [`bindings::Table::SHIPPED`]
    /// unless a player picked another controller preset ([`Sim::set_presets`]). The original builds
    /// it when a ride starts and when the Controller Presets screen is left, never during a frame, so
    /// it is a constant of the ride like the bike's numbers: copied with a snapshot, kept local by a
    /// resync, and not hashed. A plane ride reads preset 0 whatever this says ([`Sim::table`]).
    pub bindings: bindings::Table,
    /// The input layer, read once a frame *before* the simulation — which is where the original
    /// reads it ([`tick`]). It is state and not a parameter: the press and release timers and the
    /// one-frame event queue are what a gesture is decoded out of.
    pub input: Manager,
    /// The bike and the rider on it.
    pub rider: Rider,
    /// The biplane, when the ride is a plane's: with one here [`Sim::step`] flies it and leaves the
    /// bike where it stands, as the original's Switch To Machine does. `None` is a bike ride, which
    /// walks and hashes exactly as it did before the plane was a lane ([`flight`]).
    pub plane: Option<flight::Flight>,
    /// Players 2 to 8, in player order ([`Sim::add_player`]). Player 1 is [`Sim::rider`], on
    /// [`Sim::phy`], as it always was; an empty list is a one-player session, which steps, walks and
    /// hashes exactly as the simulation did before there could be a second ([`Sim::step_all`]).
    pub others: Vec<Player>,
    /// This player's session markers (P88): five saved spots and the d-pad's selection, stepped by
    /// [`markers::frame`] after the tick. Untouched, it is not walked or sent, so every state that
    /// never used them hashes and encodes as before.
    pub markers: markers::Markers,
}

/// The most riders a session holds: the original's start fills a field of up to eight.
pub const MAX_PLAYERS: usize = 8;

/// A player after the first: their own bike's numbers and the rider on it — controls, bike, tricks,
/// wreck and recovery all inside the [`Rider`].
#[derive(Clone, Debug)]
pub struct Player {
    /// The bike this player rides, off its `.phy`. A parameter of the run like [`Sim::phy`], left
    /// out of the walk for the same reason, and shared by every clone of the state the same way
    /// ([`snapshot`]).
    pub phy: Arc<Phy>,
    pub rider: Rider,
}

impl Sim {
    /// A fresh state from a seed, on the stand-in bike and a level floor ([`stock`]).
    ///
    /// A seed is not enough to describe a ride — the bike, its wheel mounts, the spawn and the
    /// track all come off the user's own disc — so what a bare seed builds is the bike this crate
    /// can build without one. A fixture that means to prove the port against a recording of the
    /// original carries a whole state in [`replay::Fixture::start`] instead.
    pub fn new(seed: u32) -> Sim {
        let tables = Arc::new(Tables::build());
        let phy = stock::phy(&tables.ride.curve);
        let mut sim = Sim::ride(seed, tables, phy, stock::SPAWN, stock::FACING, Set::default());
        // The wreck test off, which is the stand-in's own doing and not the original's: a bike
        // placed on a start line has no velocity and has not asked the ground anything yet, and
        // S10's airborne test answers `NoseDive` on the first frame of exactly that state
        // ([`stock`] has the reproducer). A down rider is not integrated, so with the test on, the
        // state a bare seed builds would stop being a ride on its first tick — and every fixture
        // the port records against itself starts here.
        sim.rider.sense.wrecks_enabled = false;
        sim
    }

    /// A state built around a bike somebody has read off a disc: the shell's constructor.
    ///
    /// `set` is the rider's own 24 stunts ([`trick::Stunts::selection`]); the wheel mounts are
    /// [`stock::mounts`]'s, because the model's own wheel nodes are the renderer's side of the disc.
    pub fn ride(
        seed: u32,
        tables: Arc<Tables>,
        phy: Phy,
        at: Vf,
        facing: Vf,
        set: Set,
    ) -> Sim {
        let mounts = stock::mounts(&phy);
        let rider = Rider::new(&phy, mounts, at, facing, set);
        Sim::with_rider(seed, tables, phy, rider)
    }

    /// A state around a rider somebody else built.
    ///
    /// The shell takes the wheel mounts off the bike *model's* own nodes rather than off the `.phy`
    /// (`crates/openmx/src/main.rs`, S22) and turns the wreck test on or off from its command line,
    /// so it builds the [`Rider`] itself and hands it over. Everything else about a state is the
    /// same whichever way the bike arrived.
    pub fn with_rider(seed: u32, tables: Arc<Tables>, phy: Phy, rider: Rider) -> Sim {
        let mut input = Manager::new();
        // Pad 0 plugged in and pad 1 not, which is the shape every capture of this game was taken
        // in and the shape a one-player session has.
        input.pad_mut(0).connected = true;
        Sim {
            ticks: 0,
            rng: seed,
            tables,
            phy: Arc::new(phy),
            bindings: bindings::Table::SHIPPED,
            input,
            rider,
            plane: None,
            others: Vec::new(),
            markers: markers::Markers::default(),
        }
    }

    /// **One frame of the simulation**, in the original's order: the pads are read, and then the
    /// world moves ([`tick`] is what calls this, and says why that order is not negotiable).
    ///
    /// The world is an argument rather than a field because it is the one thing in a ride that
    /// cannot be a snapshot: the shell's is a track's terrain and its collision meshes borrowed out
    /// of the disc's archives, and a fixture's is a flat floor. Everything else a frame touches is
    /// in `self`.
    pub fn frame(
        &mut self,
        dt: F,
        reports: &[Report; PADS],
        world: &mut dyn World,
    ) -> crate::rider::Frame {
        let controls = self.poll(dt, reports);
        self.step(dt, &controls, world)
    }

    /// The pad half of a frame: the device manager's update, and the rider's hands off it.
    ///
    /// Split out of [`Sim::frame`] for the shell, which reads the pad on every frame and rides on
    /// only some of them (its pause is a page over a running frame in the original, and a stopped
    /// ride in a shell with no pages).
    pub fn poll(&mut self, dt: F, reports: &[Report; PADS]) -> Controls {
        self.input.tick(dt, reports);
        Controls::through(self.input.pad(0), self.table())
    }

    /// Build the binding table on each pad's controller preset, pad 0 first, as the original's builder
    /// does when a ride starts or the Controller Presets screen is left (P79): the preset's eight
    /// commands on that pad, the fixed ones on both. Preset 0 on both is [`bindings::Table::SHIPPED`].
    pub fn set_presets(&mut self, presets: [usize; PADS]) {
        self.bindings = bindings::Table::built(presets.map(|p| p.min(bindings::PRESETS - 1)));
    }

    /// The table the pads are read through this frame: [`Sim::bindings`] on the bike, and preset 0 in
    /// a plane — on any vehicle but the bike the original builds preset 0 whatever the profile says.
    pub fn table(&self) -> &bindings::Table {
        if self.plane.is_some() { &bindings::Table::SHIPPED } else { &self.bindings }
    }

    /// The simulation half: one frame of the rider, and the tick counter behind it.
    ///
    /// With other players in the session they are stepped too, after player 1, each on what their
    /// own pad holds ([`Sim::step_all`]; a player with no pad of their own holds nothing). A caller
    /// that has every player's controls — online play, a fixture of several — calls `step_all`.
    pub fn step(&mut self, dt: F, controls: &Controls, world: &mut dyn World) -> crate::rider::Frame {
        if !self.others.is_empty() && self.plane.is_none() {
            let mut all = self.pad_controls();
            all[0] = *controls;
            return self.step_all(dt, &all, world).swap_remove(0);
        }
        if let Some(plane) = &mut self.plane {
            // The plane's input step: the player's pad, through I6's mapping, into the five fields the
            // controls read. The bike's `controls` are not the plane's: the plane reads the pad itself.
            plane.frame(dt, flight::pad(self.input.pad(0)), world, &self.tables.ride);
            self.ticks += 1;
            return crate::rider::Frame::default();
        }
        let said = self.rider.frame(&self.phy, &self.tables, controls, dt, world);
        self.ticks += 1;
        said
    }

    /// How many riders the session holds: player 1 and the others.
    pub fn players(&self) -> usize {
        1 + self.others.len()
    }

    /// Put another player in the session, at the end of the order: the original spawns its humans
    /// in player order and every list a frame is walked in appends at the tail, so the player added
    /// last is stepped last. Answers the new player's index (1 for player 2).
    ///
    /// Pads are not touched: plugging a second controller in is the caller's, through
    /// [`Sim::input`], because a player may as well be somebody on the other end of a connection.
    pub fn add_player(&mut self, phy: Phy, rider: Rider) -> usize {
        assert!(self.players() < MAX_PLAYERS, "a session holds at most {MAX_PLAYERS} riders");
        self.others.push(Player { phy: Arc::new(phy), rider });
        self.others.len()
    }

    /// Player `i`'s rider: 0 is [`Sim::rider`].
    pub fn rider_of(&self, i: usize) -> &Rider {
        if i == 0 { &self.rider } else { &self.others[i - 1].rider }
    }

    /// [`Sim::poll`] for every player: the device manager's update, then each player's hands off
    /// their own pad — player `i` on pad `i`, and neutral controls for a player past the pads there
    /// are ([`input::PADS`]).
    pub fn poll_all(&mut self, dt: F, reports: &[Report; PADS]) -> Vec<Controls> {
        self.input.tick(dt, reports);
        self.pad_controls()
    }

    fn pad_controls(&self) -> Vec<Controls> {
        (0..self.players())
            .map(|i| if i < PADS { Controls::through(self.input.pad(i), self.table()) } else { Controls::default() })
            .collect()
    }

    /// **One frame of every rider**, in player order, and the tick counter behind them: player 1,
    /// then player 2, and so on, each one's whole frame before the next one's starts — the order the
    /// original's update walk visits them in (`research/oracle.txt`, S54). `controls[i]` is player
    /// `i`'s.
    ///
    /// Each rider's frame sees every other rider where they stand *now*: the ones before it in the
    /// order already moved this frame, the ones after it not yet, which is what a sequential walk
    /// of the scene gives the original. That is the one way riders reach one another — their head
    /// ray striking another's body ([`ride::hits_among`]), which is [`air::Wreck::Joust`] when the
    /// one struck is high enough above.
    ///
    /// With one player this is [`Sim::step`] and nothing else, so a one-player hash does not move. A
    /// plane ride is one player's: with a plane up, only it is stepped, as before.
    pub fn step_all(&mut self, dt: F, controls: &[Controls], world: &mut dyn World) -> Vec<crate::rider::Frame> {
        assert_eq!(controls.len(), self.players(), "one set of controls per player");
        if self.others.is_empty() || self.plane.is_some() {
            return vec![self.step(dt, &controls[0], world)];
        }
        let mut frames = Vec::with_capacity(controls.len());
        for (i, c) in controls.iter().enumerate() {
            let rivals: Vec<ride::Rival> = (0..self.players())
                .filter(|&j| j != i)
                .map(|j| self.rider_of(j))
                .filter(|r| r.state.wreck.is_none())
                // A ghost is nobody's rival: its bodies ignore the vehicle class, and the pair
                // filter reads both bodies' lists (S63, `ghost`).
                .filter(|r| !r.ghost.on)
                .map(|r| r.ride.rival())
                .collect();
            let tables = &self.tables;
            let frame = if i == 0 {
                self.rider.frame_among(&self.phy, tables, c, dt, world, &rivals)
            } else {
                let p = &mut self.others[i - 1];
                p.rider.frame_among(&p.phy, tables, c, dt, world, &rivals)
            };
            frames.push(frame);
        }
        self.ticks += 1;
        frames
    }

    /// Advance one frame from a fixture's own input, over a flat floor.
    ///
    /// What a [`replay::Fixture`] can express, and so what [`replay::verify`] drives: an
    /// [`Input`] is a button mask and a stick, which is rebuilt into the pad report the game would
    /// have read ([`Sim::reports`]). The step is the 30 Hz clamp, because that is the step every
    /// capture of the original in this project was taken at — a fixed-step port cannot replay a run
    /// the original measured frame by frame, and `tests/replay.rs` refuses a capture that is not at
    /// the clamp.
    pub fn tick(&mut self, input: Input) {
        self.tick_at(F(tick::DeltaTime::CAP_30), input);
    }

    /// [`Sim::tick`] with the frame length the caller measured: what [`tick::Engine`] hands its
    /// game, and so the path a live loop with no track loaded takes.
    pub fn tick_at(&mut self, dt: F, input: Input) {
        self.tick_frame(dt, &Sim::reports(input));
    }

    /// One frame over a flat floor from the pad reports the platform layer actually read: what a
    /// [`replay::PadTape`] carries, which an [`Input`] cannot (the twelve pressures, and the frame
    /// length the engine measured).
    pub fn tick_frame(&mut self, dt: F, reports: &[Report; PADS]) -> crate::rider::Frame {
        self.frame(dt, reports, &mut Flat::default())
    }

    /// [`Sim::tick`] over a world the caller owns: the fixed 30 Hz step and a fixture's own
    /// [`Input`], but the ground under the wheels is whatever was handed in.
    ///
    /// This is the step [`replay::verify_in`] drives, and the difference between replaying a
    /// *recording* and replaying a ride: a recording of the original is a minute over a track at
    /// some height, and over [`Flat`]'s floor at y = 0 the same input free-falls instead (S28).
    pub fn tick_in(&mut self, input: Input, world: &mut dyn World) -> crate::rider::Frame {
        self.frame(F(tick::DeltaTime::CAP_30), &Sim::reports(input), world)
    }

    /// A fixture's [`Input`] as the pad reports the game would have read.
    ///
    /// Lossy in the direction that matters and exact in the direction a fixture is used: a digital
    /// press is full pressure, so a session played on buttons rebuilds byte for byte, and a
    /// throttle held *half* way cannot be expressed by a mask and does not survive. A recording
    /// that needs the strengths is a [`replay::PadTape`] and not a list of [`Input`]s.
    pub fn reports(input: Input) -> [Report; PADS] {
        let mut report = Report::from_buttons(input.buttons);
        // Modulo 256, as [`Input::stick`] holds it: -128 is `0xff`, not a second `0x00`.
        let byte = |v: i8| AXIS_CENTRE.wrapping_add(v as u8);
        report.set_axis(2, byte(input.stick.0));
        report.set_axis(3, byte(input.stick.1));
        let mut reports = [Report::missing(); PADS];
        reports[0] = report;
        reports
    }

    /// Where the bike is: the chassis transform's origin, which is the row 3 of the world transform
    /// a capture of the original records.
    pub fn bike_pos(&self) -> Vf {
        self.rider.ride.position()
    }

    /// What the bike is doing: the body's velocity, which is the quad beside that transform.
    pub fn bike_vel(&self) -> Vf {
        self.rider.ride.chassis.vel
    }

    /// The state in 64 bits. Add a field to [`hash`]'s walk the moment you add it to the state, or a
    /// replay test will pass over a divergence it cannot see.
    ///
    /// One line, because the walk that produces it lives in [`hash`] and is shared with the dump mode
    /// that says *which* field differed.
    pub fn hash(&self) -> u64 {
        hash::of(self)
    }
}

impl Clone for Sim {
    /// A snapshot. The tables and the bike's parameters are shared rather than copied — they are
    /// immutable — so this is a copy of the bike, the rider and the pads and nothing else.
    fn clone(&self) -> Sim {
        Sim {
            ticks: self.ticks,
            rng: self.rng,
            tables: Arc::clone(&self.tables),
            phy: Arc::clone(&self.phy),
            bindings: self.bindings,
            input: self.input,
            rider: self.rider.clone(),
            plane: self.plane.clone(),
            others: self.others.clone(),
            markers: self.markers.clone(),
        }
    }
}

impl fmt::Debug for Sim {
    /// The state a human wants in a failure message: which tick, and where the bike is. The whole
    /// state is [`hash::Dump`], which prints it field by field; the tables have no `Debug` of their
    /// own and thirty thousand entries of one would not help anybody.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sim")
            .field("ticks", &self.ticks)
            .field("rng", &self.rng)
            .field("bike.pos", &self.bike_pos())
            .field("bike.vel", &self.bike_vel())
            .field("phase", &self.rider.phase())
            .finish_non_exhaustive()
    }
}

/// FNV-1a over bytes, seeded with the running value: integer-only, so the answer is the same on x86-64,
/// aarch64 and wasm32, which is the entire point.
///
/// Floats are hashed as their *bit patterns*, never as values: `0.0 == -0.0` is true, and a port that has
/// drifted to `-0.0` has drifted. Equally, never hash a `f64`, a `HashMap`'s iteration order, or a
/// pointer; all three vary between runs of the same binary.
pub fn state_hash(bytes: &[u8], seed: u64) -> u64 {
    let mut h = if seed == 0 {
        0xcbf2_9ce4_8422_2325
    } else {
        seed
    };
    for &b in bytes {
        h ^= b as u64;
        // 2⁴⁰ + 2⁸ + 0xb3, FNV-64's prime, written in full so a missing digit cannot hide in it. The
        // standard constant matters beyond pedantry: a capture tool or a script outside this crate can
        // reproduce a state hash from any FNV-1a implementation, and `tests/hash.rs` checks it against
        // the published vectors.
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// An example subsystem, and the only thing in this crate that does any arithmetic. Written the way a
/// ported function should be: PS2 floats, the original's operation order, and a comment saying where it
/// came from in words — never an address or a decompiler's name for it.
pub mod physics {
    #![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

    use super::*;

    /// Gravity, in units per frame squared, as the original's data table holds it.
    const GRAVITY: f32 = -0.15;
    /// How much of its spin a ball keeps each frame.
    const SPIN_DECAY: f32 = 0.98;

    /// A thing that flies. Two vectors and nothing else; the original's object is bigger, and the rest of
    /// it belongs to whichever subsystem actually reads it.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Ball {
        pub pos: Vf,
        pub vel: Vf,
        pub spin: F,
    }

    impl Ball {
        /// One frame of free flight: `vel += gravity`, then `pos += vel`.
        ///
        /// Note the order and the lack of a `dt`: a PS2 game's integration step is one frame, and
        /// "improving" this to `pos += vel * dt` changes every trajectory in the game.
        pub fn step(&mut self) {
            // Vectors through `Vf` (VU0's adder), scalars through `F` (the EE FPU's). They are not the
            // same model — see `ps2_float` — so which one a value goes through is part of the port.
            self.vel = self.vel.add(Vf([0.0, GRAVITY, 0.0, 0.0]));
            self.pos = self.pos.add(self.vel);
            self.spin *= SPIN_DECAY;
        }

        /// Hashed as bit patterns, in a fixed order — by the subsystem's own walk in [`super::hash`],
        /// so that there is one description of this struct's state and not two to keep in step.
        pub fn hash(&self, seed: u64) -> u64 {
            crate::hash::of_with_seed(self, seed)
        }
    }
}

/// Actor lifetime: the two linked lists a level's live objects hang in, how the world build puts an
/// actor in them and takes it out again, and the order everything afterwards walks them in — which is
/// the order random draws and collision tie-breaks resolve in. Also the interned name that an actor's
/// "kind" actually is.
pub mod actor;

/// The bike in the air: where the sim decides the wheels have left the ground, what the stick may still
/// do up there, the preload and backflip carried off the lip, and the test a landing has to pass for the
/// rider to stay on. Half of how riding around feels is in here.
pub mod air;

/// The disc's two archive formats: the one container holding the game, and the compressed containers
/// holding each track, bike and menu. Pure parsing over byte slices — see the module's own docs for why
/// it hands back offsets instead of reading files.
pub mod archive;

/// The biplane: the pad into its control surfaces through the rate limits, the pitch-rate autopilot,
/// the engine's thrust and the aerodynamic coefficients into the body-frame force and moment; and its
/// crash: the test that decides it has hit something, the wreck it flies on in, and the reset that puts
/// it back at its spawn.
pub mod biplane;

/// The camera for normal play: the preset record the disc ships one of per camera, what the camera
/// points at, the eight first-order lags that make it follow rather than snap, and the limits on its
/// angles, its distance and its field of view. Presentation, not simulation — the module's own docs
/// say how that was checked, and what it costs.
pub mod camera;

/// The chassis under the bike: the rigid body the ground step pushes on. The step that turns a
/// substep's force and torque into motion — two integrations, the limits and the freedom flags that
/// pin them, the transform brought along — plus the per-frame speed measure, the transform's
/// rotate/unrotate pair, the two-step integrator itself and the contact distributor that spreads a
/// frame's collision contacts onto the two wheels.
pub mod chassis;

/// Collision against the world: the shape test the original sweeps a bundle of probe segments
/// through a collision mesh's tree with, how a hit resolves into a push-out, and the limits the
/// sweep allows itself. The question every wheel, bale and rider asks once a substep.
pub mod collide;

/// The sweep probe's narrow phase: its segments stood in the world and wrapped in a sphere, then swept
/// through a mesh or a hierarchy's meshes in their own space and taken back out — what a collision
/// object's sweep back finds.
pub mod sweep_probe;

/// A dynamics body's collision object resolving its contacts every substep: the relative velocity at
/// each contact, the damped spring or the impulse along the normal, the friction across it, and the
/// damping a contact lowers. What makes the biplane stop at what it hits.
pub mod contact;

/// A collision object's contact search, once a frame before the world steps: the sweep back out of
/// what the body flew into, the rays against the terrain's cells, and the rays against the world —
/// the list [`contact`] resolves, and the flags the crash test reads.
pub mod contact_search;

/// The nearby search a collision object's probes make: the pair filter, the candidates within a
/// probe's reach handed to its narrow phase, placed groups part by part, and who hears about a hit.
pub mod world_search;
/// A ray probe's narrow phase: its tree of segments against a mesh's tree of triangles, one contact
/// per segment, against a mesh or a placed group part by part.
pub mod ray_probe;

/// The broad phase under it: the world's collidable tree and moving-object list, the query that walks
/// them, the gather that fills a nearby search from them, and a probe tested straight against the
/// world when it keeps no nearby search.
pub mod broad_phase;

/// How the world fills it: the static tree laid over a track's field and a collidable keyed and put in
/// (S96), which decides the order a gather hands candidates out in.
pub mod broad_build;

/// The worked example from `docs/EXAMPLE.md`: three functions of the synthetic program that `./kit demo`
/// generates, ported, with `tests/demo.rs` proving them bit-exact against the original's instructions.
/// Delete it once your own port has something real in it.
pub mod demo;

/// The state hash: one walk over the whole simulation state, hashed into 64 bits for a per-tick
/// comparison, or dumped field by field to say which field first differed. Every port after this one is
/// checked with it.
pub mod hash;

/// The matrix and quaternion helpers everything with a bone or a camera is built out of: the 4x4
/// product, the point transform, the look-at frame, a quaternion to a matrix and back, the angular
/// velocity derivative, and the spherical interpolation a blended bone goes through. All of them in the
/// original's arithmetic and its instruction order, because that order is what fixes the last bit.
pub mod math;

/// Movement on the ground: the step that turns the throttle, the brake and the rider's lean into forces
/// on the chassis — the centre of gravity moving with the stick, the steering fading out with speed, the
/// gearbox and the clutch's launch ramp, the drag that sets the top speed — and, because every one of
/// them reads what the last wrote, the order the whole ground frame runs in.
pub mod r#move;

/// The input layer: the pad read once a frame, before the simulation, and mapped into the game's own
/// bitmask. Sixteen controls per pad with "held", their press and release timers, and the one-frame
/// event queue the front end reacts to — the original keeps no "pressed this frame" mask, so the module
/// says exactly which frame latches what. A pure function of (state, pad report): nothing here touches
/// the hardware.
pub mod input;

/// The original's binding table (P73): which of a pad's sixteen controls each of the game's commands
/// is on, built from a per-player preset, and the two ways the game reads a command through it.
pub mod bindings;

/// Animation driving: which motion plays for the state the rider is in, how far into it the frame is,
/// and how one motion blends into the next. The tail of the rider update that the trick clocks stop
/// short of: the selection per action id, the cross-fade a restarted motion starts with, the clip's own
/// dt advance beside the phase the trick's progress drives it at, and the inclusive window a frame
/// fires its action frames from. Timing, not looks: the pose the airborne blend lands on is the one the
/// landing test judges.
pub mod motion;

/// The bike's own numbers, off the disc: the `.phy` file every vehicle's physics is written in, the
/// reader the engine reads it with, and the whole key-to-parameter map the loader walks — which is
/// where the ground step's and the air step's parameters come from, instead of from a test.
pub mod phy;

/// The acceptance test for the whole port: a fixture of recorded inputs plus the state it starts from,
/// replayed through the simulation and compared tick by tick against the recording's hashes or its
/// watched fields. A failure is one line — `tick 413: bike.vel[1] -58.94952 vs -58.949516` — not a diff of two
/// files. The module's own docs say how a fixture is made and where one lives.
pub mod replay;

/// The whole bike, assembled: the four views of the chassis and the two of each wheel lined up into
/// one state, the tuning the `.phy` reader leaves, and one frame of the ground step driven in the
/// order `move.rs` drives it with the engine's own callees dispatched where the original calls them.
/// This is the module a shell rides; everything it does is somebody else's proved port, bar the
/// handful of engine helpers nobody has read yet, which it marks as its own.
pub mod ride;

/// A rider on the bike: the trick machine, the airborne wreck test and the ride state machine wired
/// onto `ride` in the order the original's ridden actor runs them, so that one value is a person who
/// can gesture for a stunt, hold it, land it and come off. The joining is all this module is — every
/// decision inside it belongs to one of the four ported modules it calls.
pub mod rider;

/// The rider's *own* animation state machine: the function in front of `vehicle_anim`'s step, which
/// is what decides the state it is handed. The gate's get-ready sequence, the wreck family, the
/// reverse pair and the look-around idle — the clips in a rider's own motion file that nothing the
/// bike asks for could ever reach. Also the body actor's identity: the actor `vehicle_anim` ports the
/// step of is the rider's body, and this is the half of it that chooses.
pub mod rider_anim;

/// The rider's ragdoll: the eighteen bones and seventeen joints the rider is thrown off the bike as,
/// their records for riding and for the wreck, and the two entries that switch between them.
pub mod ragdoll;

/// The ragdoll's nine bone collisions: each bone's sphere pushed out of the ground and the scene,
/// with an impulse at the contact that turns the bone as well as moving it.
pub mod ragdoll_contact;
/// Where the ragdoll's bones start from, and the reset that puts them back there.
pub mod ragdoll_pose;
/// The ragdoll run in the ride while the rider is down (S53).
pub mod ragdoll_ride;
/// The rider body actor's whole tick round the ragdoll's step: the mode machine and the settle (S61).
pub mod ragdoll_body;

/// The game's random numbers: the C library's 64-bit LCG that every subsystem calls, plus the two
/// shapes its call sites wrap it in (a float in `[0, 1)`, and a uniform index into a list). Everything
/// later that involves chance reads this sequence, so it is ported before anything that uses it.
pub mod rng;

/// The whole ride saved, put back and sent to another machine: what online play's rollback copies
/// every tick and a resync carries. The constants stay shared and out of the bytes.
pub mod snapshot;

/// The main actor's state machine: riding, down after a wreck, waiting, put back — the one axis the
/// rest of the actor hangs off, since a down rider's physics does not step at all. The reasons a rider
/// comes off, the two clocks that decide when it goes back on, the pad chord beside them, and which of
/// those transitions share a frame.
pub mod state;

/// The bike and the floor a simulation rides when nobody has handed it a disc: the stand-in `.phy`
/// a bare [`Sim::new`] builds its ride out of, and where it stands it. Hand-written, and not the
/// disc's — the module's own head says why one has to exist at all.
pub mod stock;

/// The fixed update: one frame of the game as the original runs it — one mode request serviced, the
/// wall clock sampled and averaged into `dt`, the pads read, and then the simulation. The top-level
/// step everything else in this crate hangs off, and the place to look for what runs before what.
pub mod tick;

/// One track as the disc stores it: the scene that lists what is on it, the collision shapes the
/// simulation tests against, and the terrain the wheels run on. Pure parsing over borrowed bytes — the
/// app layer reads them out of the archives and hands them over.
pub mod track;

/// Tricks: the table of stunts the disc ships, the stick-and-button gesture that names one, the clocks
/// that decide whether it counts, and the trick's own half of what makes a landing a crash.
pub mod trick;

/// The wheel, in the order the ground step drives it: the suspension ray that looks for the ground
/// under the wheel and picks between its answer and the collision sweep's, the tire forces that come
/// out of the contact it settled on, and the wheel's own spin and steering. The ten fields the ground
/// step empties every substep and keeps by offset get their names here.
pub mod wheel;

/// The tire's grip: the frame at the contact patch the tire pushes along, and the per-surface force
/// law that says how hard it pushes across and along it.
pub mod tire;

/// The vehicle's own animation state machine, which is what the animation library was written for:
/// the sixty-entry switch on the bike's animation state, its own clip table, the two lags the rows are
/// mixed along, and the two entry points of that library — the four-clip blend and a sixth arm — that
/// nothing else in the game reaches. `motion` is the rider's half of the same library; this is the
/// bike's, and the bigger of the two.
pub mod vehicle_anim;

/// The biplane's moving parts and its pilot: the plane's request block, which turns a rider's body
/// into a pilot leaning with the stick, and the flaps, rudder and prop posed from the controls. Here
/// rather than in alphabetical order, beside the step it drives.
pub mod biplane_anim;

/// The world outside the bike, as the ride asks about it: where the ground is under a point, and the
/// scene's collision meshes stood where the scene puts them, with the sweep against the lot.
pub mod world;

/// The biplane as a ride: the state a plane session carries between ticks, built off `biplane.phy`,
/// and the frame [`Sim::step`] runs while the plane has the controller.
pub mod flight;

/// The biplane's world search (S48): its collision object's ray and sweep probes against the
/// track's collision meshes.
pub mod flight_search;

/// The engine's shared smooth noise: 1-D gradient noise over tables the game builds off its own
/// `rand()` on the first sample, which the plane's engine note and the rider's balance wobble read.
pub mod noise;

/// The wreck sound's pick: which of the rider's wreck sounds plays, and the one draw from the
/// generator it costs.
pub mod wreck_sound;

/// The wreck bike (S69): `Wreckbike.phy`'s body and collision object, and the world node's tick that
/// moves them.
pub mod wreck_bike;

/// The bike's ghost after a put-back: the fade back in to solid, and the other riders passing
/// through it until it has.
pub mod ghost;

/// Session markers (P88): five spots a player saves on the d-pad and rides back to, and the other
/// players to ride to.
pub mod markers;

/// Game modifiers (P55): percentages on the loaded files' numbers, set before a ride starts, and
/// nothing at all at their defaults.
pub mod modifiers;

/// Off the map: the bike's cannonball, the throw back toward the middle of a map a rider has left.
pub mod off_map;

/// The wreck page: the viewport fade a wreck's reset asks for, and the put-back it hands back three
/// ticks later.
pub mod wreck_page;

/// The points (S79): what a trick, a wheelie, a stoppie and a backflip score, the combo they add up
/// in, and the rider's total it is banked into — or lost from, on a wreck.
pub mod score;

#[cfg(test)]
mod tests {
    use super::*;

    /// A hundred ticks of the state a bare seed builds: the stand-in bike dropped a foot onto a
    /// level floor with nobody touching the pad ([`the_stand_in_bike_is_dropped_onto_the_floor`]
    /// says what that is).
    ///
    /// The same literal as `tests/hash.rs`' own, and it moves whenever the state type or the ride
    /// does. What it is here for is the *platform*: see the test below it.
    const HUNDRED_TICKS: u64 = 0x09a0_2520_1f3f_518b;

    /// The hash is a constant of the code, not of the machine: this value must be the same on x86-64,
    /// aarch64 and wasm32. When a change to the state type changes it, that is expected — update it. When
    /// a *platform* changes it, something in here is using host arithmetic, and that is the bug this test
    /// exists to catch.
    #[test]
    fn a_hundred_ticks_hash_the_same_everywhere() {
        let mut sim = Sim::new(1);
        for _ in 0..100 {
            sim.tick(Input::default());
        }
        assert_eq!(sim.hash(), HUNDRED_TICKS, "{sim:?}");
    }

    /// What a bare seed's state actually does, written down because every fixture the port records
    /// against itself starts here ([`stock`]): the bike is dropped a foot onto a level floor, the
    /// wheels take the landing and it settles there with nobody on the pad — a ride, not a wreck.
    #[test]
    fn the_stand_in_bike_is_dropped_onto_the_floor() {
        let mut sim = Sim::new(1);
        assert_eq!(sim.bike_pos(), stock::SPAWN, "a fresh state is on the start line");
        assert_eq!(sim.bike_vel(), Vf::ZERO, "with nothing moving");

        let mut wrecked = None;
        for k in 0..100u64 {
            let said = sim.tick_frame(F(tick::DeltaTime::CAP_30), &Sim::reports(Input::default()));
            if said.wreck.is_some() && wrecked.is_none() {
                wrecked = Some((k, said.wreck));
            }
        }
        assert_eq!(wrecked, None, "the stand-in's wreck test is off: {sim:?}");
        assert_eq!(sim.ticks, 100);
        assert_eq!(sim.rider.phase(), crate::state::Phase::Riding, "{sim:?}");
        // And it has come to rest on the floor it was dropped above, on both wheels, under its own
        // suspension: three hundred more ticks do not move it by a bit. (The origin rests a little
        // *below* the floor plane because it is the chassis' own reference point and not the lowest
        // part of the bike — what is on the floor is the wheels.)
        let (front, rear) = (&sim.rider.ride.front, &sim.rider.ride.rear);
        assert!(front.mv.contact && rear.mv.contact, "both wheels on the floor: {sim:?}");
        let rest = format!("{:.3}", sim.bike_pos().y());
        for _ in 0..300 {
            sim.tick(Input::default());
        }
        let now = format!("{:.3}", sim.bike_pos().y());
        assert_eq!(now, rest, "it stays where it settled: {sim:?}");
    }

    #[test]
    fn a_replay_reports_the_first_frame_that_differs() {
        let mut sim = Sim::new(7);
        let inputs = vec![Input::default(); 10];
        let hashes: Vec<u64> = inputs
            .iter()
            .map(|&i| {
                sim.tick(i);
                sim.hash()
            })
            .collect();
        let good = replay::Fixture {
            seed: 7,
            inputs: inputs.clone(),
            hashes: hashes.clone(),
            ..Default::default()
        };
        assert!(replay::verify(&good).is_ok());

        let mut bad = good.clone();
        bad.hashes[4] ^= 1;
        let d = replay::verify(&bad).expect_err("a changed hash must be noticed");
        assert_eq!(d.tick, 5, "the fifth tick is the one that differs");
    }
}
