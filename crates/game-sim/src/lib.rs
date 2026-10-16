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

use ps2_float::{F, Vf};

/// One frame of controller input. Replace with the game's own buttons; keep it `Copy` and small, because
/// a replay fixture is a list of these.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Hash)]
pub struct Input {
    pub buttons: u16,
    pub stick: (i8, i8),
}

/// The entire simulation state.
///
/// One value, no pointers, `Clone` for snapshots. When this grows past a screenful, split it into
/// per-subsystem structs that live here — not into globals.
#[derive(Clone, Debug)]
pub struct Sim {
    pub ticks: u64,
    pub rng: u32,
    pub ball: physics::Ball,
}

impl Sim {
    /// A fresh state from a seed. The seed is part of the state and part of a replay fixture: whatever
    /// the original seeded its RNG with (a frame counter, a button press pattern) comes in here, never
    /// from the clock.
    pub fn new(seed: u32) -> Sim {
        // a serve, so the example has something to integrate
        Sim {
            ticks: 0,
            rng: seed,
            ball: physics::Ball {
                vel: Vf([0.5, 3.0, 0.0, 0.0]),
                spin: F(1.0),
                ..Default::default()
            },
        }
    }

    /// Advance one frame. This is the function whose every detail has to match the original, including
    /// the *order* the subsystems run in — which is nearly always the order the original's main loop
    /// calls them, and nearly never the order that reads nicely.
    pub fn tick(&mut self, _input: Input) {
        self.ball.step();
        self.ticks += 1;
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

/// The camera for normal play: the preset record the disc ships one of per camera, what the camera
/// points at, the eight first-order lags that make it follow rather than snap, and the limits on its
/// angles, its distance and its field of view. Presentation, not simulation — the module's own docs
/// say how that was checked, and what it costs.
pub mod camera;

/// Collision against the world: the shape test the original sweeps a bundle of probe segments
/// through a collision mesh's tree with, how a hit resolves into a push-out, and the limits the
/// sweep allows itself. The question every wheel, bale and rider asks once a substep.
pub mod collide;

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
/// watched fields. A failure is one line — `tick 413: ball.vel[1] -58.94952 vs -58.949516` — not a diff of two
/// files. The module's own docs say how a fixture is made and where one lives.
pub mod replay;

/// The game's random numbers: the C library's 64-bit LCG that every subsystem calls, plus the two
/// shapes its call sites wrap it in (a float in `[0, 1)`, and a uniform index into a list). Everything
/// later that involves chance reads this sequence, so it is ported before anything that uses it.
pub mod rng;

/// The main actor's state machine: riding, down after a wreck, waiting, put back — the one axis the
/// rest of the actor hangs off, since a down rider's physics does not step at all. The reasons a rider
/// comes off, the two clocks that decide when it goes back on, the pad chord beside them, and which of
/// those transitions share a frame.
pub mod state;

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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(sim.hash(), 0x7bbf_5ca2_d10e_b072, "{:?}", sim.ball);
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
