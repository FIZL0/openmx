//! The acceptance test for the whole port: same inputs from the same state, same simulation, tick by
//! tick.
//!
//! A **fixture** is one recorded session: where the run starts, the input of every frame, and what the
//! state was expected to be while it ran. [`verify`] replays it and stops at the **first** tick that
//! disagrees, because the first disagreement is the bug and everything after it is noise. What comes
//! back is one line, with a tick number and two values:
//!
//! ```text
//! tick 413: bike.vel[1] -58.94952 (0xc26bcc4f) vs -58.949516 (0xc26bcc4e)
//! ```
//!
//! not a diff of two files. That is the whole design goal of this module: a failure a human reads in
//! one glance, and a tick number to re-run to.
//!
//! # The two things a fixture can expect
//!
//! Both come from the same recording and both are checked on every tick they cover:
//!
//! - **Hashes** — [`Fixture::hashes`], one [`Sim::hash`] per frame. Cheap (no allocation, one FNV round
//!   per state byte), covers *every* field of the state, and says nothing about *which* field moved.
//!   This is what a port's own recorded run holds, and what makes the fixture a cross-platform test for
//!   free: the hash is integer-only, so x86-64, aarch64 and wasm32 must produce the same list.
//! - **Watch values** — [`Fixture::watches`], a named field of the state with the value it had on the
//!   ticks it was sampled. These are what a *capture of the original* gives you (`./kit record` writes
//!   one CSV column per field; [`Capture`] turns those columns into watches), and they are the ones
//!   that name the field in the failure line. A fixture usually has a handful of watches and a hash per
//!   frame: the hash notices everything, the watches explain the ticks they cover.
//!
//! Where a tick has both, the watches are compared first — a named field beats `state hash a vs b`.
//!
//! # Tick numbering
//!
//! Tick 0 is the state the fixture starts from, before any input. Tick *k* is the state after the
//! *k*-th input frame has been simulated, so a fixture with 600 inputs is checked at ticks 0..=600, and
//! `hashes[k - 1]` is the hash expected at tick *k*. Numbering is relative to the start of the fixture,
//! not to [`Sim::ticks`], so a fixture that starts from a mid-session state still reads `tick 1` for its
//! first frame.
//!
//! This is also where frame alignment goes wrong, every time, in every project: a capture samples the
//! original's state at *some* point in its frame, and if that point is not where the port's tick ends,
//! the whole run is off by one and every value differs by exactly one frame of motion. [`Watch::shifted`]
//! is the knob for that, and `tests/replay.rs` has a test showing what an off-by-one looks like
//! (everything differs from tick 1, by one frame of gravity) so it is recognised rather than
//! investigated.
//!
//! # Exact, with no tolerance, on purpose
//!
//! There is no `--tol` here. Simulation state is bit-exact in this project (see `AGENT.md`), floats are
//! compared as bit patterns, and a replay harness that accepts "close enough" quietly stops being a
//! proof the day the port drifts in the last bit — which is the day it matters. For the rendering-ish
//! numbers that legitimately need a tolerance there is `./kit record --diff --tol`, which is not a
//! simulation proof and does not pretend to be. This module therefore does no float arithmetic at all,
//! and denies the lint that would let some in.
//!
//! # Making one
//!
//! Two sources, and the difference between them matters:
//!
//! - [`record`] runs the port and writes down what it did. That is a **regression** fixture: it proves
//!   the port still does what it did yesterday, which is worth having on every `./kit check`, and
//!   proves nothing whatever about the original.
//! - [`Capture::parse`] reads a `./kit record` CSV of the **original** and turns chosen columns into
//!   watches. That is an **acceptance** fixture, and the only kind that proves the port is right.
//! - [`P2m2::parse`] reads a human's PCSX2 input recording (`.p2m2`) and [`P2m2::tape`] turns it into
//!   the pad the game was handed, frame by frame. That is the input half of an acceptance fixture with
//!   no capture session in it at all — somebody rode for a minute, and the recording is replayable on
//!   the original whenever the port changes.
//!
//! # The world under it
//!
//! A fixture is the *rider's* half of a session: a start state, the input, and what the state was. The
//! other half is the ground it was ridden over, and that cannot be in a fixture — the world is a
//! track's terrain and its collision meshes borrowed out of the user's own disc ([`crate::world`]), not
//! a value a file can carry. So a fixture *names* it ([`Fixture::world`], [`Where`]) and the caller
//! hands the thing itself to [`verify_in`]. [`verify`] is the flat-floor replay, which is right for a
//! fixture the port recorded against itself and wrong for a recording of the original: the acceptance
//! run spent S8 to S28 diverging at tick 1 because the original's bike was riding Free09 at a ground
//! height of 208 and the replay was free-falling towards [`crate::world::Flat`]'s floor at y = 0. A
//! fixture that names a world and is replayed by `verify` is refused for that reason
//! ([`Reason::WrongWorld`]) rather than quietly dropped 208 units.
//!
//! A fixture is a text file ([`Fixture::parse`], [`Fixture::to_text`]) so a recorded session can be
//! stored and replayed later. Those files hold values read out of the game, so they are game data:
//! they live in the git-ignored `context/`, never in the repository — see `tests/replay.rs` for how a
//! test finds them and what it does when they are absent.

#![deny(clippy::float_arithmetic)] // a comparison does no arithmetic on the values it compares

use core::fmt;

use ps2_float::F;

use crate::hash::{self, Dump, Leaf};
use crate::input::{self, Manager, PADS, Report};
use crate::{Input, Sim};

/// The version written into a fixture file's `format` line, bumped when the text format changes in a
/// way an older reader would misread. A file with no `format` line is read as version 1.
pub const FORMAT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------------------------------

/// One recorded session: where it starts, what was pressed, and what the state was.
#[derive(Clone, Debug, Default)]
pub struct Fixture {
    /// What this fixture is, for the failure line and for a human reading the directory. Free text
    /// without newlines; a slug like `attract-lap1` is ideal.
    pub name: String,
    /// What the original was seeded with. The start state when [`Fixture::start`] is `None`.
    pub seed: u32,
    /// The exact state the run starts from, when a seed is not enough — a session that begins from a
    /// save state in the middle of a level. Takes precedence over [`Fixture::seed`].
    ///
    /// Not expressible in the text format yet (the state type has no reader): a fixture that needs one
    /// is built in code, from whatever reads the original's RAM image.
    pub start: Option<Sim>,
    /// One entry per frame, in order. The length of this list is the length of the run.
    pub inputs: Vec<Input>,
    /// The expected [`Sim::hash`] per frame: `hashes[k - 1]` is checked at tick *k*. May be shorter
    /// than [`Fixture::inputs`] (the ticks past its end are then only checked by watches), and empty
    /// means "watches only", which is how an acceptance fixture starts life — the original's hash
    /// cannot be captured, only the port's.
    pub hashes: Vec<u64>,
    /// Named fields of the state with the values they were sampled at. Any number, any ticks.
    pub watches: Vec<Watch>,
    /// **Which world the recording was ridden on**, when it was ridden on one: the disc's own names
    /// for the track and the area, resolved into a [`crate::world::World`] by whoever has the disc
    /// open and handed to [`verify_in`].
    ///
    /// `None` is a fixture with no ground of its own — every fixture the port records against itself,
    /// which rides [`crate::world::Flat`] because what is being replayed there is the rider and not
    /// the track. A fixture that names one cannot be replayed by [`verify`] at all: see the module's
    /// own note on the world, and [`Reason::WrongWorld`].
    ///
    /// Not expressible in the text format yet, for the same reason [`Fixture::start`] is not.
    pub world: Option<Where>,
}

/// Which world a session was ridden on, in the disc's own spelling: a track's container and one area
/// inside it.
///
/// Two names rather than one because a track archive can hold more than one scene, and the scene is
/// what carries the terrain's layout, the props and the start line — the area, in the game's own
/// vocabulary. Names and not file contents: a fixture is data a test can hold, and a world is
/// megabytes of somebody's disc.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Where {
    /// The track's container as `MX.PSR` spells it: `Tracks\free09.res`.
    pub track: String,
    /// The area's scene member inside that container, as the archive spells it:
    /// `tracks\free09\free09.scn`.
    pub area: String,
}

impl fmt::Display for Where {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(w, "{} ({})", self.area, self.track)
    }
}

impl Fixture {
    /// The state this fixture starts from: [`Fixture::start`] if it has one, else [`Sim::new`].
    pub fn initial(&self) -> Sim {
        self.start.clone().unwrap_or_else(|| Sim::new(self.seed))
    }

    /// The last tick the run reaches, which is the number of input frames.
    pub fn last_tick(&self) -> u64 {
        self.inputs.len() as u64
    }

    /// How many values this fixture actually checks — the number in the failure message's denominator,
    /// and the number to look at with suspicion when a new fixture passes first time.
    pub fn checks(&self) -> usize {
        self.hashes.len() + self.watches.iter().map(|w| w.samples.len()).sum::<usize>()
    }
}

/// One field of the state, and what it held on the ticks it was sampled.
///
/// A watch is one column of a capture: `./kit record`'s `bike_xform.m31` becomes the port's
/// `bike.pos[1]`,
/// whose name is a path through the state's walk (see [`crate::hash`]). Samples need not be dense —
/// a capture with `every = 4`, or with rows gated by a `when` condition, is sparse and that is fine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    /// The path of the field in the state's walk, exactly as a [`Dump`] prints it: `bike.vel[1]`,
    /// `players[2].bike.spin`.
    pub path: String,
    /// `(tick, expected value)`, in ascending tick order. [`verify`] walks them with a cursor, so a
    /// sample out of order is not compared — and is reported at the end of the run rather than
    /// silently skipped.
    pub samples: Vec<Sample>,
}

impl Watch {
    /// A watch of `path` from `(tick, value)` pairs.
    pub fn new(path: impl Into<String>, samples: Vec<Sample>) -> Watch {
        Watch { path: path.into(), samples }
    }

    /// The same watch with every sample moved `by` ticks, for a capture whose sample point is not where
    /// the port's tick ends. `by = 1` means "what the capture called frame 0 is the port's tick 1".
    ///
    /// Samples that would land before tick 0 are dropped: shifting a capture forward cannot invent a
    /// state the run never had.
    pub fn shifted(mut self, by: i64) -> Watch {
        self.samples.retain(|s| (s.tick as i64).saturating_add(by) >= 0);
        for s in &mut self.samples {
            s.tick = (s.tick as i64 + by) as u64;
        }
        self
    }
}

/// One recorded value, and the tick it belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub tick: u64,
    pub value: Value,
}

/// A recorded value, in the shapes a capture or a fixture file can hold.
///
/// Deliberately not [`Leaf`]: a leaf carries the *width* of its integer, which the recording and the
/// port need not agree on (a capture reads a flag as `u32`, the port's field is a `u8`) and which is
/// not a divergence. Floats, by contrast, are compared as raw bit patterns, because `0.0` and `-0.0`
/// are different states and a port that has drifted in the last bit has drifted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    /// An unsigned integer of any width.
    Uint(u64),
    /// A signed integer of any width.
    Int(i64),
    /// A single-precision float, as its raw pattern.
    Float(u32),
    /// A flag. Matches a `bool` field and an integer field holding 0 or 1.
    Bool(bool),
    /// A run of bytes, as its length and the digest [`crate::hash::Walk::blob`] would give it.
    Blob { len: usize, digest: u64 },
}

impl Value {
    /// How this value prints and compares: as the equivalent [`Leaf`], so a recorded value and a
    /// walked field are shown to the reader in exactly the same form.
    pub fn as_leaf(self) -> Leaf {
        match self {
            Value::Uint(v) => Leaf::Uint { value: v, width: 8 },
            Value::Int(v) => Leaf::Int { value: v, width: 8 },
            Value::Float(bits) => Leaf::Float(bits),
            Value::Bool(b) => Leaf::Bool(b),
            Value::Blob { len, digest } => Leaf::Blob { len, digest },
        }
    }

    /// The recorded value this field of the port holds, for turning a run into a fixture.
    pub fn of_leaf(leaf: Leaf) -> Value {
        match leaf {
            Leaf::Uint { value, .. } => Value::Uint(value),
            Leaf::Int { value, .. } => Value::Int(value),
            Leaf::Float(bits) => Value::Float(bits),
            Leaf::Bool(b) => Value::Bool(b),
            Leaf::Blob { len, digest } => Value::Blob { len, digest },
        }
    }

    /// Compare against a field of the port's state.
    ///
    /// Integer widths and signedness are allowed to differ (the recording's choice of type is the
    /// *recorder's*, not the game's); kinds are not. A float recorded where the port has an integer is
    /// [`Verdict::WrongKind`] rather than a difference, because it means the watch is pointed at the
    /// wrong field and the values are not worth reading.
    pub fn compare(self, got: Leaf) -> Verdict {
        let same = match (self, got) {
            (Value::Uint(a), Leaf::Uint { value: b, .. }) => a == b,
            (Value::Uint(a), Leaf::Int { value: b, .. }) => i128::from(a) == i128::from(b),
            (Value::Int(a), Leaf::Int { value: b, .. }) => a == b,
            (Value::Int(a), Leaf::Uint { value: b, .. }) => i128::from(a) == i128::from(b),
            (Value::Float(a), Leaf::Float(b)) => a == b,
            (Value::Bool(a), Leaf::Bool(b)) => a == b,
            // A flag read as an integer, which is how a capture reads one.
            (Value::Bool(a), Leaf::Uint { value: b, .. }) => u64::from(a) == b,
            (Value::Uint(a), Leaf::Bool(b)) => a == u64::from(b),
            (
                Value::Blob { len: al, digest: ad },
                Leaf::Blob { len: bl, digest: bd },
            ) => al == bl && ad == bd,
            _ => return Verdict::WrongKind,
        };
        if same { Verdict::Same } else { Verdict::Different }
    }
}

impl fmt::Display for Value {
    /// As the matching [`Leaf`] prints: a float as both its decimal and its pattern, so the line says
    /// what a human compares *and* what actually differed.
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(w, "{}", self.as_leaf())
    }
}

/// The result of comparing one recorded value against one field of the port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Same,
    Different,
    /// The two are not the same kind of thing: the watch is on the wrong field.
    WrongKind,
}

// ---------------------------------------------------------------------------------------------------
// Replaying one
// ---------------------------------------------------------------------------------------------------

/// Why a replay stopped: one tick, one reason, one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// The tick the fixture and the port stopped agreeing on, numbered as the module describes.
    pub tick: u64,
    pub reason: Reason,
}

/// What disagreed. Three of these are the port being wrong; two are the fixture being wrong, which is
/// worth telling apart — a fixture recorded by an older build is not a regression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    /// A watched field holds a different value. The ordinary failure, and the informative one.
    Field { path: String, want: Value, got: Leaf },
    /// The state hash differs and no watch covers this tick, so the harness can say that the state
    /// moved but not which part of it. Add a watch, or compare the two states with
    /// [`crate::hash::first_difference`] — [`state_at`] re-runs the fixture to this tick for that.
    Hash { want: u64, got: u64 },
    /// The fixture watches a field the state does not have. The fixture is older than the port's walk,
    /// or the path is a typo — not a divergence in the simulation.
    NoSuchField { path: String },
    /// The fixture records a float where the port has an integer, or similar: the watch is on the
    /// wrong field. Also the fixture's problem, not the port's.
    WrongKind { path: String, want: Value, got: Leaf },
    /// The run ended before something the fixture expected was checked: samples past the last input
    /// frame, samples out of tick order, or more hashes than there are frames. A fixture that quietly
    /// checks less than it claims is the one failure mode a passing test cannot show you.
    Unreached { what: String, tick: u64 },
    /// The fixture was ridden on a named world ([`Fixture::world`]) and the replay was handed a flat
    /// floor. The caller's problem, not the port's, and reported at tick 0 because none of the run
    /// means anything: a recording of a ride over a track at a height of 208 replayed over
    /// [`crate::world::Flat`] is a bike in free fall from the first frame. Call [`verify_in`] with the
    /// world the fixture names.
    WrongWorld { want: Where },
}

impl fmt::Display for Divergence {
    /// `tick 413: bike.vel[1] -58.94952 (0xc26bcc4f) vs -58.949516 (0xc26bcc4e)` — expected first (the
    /// recording), the port's value second, the whole failure on one line.
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(w, "tick {}: ", self.tick)?;
        match &self.reason {
            Reason::Field { path, want, got } => write!(w, "{path} {want} vs {got}"),
            Reason::Hash { want, got } => {
                write!(w, "state hash 0x{want:016x} vs 0x{got:016x}")
            }
            Reason::NoSuchField { path } => {
                write!(w, "the state has no field '{path}' (the fixture is older than the walk)")
            }
            Reason::WrongKind { path, want, got } => {
                write!(w, "{path} is the wrong kind of field: recorded {want}, state holds {got}")
            }
            Reason::Unreached { what, tick } => {
                write!(w, "the run ended with {what} at tick {tick} unchecked")
            }
            Reason::WrongWorld { want } => write!(
                w,
                "this recording was ridden on {want} and the replay was given a flat floor — \
                 `verify_in` takes the world, `verify` is the flat one",
            ),
        }
    }
}

impl std::error::Error for Divergence {}

/// **Replay a fixture over a flat floor.** Returns the final state, or the first tick that disagreed.
///
/// Every tick of the fixture is checked: the watches that cover it (which name their field when they
/// differ) and then its hash. A tick with neither is simulated and not examined — which is normal,
/// since a capture of the original cannot produce a hash.
///
/// Cost: one [`Dump`] on the ticks that have watch samples, nothing but the hash on the rest, so a
/// fixture with four watched fields and a hash per frame runs at a few hundred thousand ticks a second.
///
/// A fixture that names the world it was ridden on ([`Fixture::world`]) is **refused** here with
/// [`Reason::WrongWorld`] — replay it with [`verify_in`] and the world it names.
pub fn verify(f: &Fixture) -> Result<Sim, Divergence> {
    if let Some(want) = &f.world {
        return Err(Divergence {
            tick: 0,
            reason: Reason::WrongWorld { want: want.clone() },
        });
    }
    verify_in(f, &mut crate::world::Flat::default())
}

/// **Replay a fixture over a world.** [`verify`]'s own body, with the ground the recording was ridden
/// over instead of a level floor at y = 0.
///
/// The world is an argument and not part of the fixture because it is the one thing in a ride that
/// cannot be a snapshot ([`Sim::frame`] says the same): the ride asks it where the surface under each
/// wheel is and what the sweep hits, and the answers come out of a track's terrain and its collision
/// meshes, borrowed from the user's own disc. The fixture carries the *name* of it
/// ([`Fixture::world`]) so that a reader with the disc open can produce the right one, and so that a
/// recording of the original cannot be replayed over the wrong ground by accident.
///
/// It takes `&mut dyn World` rather than a generic because the two worlds in this project are not
/// interchangeable in size — a flat floor is two fields and a track is a heightfield and three hundred
/// collision meshes — and because the ride's own frame already takes it that way.
pub fn verify_in(f: &Fixture, world: &mut dyn crate::world::World) -> Result<Sim, Divergence> {
    let mut sim = f.initial();
    // One cursor per watch into its samples, advanced as the run passes them. Keeps the per-tick cost
    // proportional to the watches, not to the length of the recording.
    let mut cursor = vec![0usize; f.watches.len()];

    check_watches(f, &mut cursor, 0, &sim)?;
    for (k, &input) in f.inputs.iter().enumerate() {
        sim.tick_in(input, world);
        let tick = k as u64 + 1;
        check_watches(f, &mut cursor, tick, &sim)?;
        if let Some(&want) = f.hashes.get(k) {
            let got = hash::of(&sim);
            if got != want {
                return Err(Divergence { tick, reason: Reason::Hash { want, got } });
            }
        }
    }

    // Anything the fixture expected and the run never reached. A truncated input list, a sample out of
    // tick order, or a hash list longer than the run: all of them make a fixture check fewer values
    // than it says it does, and all of them pass silently without this.
    for (w, &at) in f.watches.iter().zip(&cursor) {
        if let Some(s) = w.samples.get(at) {
            return Err(Divergence {
                tick: f.last_tick(),
                reason: Reason::Unreached { what: w.path.clone(), tick: s.tick },
            });
        }
    }
    if f.hashes.len() > f.inputs.len() {
        return Err(Divergence {
            tick: f.last_tick(),
            reason: Reason::Unreached {
                what: "the state hash".to_string(),
                tick: f.hashes.len() as u64,
            },
        });
    }
    Ok(sim)
}

/// The watches that have a sample at `tick`, compared against the state. Dumps the state at most once,
/// and only when there is something to compare.
fn check_watches(
    f: &Fixture,
    cursor: &mut [usize],
    tick: u64,
    sim: &Sim,
) -> Result<(), Divergence> {
    if !f
        .watches
        .iter()
        .zip(cursor.iter())
        .any(|(w, &at)| w.samples.get(at).is_some_and(|s| s.tick == tick))
    {
        return Ok(());
    }
    let dump = Dump::of(sim);
    for (w, at) in f.watches.iter().zip(cursor.iter_mut()) {
        while let Some(s) = w.samples.get(*at) {
            if s.tick != tick {
                break;
            }
            *at += 1;
            let Some(got) = dump.get(&w.path) else {
                return Err(Divergence {
                    tick,
                    reason: Reason::NoSuchField { path: w.path.clone() },
                });
            };
            match s.value.compare(got) {
                Verdict::Same => {}
                Verdict::Different => {
                    return Err(Divergence {
                        tick,
                        reason: Reason::Field { path: w.path.clone(), want: s.value, got },
                    });
                }
                Verdict::WrongKind => {
                    return Err(Divergence {
                        tick,
                        reason: Reason::WrongKind { path: w.path.clone(), want: s.value, got },
                    });
                }
            }
        }
    }
    Ok(())
}

/// The port's state at `tick` of a fixture, with nothing checked: what to call when [`verify`] reports
/// a bare [`Reason::Hash`] and the next question is *which field*.
///
/// ```no_run
/// # use game_sim::{hash, replay::{self, Fixture}};
/// # let fixture = Fixture::default();
/// # let reference = game_sim::Sim::new(0);
/// if let Err(d) = replay::verify(&fixture) {
///     let mine = replay::state_at(&fixture, d.tick);
///     // `reference` being the state the original was in at that tick, however it was obtained
///     println!("tick {}: {:?}", d.tick, hash::first_difference(&reference, &mine));
/// }
/// ```
pub fn state_at(f: &Fixture, tick: u64) -> Sim {
    state_at_in(f, tick, &mut crate::world::Flat::default())
}

/// [`state_at`] over the world the fixture was ridden on, which is the one that answers the question
/// for a recording of the original: the same re-run [`verify_in`] does, stopped at `tick` and
/// checking nothing.
pub fn state_at_in(f: &Fixture, tick: u64, world: &mut dyn crate::world::World) -> Sim {
    let mut sim = f.initial();
    for &input in f.inputs.iter().take(tick as usize) {
        sim.tick_in(input, world);
    }
    sim
}

/// Run the port over `inputs` and write down what it did: a hash per frame, plus a sample of every
/// path in `paths` on every tick from 0 to the end.
///
/// **This is a regression fixture, not a proof.** It says the port still behaves as it did when the
/// fixture was made, which catches every accidental change to the simulation and is worth running on
/// every `./kit check`. It says nothing at all about whether the port matches the original; for that
/// the expected values have to come from the original ([`Capture`]).
///
/// Fails with [`Reason::NoSuchField`] if a path is not in the state's walk, rather than recording a
/// watch with nothing in it — a fixture that checks a typo would pass forever.
pub fn record(
    name: impl Into<String>,
    seed: u32,
    inputs: &[Input],
    paths: &[&str],
) -> Result<Fixture, Reason> {
    record_at(name, seed, None, inputs, paths)
}

/// [`record`] from a state somebody else built, rather than from a seed.
///
/// What a recording of the *original* starts from is a bike in the middle of a session — a save state,
/// or an acceptance capture's own first row — and that state is not [`Sim::new`]'s spawn. A regression
/// fixture of such a run has to start where the run started, or its first tick is a teleport; so the
/// state goes in and comes back out in [`Fixture::start`], which is what [`verify`] replays from.
///
/// The fixture's [`Fixture::seed`] is the state's own [`Sim::rng`], so the two halves of a fixture
/// cannot disagree about what the run was seeded with.
pub fn record_from(
    name: impl Into<String>,
    start: Sim,
    inputs: &[Input],
    paths: &[&str],
) -> Result<Fixture, Reason> {
    let seed = start.rng;
    record_at(name, seed, Some(start), inputs, paths)
}

fn record_at(
    name: impl Into<String>,
    seed: u32,
    start: Option<Sim>,
    inputs: &[Input],
    paths: &[&str],
) -> Result<Fixture, Reason> {
    let mut sim = start.as_ref().map_or_else(|| Sim::new(seed), Sim::clone);
    {
        // A path that is not in the walk is a typo, and a watch of a typo would pass for ever.
        let at_start = Dump::of(&sim);
        for p in paths {
            if at_start.get(p).is_none() {
                return Err(Reason::NoSuchField { path: (*p).to_string() });
            }
        }
    }
    let mut watches: Vec<Watch> = paths.iter().map(|p| Watch::new(*p, Vec::new())).collect();
    let mut hashes = Vec::with_capacity(inputs.len());

    let sample = |sim: &Sim, tick: u64, watches: &mut Vec<Watch>| {
        if watches.is_empty() {
            return;
        }
        let dump = Dump::of(sim);
        for w in watches.iter_mut() {
            if let Some(leaf) = dump.get(&w.path) {
                w.samples.push(Sample { tick, value: Value::of_leaf(leaf) });
            }
        }
    };

    sample(&sim, 0, &mut watches);
    for (k, &input) in inputs.iter().enumerate() {
        sim.tick(input);
        hashes.push(hash::of(&sim));
        sample(&sim, k as u64 + 1, &mut watches);
    }

    Ok(Fixture {
        name: name.into(),
        seed,
        start,
        inputs: inputs.to_vec(),
        hashes,
        watches,
        // The port riding its own flat floor, which is what the loop above stepped: a regression
        // fixture has no track in it and must not claim one.
        world: None,
    })
}

// ---------------------------------------------------------------------------------------------------
// The fixture file
// ---------------------------------------------------------------------------------------------------

/// Where a fixture or a capture could not be read, with the line it was on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based, so it matches what an editor shows.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(w, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError { line, message: message.into() })
}

/// A number as the kit writes them everywhere: `0x…` is hex, anything else is decimal.
fn parse_u64(s: &str) -> Option<u64> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

fn parse_i64(s: &str) -> Option<i64> {
    if let Some(rest) = s.strip_prefix('-') {
        parse_u64(rest).and_then(|v| i64::try_from(v).ok()).map(|v| -v)
    } else {
        parse_u64(s).and_then(|v| i64::try_from(v).ok())
    }
}

impl Fixture {
    /// Read a fixture file. The format, which is lines of words and nothing cleverer:
    ///
    /// ```text
    /// # game-sim replay fixture
    /// format 1
    /// fixture attract-lap1
    /// seed 0x1234abcd
    /// frame 0x0000 0 0 0x7bbf5ca2d10eb072     # buttons, stick x, stick y, [state hash]
    /// frame 0x0010 -64 0 0x1d02c5e9b0b8a1ff
    /// watch 413 bike.vel[1] float 0x3fc00000  # tick, path, kind, value
    /// watch 413 ticks uint 413
    /// ```
    ///
    /// `frame` lines are the run, in order. Their hash is optional, but either every frame has one or
    /// none does: a fixture whose hashes stop halfway through, with nothing saying so, is a fixture
    /// that stops checking halfway through. `watch` lines may come in any order and be anywhere in the
    /// file; they are sorted by tick on the way in. Kinds are `float`, `uint`, `int`, `bool` and
    /// `blob`, and a float may be written as a pattern (`0x3fc00000`, exact) or as a decimal (`1.5`,
    /// which is read to the nearest float — fine for a hand-written fixture, not for a recorded one).
    ///
    /// Unknown keywords are errors, with the line number: a typo in a fixture must never silently
    /// check less than it looks like it does.
    pub fn parse(text: &str) -> Result<Fixture, ParseError> {
        let mut f = Fixture::default();
        let mut hashes_seen = 0usize;
        let mut watches: Vec<Watch> = Vec::new();

        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut word = line.split_whitespace();
            let keyword = word.next().unwrap_or("");
            match keyword {
                "format" => {
                    let v = word.next().and_then(parse_u64);
                    match v {
                        Some(v) if v as u32 == FORMAT_VERSION => {}
                        Some(v) => {
                            return err(n, format!("fixture format {v}, this build reads {FORMAT_VERSION}"));
                        }
                        None => return err(n, "format wants a version number"),
                    }
                }
                "fixture" => f.name = line["fixture".len()..].trim().to_string(),
                "seed" => match word.next().and_then(parse_u64) {
                    Some(v) if v <= u64::from(u32::MAX) => f.seed = v as u32,
                    _ => return err(n, "seed wants a 32-bit number"),
                },
                "frame" => {
                    let fields: Vec<&str> = word.collect();
                    if fields.len() < 3 || fields.len() > 4 {
                        return err(n, "frame wants: <buttons> <stick x> <stick y> [hash]");
                    }
                    let Some(buttons) = parse_u64(fields[0]).filter(|v| *v <= u64::from(u16::MAX))
                    else {
                        return err(n, format!("frame buttons '{}' is not a 16-bit number", fields[0]));
                    };
                    let (Some(x), Some(y)) = (parse_i8(fields[1]), parse_i8(fields[2])) else {
                        return err(n, "frame stick wants two numbers in -128..=127");
                    };
                    f.inputs.push(Input { buttons: buttons as u16, stick: (x, y) });
                    if let Some(h) = fields.get(3) {
                        let Some(h) = parse_u64(h) else {
                            return err(n, format!("frame hash '{h}' is not a number"));
                        };
                        if hashes_seen != f.inputs.len() - 1 {
                            return err(n, "this frame has a hash but an earlier one does not");
                        }
                        f.hashes.push(h);
                        hashes_seen += 1;
                    } else if hashes_seen != 0 {
                        return err(n, "this frame has no hash but an earlier one does");
                    }
                }
                "watch" => {
                    let fields: Vec<&str> = word.collect();
                    if fields.len() != 4 {
                        return err(n, "watch wants: <tick> <path> <kind> <value>");
                    }
                    let Some(tick) = parse_u64(fields[0]) else {
                        return err(n, format!("watch tick '{}' is not a number", fields[0]));
                    };
                    let value = parse_value(fields[2], fields[3])
                        .ok_or_else(|| ParseError {
                            line: n,
                            message: format!("'{} {}' is not a value", fields[2], fields[3]),
                        })?;
                    let path = fields[1];
                    match watches.iter_mut().find(|w| w.path == path) {
                        Some(w) => w.samples.push(Sample { tick, value }),
                        None => watches.push(Watch::new(path, vec![Sample { tick, value }])),
                    }
                }
                other => return err(n, format!("unknown keyword '{other}'")),
            }
        }

        for w in &mut watches {
            w.samples.sort_by_key(|s| s.tick);
        }
        f.watches = watches;
        Ok(f)
    }

    /// Write the fixture out in the format [`Fixture::parse`] reads. Floats go out as patterns, so the
    /// round trip is exact.
    pub fn to_text(&self) -> String {
        use fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "# game-sim replay fixture: {} frames, {} checks", self.inputs.len(), self.checks());
        let _ = writeln!(s, "format {FORMAT_VERSION}");
        if !self.name.is_empty() {
            let _ = writeln!(s, "fixture {}", self.name);
        }
        let _ = writeln!(s, "seed 0x{:08x}", self.seed);
        for (k, i) in self.inputs.iter().enumerate() {
            let _ = write!(s, "frame 0x{:04x} {} {}", i.buttons, i.stick.0, i.stick.1);
            match self.hashes.get(k) {
                Some(h) => {
                    let _ = writeln!(s, " 0x{h:016x}");
                }
                None => {
                    let _ = writeln!(s);
                }
            }
        }
        for w in &self.watches {
            for sample in &w.samples {
                let _ = writeln!(s, "watch {} {} {}", sample.tick, w.path, write_value(sample.value));
            }
        }
        s
    }
}

fn parse_i8(s: &str) -> Option<i8> {
    parse_i64(s).and_then(|v| i8::try_from(v).ok())
}

fn parse_value(kind: &str, text: &str) -> Option<Value> {
    match kind {
        "uint" => parse_u64(text).map(Value::Uint),
        "int" => parse_i64(text).map(Value::Int),
        "bool" => match text {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        "float" => parse_f32(text).map(Value::Float),
        // `<len>:<digest>`, the two numbers a blob leaf holds.
        "blob" => {
            let (len, digest) = text.split_once(':')?;
            Some(Value::Blob {
                len: parse_u64(len)? as usize,
                digest: parse_u64(digest)?,
            })
        }
        _ => None,
    }
}

/// A float as a bit pattern (`0x3fc00000`, exact) or as text (`1.5`, `-inf`, `nan` — read to the
/// nearest float by the standard library's parser, which rounds correctly and does no arithmetic).
fn parse_f32(text: &str) -> Option<u32> {
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16).ok();
    }
    text.parse::<f32>().ok().map(f32::to_bits)
}

fn write_value(v: Value) -> String {
    match v {
        Value::Uint(x) => format!("uint {x}"),
        Value::Int(x) => format!("int {x}"),
        Value::Float(bits) => format!("float 0x{bits:08x}"),
        Value::Bool(b) => format!("bool {b}"),
        Value::Blob { len, digest } => format!("blob {len}:0x{digest:016x}"),
    }
}

// ---------------------------------------------------------------------------------------------------
// A capture of the original
// ---------------------------------------------------------------------------------------------------

/// A `./kit record` CSV, read as rows of text: the bridge from a capture of the original to the
/// watches of a fixture.
///
/// The recorder writes a header row of column names, then one row per sampled frame whose first column
/// is the game's own frame counter. Columns are named after the watch spec's fields, with a suffix per
/// lane (`bike_vel.x`), so turning a capture into a fixture is a list of "this column is that field of
/// my state":
///
/// ```
/// # use game_sim::replay::Capture;
/// let csv = "frame,bike_vel.x,bike_vel.y,laps\n\
///            101234,0.5,3,0\n\
///            101235,1,5.85,0\n";
/// let cap = Capture::parse(csv).expect("the recorder's own format");
/// let watches = vec![
///     cap.float("bike_vel.y", "bike.vel[1]").unwrap(),
///     cap.uint("laps", "ticks").unwrap(),
/// ];
/// assert_eq!(watches[0].samples.len(), 2);
/// ```
///
/// Cells are kept as text and converted per column, on purpose: the recorder writes a float with a
/// `repr()` that round-trips every bit, an integer as decimal, a `fmt = "hex"` column as `0x…`, and a
/// `bytes:N` field as a hex string — and which of those a column is, only the person who wrote the
/// watch spec knows. Guessing from the text would read `3` as an integer and quietly turn a bit-exact
/// float comparison into nothing.
///
/// Two things from `watch/README.md` that matter here:
///
/// - **An empty cell is "not sampled", not zero.** A pointer chain that went null leaves the cell
///   blank and the row is still written; those samples are skipped rather than compared against a
///   state the original did not have.
/// - **A `scale` or `fmt` column is not bit-exact** and must not be used for a simulation proof. This
///   reader cannot tell — the scaling happened before the CSV was written — so that one is on the
///   person choosing the columns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capture {
    /// The header row, including the leading `frame`.
    pub columns: Vec<String>,
    /// The game's frame counter per row, in the order the rows were written.
    pub frames: Vec<u64>,
    /// One row of cells per entry in [`Capture::frames`], parallel to [`Capture::columns`] (including
    /// the `frame` column, so an index into one indexes the other).
    pub rows: Vec<Vec<String>>,
}

impl Capture {
    /// Read a recorder CSV. Rows with the wrong number of cells are an error with their line number: a
    /// capture that is half a row short is not evidence, and `CAPTURE FAILED` means exactly this.
    pub fn parse(text: &str) -> Result<Capture, ParseError> {
        let mut lines = text.lines().enumerate();
        let header = loop {
            match lines.next() {
                Some((_, l)) if l.trim().is_empty() => continue,
                Some((i, l)) => break (i + 1, l),
                None => return err(1, "the capture is empty"),
            }
        };
        let columns: Vec<String> = header.1.split(',').map(|c| c.trim().to_string()).collect();
        if columns.first().map(String::as_str) != Some("frame") {
            return err(header.0, "a capture's first column must be 'frame'");
        }

        let mut cap = Capture { columns, frames: Vec::new(), rows: Vec::new() };
        for (i, line) in lines {
            let n = i + 1;
            if line.trim().is_empty() {
                continue;
            }
            let cells: Vec<String> = line.split(',').map(|c| c.trim().to_string()).collect();
            if cells.len() != cap.columns.len() {
                return err(
                    n,
                    format!("{} cells for {} columns", cells.len(), cap.columns.len()),
                );
            }
            let Some(frame) = parse_u64(&cells[0]) else {
                return err(n, format!("frame '{}' is not a number", cells[0]));
            };
            cap.frames.push(frame);
            cap.rows.push(cells);
        }
        if cap.frames.is_empty() {
            return err(header.0, "the capture has a header and no rows");
        }
        Ok(cap)
    }

    /// The tick a row belongs to: its frame counter less the first row's, so the first row of a
    /// capture is tick 0 — the state the run starts from. Use [`Watch::shifted`] when the capture's
    /// sample point does not line up with the end of the port's tick.
    pub fn tick_of(&self, row: usize) -> u64 {
        self.frames[row].saturating_sub(self.frames[0])
    }

    /// How many frames the capture spans, counting the gaps a sparse capture leaves (`every = N`, a
    /// `when` condition, a dropped frame). Not the number of rows.
    pub fn span(&self) -> u64 {
        self.tick_of(self.frames.len() - 1)
    }

    /// A column as a watch of float bit patterns.
    pub fn float(&self, column: &str, path: &str) -> Result<Watch, ParseError> {
        self.column(column, path, "float", |s| parse_f32(s).map(Value::Float))
    }
    /// A column as a watch of unsigned integers (decimal, or `0x…` from a `fmt = "hex"` column).
    pub fn uint(&self, column: &str, path: &str) -> Result<Watch, ParseError> {
        self.column(column, path, "unsigned", |s| parse_u64(s).map(Value::Uint))
    }
    /// A column as a watch of signed integers.
    pub fn int(&self, column: &str, path: &str) -> Result<Watch, ParseError> {
        self.column(column, path, "signed", |s| parse_i64(s).map(Value::Int))
    }
    /// A column as a watch of flags: `0`/`1`, or `true`/`false`.
    pub fn bool(&self, column: &str, path: &str) -> Result<Watch, ParseError> {
        self.column(column, path, "boolean", |s| match s {
            "0" | "false" => Some(Value::Bool(false)),
            "1" | "true" => Some(Value::Bool(true)),
            _ => None,
        })
    }
    /// A `bytes:N` column — a hex string — as a watch of [`Value::Blob`], digested the way
    /// [`crate::hash::Walk::blob`] digests the bytes it walks, so the two compare.
    pub fn blob(&self, column: &str, path: &str) -> Result<Watch, ParseError> {
        self.column(column, path, "hex bytes", |s| {
            let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
            if hex.len() % 2 != 0 {
                return None;
            }
            let mut bytes = Vec::with_capacity(hex.len() / 2);
            for pair in hex.as_bytes().chunks(2) {
                let text = core::str::from_utf8(pair).ok()?;
                bytes.push(u8::from_str_radix(text, 16).ok()?);
            }
            Some(Value::Blob {
                len: bytes.len(),
                digest: crate::state_hash(&bytes, 0),
            })
        })
    }

    fn column(
        &self,
        column: &str,
        path: &str,
        kind: &str,
        read: impl Fn(&str) -> Option<Value>,
    ) -> Result<Watch, ParseError> {
        let Some(at) = self.columns.iter().position(|c| c == column) else {
            return err(
                1,
                format!("no column '{column}' in the capture ({})", self.columns.join(", ")),
            );
        };
        let mut samples = Vec::new();
        for (row, cells) in self.rows.iter().enumerate() {
            let cell = cells[at].as_str();
            // An empty cell is a frame where the recorder could not resolve the field: not a zero.
            if cell.is_empty() {
                continue;
            }
            let Some(value) = read(cell) else {
                return err(
                    row + 2,
                    format!("'{cell}' in column '{column}' is not {kind}"),
                );
            };
            samples.push(Sample { tick: self.tick_of(row), value });
        }
        Ok(Watch::new(path, samples))
    }
}

// ---------------------------------------------------------------------------------------------------
// The pad tape: the input recorded, and played back
// ---------------------------------------------------------------------------------------------------

/// The `dt` the engine hands the poll when a frame came in under its 30 fps cap — the only value a
/// recording of the original is ever *allowed* to be short of, and the one a hand-written tape uses.
///
/// It is not the `dt` of a running game. The engine measures the frame and averages it, so a capture of
/// real play carries three or four distinct values in a few hundred frames (0.03283332, 0.03286665,
/// 0.03333000 in the front end; R6 saw 0.041532 in a level). That is why [`PadFrame`] carries the
/// frame's own `dt` rather than assuming this one: the millisecond timers in [`crate::input`] accumulate
/// `dt * 1000` in float, so a tape replayed at a fixed step reproduces the *buttons* and not the timers,
/// and the front end's repeat rates are a function of the timers.
pub const CAPPED_DT: F = F(0.03333);

/// One frame of pad input, exactly as the engine's own poll was handed it: the frame's `dt` and one pad
/// report per port.
///
/// This is the unit a human-played session is recorded in. A [`Input`] is what the simulation gets
/// *after* the input layer has read this, and is derived from it ([`PadFrame::input`]) rather than
/// stored: the report carries the twelve pressures and the four axes too, and the pressures are what
/// twelve of the sixteen controls are actually read from (see [`crate::input::CONTROL_MAP`]), so a
/// recording that kept only a button mask could not reproduce a control's strength.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadFrame {
    /// The frame's `dt` in seconds, as a bit pattern so that a frame is `Eq` and a tape file
    /// round-trips exactly. Read it with [`PadFrame::dt`].
    pub dt: u32,
    /// One report per pad, in the device-list order [`Manager::tick`] reads them in.
    pub reports: [Report; PADS],
}

impl Default for PadFrame {
    /// A frame at the 30 fps cap with no pad plugged in: what [`Report::missing`] means, for every pad.
    fn default() -> PadFrame {
        PadFrame { dt: CAPPED_DT.to_bits(), reports: [Report::missing(); PADS] }
    }
}

impl PadFrame {
    /// One frame from the reports the platform layer built and the `dt` the engine measured.
    pub fn new(dt: F, reports: [Report; PADS]) -> PadFrame {
        PadFrame { dt: dt.to_bits(), reports }
    }

    /// One frame of a single pad holding exactly this bitmask ([`crate::input::Control::mask`]), with
    /// no second pad: the digital source — a keyboard, the kit's virtual pad, a hand-written tape.
    ///
    /// Full pressure on every analogue control in the mask, which is what [`Report::from_buttons`]
    /// means by a digital press and what `./kit pad` actually sends.
    pub fn of_buttons(dt: F, buttons: u16) -> PadFrame {
        PadFrame::new(dt, [Report::from_buttons(buttons), Report::missing()])
    }

    /// The frame's `dt`, as the poll is handed it.
    pub fn dt(&self) -> F {
        F::from_bits(self.dt)
    }

    /// What the game's input manager would believe is held on `pad` this frame, in the game's own
    /// bitmask — which is what [`Input::buttons`] carries.
    ///
    /// Computed by running the game's own poll ([`crate::input::Pad::poll`]) on a throwaway pad, so
    /// there is one implementation of "is this control down" in the port and not two. The first of
    /// poll's two passes is a pure function of the report; the second is the timers, which this throws
    /// away.
    pub fn held_mask(&self, pad: usize) -> u16 {
        let mut p = input::Pad::new(pad as u32);
        p.connected = true;
        p.poll(self.dt(), &self.reports[pad], &mut input::EventQueue::new());
        p.held_mask()
    }

    /// The left stick of `pad` as it arrived, with the centre byte taken off: `(-127..=128, …)`, right
    /// and down positive, as the report's bytes are, and +128 (`0xff`) wrapped to -128 as
    /// [`Input::stick`] holds it (S94: clamped to 127, a full stick replayed as `0xfe`).
    ///
    /// The raw arrival value, and nothing else: the dead zone, the curve and whatever history the game
    /// keeps of it are the port's business, not the recording's.
    pub fn stick(&self, pad: usize) -> (i8, i8) {
        let report = &self.reports[pad];
        let axis = |b: u8| b.wrapping_sub(input::AXIS_CENTRE) as i8;
        (axis(report.axis(2)), axis(report.axis(3)))
    }

    /// This frame as the simulation's own input: pad 0's held controls and its left stick.
    ///
    /// The reduction a [`Fixture`] stores, and the lossy half of this module. Three things in the frame
    /// do not survive it — the second pad, the analogue strengths, and the frame's `dt` — so a fixture
    /// replays the buttons of a recorded session and a [`Playback`] replays the session.
    pub fn input(&self) -> Input {
        Input { buttons: self.held_mask(0), stick: self.stick(0) }
    }
}

/// A recording of the pad: one [`PadFrame`] per frame, in the order they were polled.
///
/// Two ways in, and they are the same two a [`Fixture`] has:
///
/// - [`PadTape::from_capture`] reads the pad columns of a `./kit record` capture of the **original**.
///   That is a human-played sequence, and it is what an acceptance fixture's inputs are made of.
/// - [`PadTape::push`] is the port writing down its own frames as it plays them, which is how a
///   session in the port becomes a regression fixture.
///
/// Out: [`PadTape::inputs`] for a [`Fixture`], [`play`] to drive the port with the whole thing, and
/// [`PadTape::to_text`]/[`PadTape::parse`] for the file in between. A tape holds values read out of the
/// game, so a tape file is game data and lives in the git-ignored `context/`, never in the repository.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PadTape {
    /// What this recording is, for a failure line and for a human reading the directory. Free text
    /// without newlines.
    pub name: String,
    /// One entry per frame, in order.
    pub frames: Vec<PadFrame>,
    /// The game modifiers the ride was made with (P55), which a replay has to load the ride with to
    /// ride it again. The original game's — every one at its default — unless the file says.
    pub modifiers: crate::modifiers::Mods,
    /// The modifiers changed while the ride went on (P98), in tick order: `(k, mods)` is the set
    /// frames `k` on ride with, put on before frame `k` is stepped — after the pause they were set in,
    /// which is not on the tape. Empty for a ride that kept the set it loaded with, which is every tape
    /// before P98.
    pub changes: Vec<(usize, crate::modifiers::Mods)>,
    /// The first player switched machine while the ride went on (P102), in tick order: `(k, word)`
    /// is the machine they are put on before frame `k` is stepped, by the shell's word for it
    /// (`bike`, `biplane`). The simulation here knows no machines: the shell answers them. Empty
    /// for a ride on one machine, which is every tape before P102.
    pub switches: Vec<(usize, String)>,
    /// The first player's session markers as the ride started with them (P99): the saved-markers
    /// file's bytes ([`crate::markers::Markers::to_file`]), which a replay loads instead of whatever
    /// is saved on the machine it plays on. Empty for a ride that started with none, which is every
    /// tape before P99.
    pub markers: Vec<u8>,
    /// The frames the first player's markers were cleared before (P99, from the pause, which is not
    /// on the tape), in order: a replay empties the slots there as the ride did.
    pub cleared: Vec<usize>,
}

impl PadTape {
    /// An empty recording, named.
    pub fn new(name: impl Into<String>) -> PadTape {
        PadTape { name: name.into(), frames: Vec::new(), modifiers: Default::default(), changes: Vec::new(), switches: Vec::new(), markers: Vec::new(), cleared: Vec::new() }
    }

    /// The modifiers frame `k` rides on: the set the ride loaded with, as the changes before it left
    /// it.
    pub fn modifiers_at(&self, k: usize) -> crate::modifiers::Mods {
        self.changes.iter().take_while(|(at, _)| *at <= k).last().map_or(self.modifiers, |(_, m)| *m)
    }

    /// The change put on just before frame `k`, if there is one there.
    pub fn change_at(&self, k: usize) -> Option<crate::modifiers::Mods> {
        self.changes.iter().rev().find(|(at, _)| *at == k).map(|(_, m)| *m)
    }

    /// The machines the first player is switched to just before frame `k`, in the order they were.
    pub fn switches_at(&self, k: usize) -> impl Iterator<Item = &str> {
        self.switches.iter().filter(move |(at, _)| *at == k).map(|(_, w)| w.as_str())
    }

    /// The markers the tape's ride started with (P99), `None` for a tape that says none, and an
    /// error for markers this build cannot read (another version's): a replay without them would not
    /// be the ride the tape is of.
    pub fn start_markers(&self) -> Result<Option<crate::markers::Markers>, String> {
        if self.markers.is_empty() {
            return Ok(None);
        }
        crate::markers::Markers::read_file(&self.markers).map(|(_, m)| Some(m))
    }

    /// Write down one frame: the port recording what it just played.
    pub fn push(&mut self, frame: PadFrame) {
        self.frames.push(frame);
    }

    /// How many frames the recording is.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// One [`Input`] per frame, for [`Fixture::inputs`].
    pub fn inputs(&self) -> Vec<Input> {
        self.frames.iter().map(PadFrame::input).collect()
    }

    /// The least and greatest `dt` in the recording, as bit patterns.
    ///
    /// Worth looking at before trusting a replay: a tape whose two ends differ is a tape of a run the
    /// original measured frame by frame, and a port with a fixed step cannot match it however good the
    /// harness is. `None` for an empty tape.
    pub fn dt_range(&self) -> Option<(F, F)> {
        let mut frames = self.frames.iter().map(PadFrame::dt);
        let first = frames.next()?;
        Some(frames.fold((first, first), |(least, most), dt| {
            (if dt < least { dt } else { least }, if dt > most { dt } else { most })
        }))
    }

    /// The pad frames of a `./kit record` capture: what a human-played session actually gave the game,
    /// ready to drive the port.
    ///
    /// Reads five columns, all of them in `watch/input.toml` and `watch/acceptance.toml`:
    ///
    /// | column | what it is |
    /// |---|---|
    /// | `dt` | the `dt` the engine handed this frame's poll |
    /// | `pad_a`, `pad_b` | the two halves of the pad library's buffer for port 0, as hex bytes |
    /// | `stamp_a`, `stamp_b` | their stamps, which say **which half the game read** |
    ///
    /// Three things that are each a silent desync if got wrong, and are all decided here:
    ///
    /// - **The half with the smaller stamp is the one the game read.** `scePadRead` copies the half
    ///   whose stamp is the larger, but the pad library refreshes a half per video field — twice per
    ///   tick of this game — so by the time the recorder samples the row, the *other* half is the fresh
    ///   one. Measured on 600 rows of two captures: the controls the game believed were held matched
    ///   the older half's word in 600, and the newer half's in 582 — the rows where a press began or
    ///   ended being exactly the rows that matter.
    /// - **Row *k* is the input of the tick that produced row *k*.** The recorder samples once a frame,
    ///   after the tick, so row 0's pad bytes belong to the tick *before* the recording started and are
    ///   dropped: a tape is one frame shorter than the capture, and `frames[k - 1]` is the input of
    ///   tick *k*, which is [`Fixture`]'s own numbering.
    /// - **Every tick must be there.** The port has nothing to simulate a missing frame's input with,
    ///   so a capture written with `every = 2`, gated by a `when`, or with a row the recorder dropped,
    ///   is refused by its row number rather than replayed with a hole in it. Density is judged by the
    ///   game's own `frame_counter` where the capture has one, because the recorder's `frame` column
    ///   counts video fields and this game ticks once per two of them.
    ///
    /// The second pad is [`Report::missing`]: no watch spec records one, and I1 found the second pad's
    /// control map all zeros in a game nobody had plugged a second pad into. A capture that grows pad-1
    /// columns is the place this changes.
    pub fn from_capture(cap: &Capture) -> Result<PadTape, ParseError> {
        let dt = column_at(cap, "dt")?;
        let (pad_a, stamp_a) = (column_at(cap, "pad_a")?, column_at(cap, "stamp_a")?);
        let (pad_b, stamp_b) = (column_at(cap, "pad_b")?, column_at(cap, "stamp_b")?);
        dense_ticks(cap)?;

        let mut tape = PadTape::new(String::new());
        // Row 0's input belongs to the tick before the recording: the first frame of the tape is row 1.
        for row in 1..cap.rows.len() {
            let line = row + 2; // the header is line 1, so row 0 is line 2
            let number = |at: usize| -> Result<u64, ParseError> {
                let cell = cap.rows[row][at].as_str();
                parse_u64(cell).ok_or_else(|| ParseError {
                    line,
                    message: format!("'{cell}' in column '{}' is not a number", cap.columns[at]),
                })
            };
            let older = if number(stamp_a)? <= number(stamp_b)? { pad_a } else { pad_b };
            let bytes = cap.rows[row][older].as_str();
            let cell = cap.rows[row][dt].as_str();
            let Some(step) = parse_f32(cell) else {
                return err(line, format!("dt '{cell}' is not a number"));
            };
            tape.push(PadFrame {
                dt: step,
                reports: [report_from_hex(bytes, line, &cap.columns[older])?, Report::missing()],
            });
        }
        Ok(tape)
    }

    /// Read a pad tape file. The format, which is the fixture format's shape:
    ///
    /// ```text
    /// # game-sim pad tape
    /// format 1
    /// tape freeride-two-minutes
    /// modifiers gravity=50 bike_speed.1=200   # P55's game modifiers, only on a modified ride
    /// modifiers_from 600 gravity=100            # P98: frames 600 on ride on these (empty: the original's)
    /// switch 900 biplane                        # P102: the first player is put on this before frame 900
    /// frame 0x3d0699a3 0x0079fffb7f7f7f7f0000000000000000ff000000  # dt, pad 0, [pad 1]
    /// frame 0x3d0699a3 *0x0040 -                   # … or a bitmask, and "-" for no pad at all
    /// ```
    ///
    /// A `frame` line is `<dt> <pad 0> [<pad 1>]`, and a pad is written one of three ways:
    ///
    /// - `0x…` or bare hex — the report's bytes as the pad library left them. Short is allowed (a
    ///   capture records the first 20, which is every byte the poll reads) and the rest are zero.
    /// - `*<mask>` — the game's own bitmask, through [`Report::from_buttons`]: for a tape written by
    ///   hand or by a digital source, where there is no hardware to quote.
    /// - `-` — no pad ([`Report::missing`]). A `frame` line with one pad means the second is absent.
    ///
    /// `dt` is a float pattern (`0x3d0699a3`, exact — what [`PadTape::to_text`] writes) or a decimal
    /// (`0.03333`, read to the nearest float). Unknown keywords are errors with their line number, so
    /// a typo cannot quietly shorten a recording.
    pub fn parse(text: &str) -> Result<PadTape, ParseError> {
        let mut tape = PadTape::default();
        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut word = line.split_whitespace();
            match word.next().unwrap_or("") {
                "format" => match word.next().and_then(parse_u64) {
                    Some(v) if v as u32 == FORMAT_VERSION => {}
                    Some(v) => {
                        return err(n, format!("tape format {v}, this build reads {FORMAT_VERSION}"));
                    }
                    None => return err(n, "format wants a version number"),
                },
                "tape" => tape.name = line["tape".len()..].trim().to_string(),
                // The game modifiers the ride was made with (P55), in their own text form.
                "modifiers" => {
                    tape.modifiers = crate::modifiers::Mods::parse(&line["modifiers".len()..])
                        .map_err(|message| ParseError { line: n, message })?;
                }
                // A change of modifiers in the ride (P98): the frame it takes before, then the set.
                "modifiers_from" => {
                    let at = word.next().and_then(parse_u64).ok_or_else(|| ParseError {
                        line: n,
                        message: "modifiers_from wants the frame it takes before, then the modifiers".into(),
                    })? as usize;
                    if tape.changes.last().is_some_and(|(last, _)| *last > at) {
                        return err(n, format!("modifiers_from {at} comes after a later one"));
                    }
                    let mods = crate::modifiers::Mods::parse(&word.collect::<Vec<_>>().join(" "))
                        .map_err(|message| ParseError { line: n, message })?;
                    tape.changes.push((at, mods));
                }
                // A switch of machine in the ride (P102): the frame it comes before, then the machine.
                "switch" => {
                    let (Some(at), Some(machine), None) = (word.next().and_then(parse_u64), word.next(), word.next()) else {
                        return err(n, "switch wants the frame it comes before, then the machine");
                    };
                    let at = at as usize;
                    if tape.switches.last().is_some_and(|(last, _)| *last > at) {
                        return err(n, format!("switch {at} comes after a later one"));
                    }
                    tape.switches.push((at, machine.to_string()));
                }
                // The markers the ride started with (P99): the markers file's bytes, in hex.
                "markers" => {
                    let hex = word.next().unwrap_or("");
                    if hex.len() % 2 != 0 || word.next().is_some() {
                        return err(n, "markers wants one run of hex bytes");
                    }
                    let bytes: Option<Vec<u8>> =
                        (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect();
                    tape.markers = bytes.ok_or_else(|| ParseError { line: n, message: "markers wants hex bytes".into() })?;
                }
                // The first player's markers cleared before this frame (P99).
                "markers_cleared" => {
                    let Some(at) = word.next().and_then(parse_u64) else {
                        return err(n, "markers_cleared wants the frame it takes before");
                    };
                    if tape.cleared.last().is_some_and(|&last| last as u64 > at) {
                        return err(n, format!("markers_cleared {at} comes after a later one"));
                    }
                    tape.cleared.push(at as usize);
                }
                "frame" => {
                    let fields: Vec<&str> = word.collect();
                    if fields.is_empty() || fields.len() > 1 + PADS {
                        return err(n, format!("frame wants: <dt> <pad 0> [pad 1] ({PADS} pads)"));
                    }
                    let Some(dt) = parse_f32(fields[0]) else {
                        return err(n, format!("frame dt '{}' is not a number", fields[0]));
                    };
                    let mut reports = [Report::missing(); PADS];
                    for (pad, field) in fields[1..].iter().enumerate() {
                        reports[pad] = parse_report(field, n)?;
                    }
                    tape.push(PadFrame { dt, reports });
                }
                other => return err(n, format!("unknown keyword '{other}'")),
            }
        }
        Ok(tape)
    }

    /// Write the recording out in the form [`PadTape::parse`] reads: every report as its bytes and
    /// every `dt` as a pattern, so the round trip is exact.
    pub fn to_text(&self) -> String {
        use fmt::Write;
        let mut s = String::new();
        let _ = writeln!(s, "# game-sim pad tape: {} frames", self.frames.len());
        let _ = writeln!(s, "format {FORMAT_VERSION}");
        if !self.name.is_empty() {
            let _ = writeln!(s, "tape {}", self.name);
        }
        // Only a modified ride says so, so the original's tapes are the bytes they always were.
        if !self.modifiers.is_original() {
            let _ = writeln!(s, "modifiers {}", self.modifiers.text());
        }
        for (at, mods) in &self.changes {
            let _ = writeln!(s, "modifiers_from {at} {}", mods.text());
        }
        for (at, machine) in &self.switches {
            let _ = writeln!(s, "switch {at} {machine}");
        }
        // Only a ride that started with markers says so (P99), for the same reason.
        if !self.markers.is_empty() {
            let _ = write!(s, "markers ");
            for byte in &self.markers {
                let _ = write!(s, "{byte:02x}");
            }
            let _ = writeln!(s);
        }
        for at in &self.cleared {
            let _ = writeln!(s, "markers_cleared {at}");
        }
        for frame in &self.frames {
            let _ = write!(s, "frame 0x{:08x}", frame.dt);
            for report in &frame.reports {
                let _ = match *report == Report::missing() {
                    true => write!(s, " -"),
                    false => write!(s, " 0x{}", hex_of(report)),
                };
            }
            let _ = writeln!(s);
        }
        s
    }
}

/// A pad report's bytes as the hex a tape file and a capture both write them.
fn hex_of(report: &Report) -> String {
    use fmt::Write;
    let mut s = String::with_capacity(report.0.len() * 2);
    for byte in report.0 {
        let _ = write!(s, "{byte:02x}");
    }
    s
}

/// One pad of a `frame` line: `-`, `*<mask>`, or the report's bytes as hex.
fn parse_report(field: &str, line: usize) -> Result<Report, ParseError> {
    if field == "-" {
        return Ok(Report::missing());
    }
    if let Some(mask) = field.strip_prefix('*') {
        return match parse_u64(mask).filter(|m| *m <= u64::from(u16::MAX)) {
            Some(m) => Ok(Report::from_buttons(m as u16)),
            None => err(line, format!("'{mask}' is not a 16-bit bitmask")),
        };
    }
    report_from_hex(field, line, "frame")
}

/// A report from a hex string of its bytes, as a capture's `bytes:N` column and a tape file both hold
/// it. Fewer than [`crate::input::REPORT_BYTES`] bytes is allowed and the rest are zero: a capture
/// records the first 20, which is every byte the poll reads (status, mode, the button word, the four
/// axes and the twelve pressures), so the bytes past them are known to be unread rather than known to
/// be zero.
fn report_from_hex(text: &str, line: usize, column: &str) -> Result<Report, ParseError> {
    let hex = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")).unwrap_or(text);
    // Eight bytes reach the first pressure; a shorter cell is a truncated recording, not a pad.
    if hex.len() < 16 || hex.len() % 2 != 0 || hex.len() > input::REPORT_BYTES * 2 {
        return err(
            line,
            format!(
                "'{text}' in '{column}' is not a pad report: wanted 8 to {} bytes of hex",
                input::REPORT_BYTES,
            ),
        );
    }
    let mut bytes = [0u8; input::REPORT_BYTES];
    for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks(2)) {
        let pair = core::str::from_utf8(pair).unwrap_or("");
        let Ok(value) = u8::from_str_radix(pair, 16) else {
            return err(line, format!("'{text}' in '{column}' is not hex"));
        };
        *byte = value;
    }
    Ok(Report::from_bytes(bytes))
}

/// The index of a capture's column, or an error naming the columns it does have.
fn column_at(cap: &Capture, name: &str) -> Result<usize, ParseError> {
    cap.columns.iter().position(|c| c == name).ok_or_else(|| ParseError {
        line: 1,
        message: format!("no column '{name}' in the capture ({})", cap.columns.join(", ")),
    })
}

/// The column holding the game's own frame counter, under either name a capture spec gives it.
///
/// Two names for one word (app+0x374): `watch/acceptance.toml` records it as `frame_counter` and
/// `watch/input.toml` as `engine_frame`. A reader that knew only one of them would silently fall back
/// to the recorder's video-field `frame` on half the captures in the project, which is the counter that
/// says twice the tick for this game.
fn game_counter(cap: &Capture) -> Option<usize> {
    ["frame_counter", "engine_frame"].iter().find_map(|name| column_at(cap, name).ok())
}

/// One row per tick, with nothing missing — the check that makes a capture an input recording.
///
/// By the game's own `frame_counter` when the capture has one (it rises by exactly 1 a tick), and
/// otherwise by the recorder's `frame` column, which counts **video fields**: this game ticks once per
/// two of them, so what is checked there is a constant stride rather than a step of 1. A capture with a
/// variable stride is one with a dropped row in it, whichever column is doing the counting.
fn dense_ticks(cap: &Capture) -> Result<(), ParseError> {
    let (counting, step) = match game_counter(cap) {
        Some(at) => (at, Some(1)),
        None => (0, None), // the `frame` column, whose stride the first two rows decide
    };
    let mut want = step;
    let mut previous: Option<u64> = None;
    for row in 0..cap.rows.len() {
        let line = row + 2;
        let cell = cap.rows[row][counting].as_str();
        let Some(value) = parse_u64(cell) else {
            return err(line, format!("'{cell}' in '{}' is not a number", cap.columns[counting]));
        };
        if let Some(last) = previous {
            let delta = value.wrapping_sub(last);
            let want = *want.get_or_insert(delta);
            if delta != want {
                return err(
                    line,
                    format!(
                        "{} {value} follows {last}, a step of {delta} where every other row steps \
                         {want}: an input recording needs every tick, so it must be recorded with \
                         every = 1 and no row dropped",
                        cap.columns[counting],
                    ),
                );
            }
        }
        previous = Some(value);
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// Playing a tape back through the port
// ---------------------------------------------------------------------------------------------------

/// The port being driven by a recording: a whole tape's worth of frames handed to the simulation.
///
/// One frame is [`Playback::advance`], which is the original's own order — the pad is read, *then* the
/// simulation runs — so a press is latched before the frame that acts on it. That ordering is the whole
/// reason this type exists rather than a loop over [`PadTape::inputs`]: the input layer is stateful (the
/// press and release timers, the one-frame event queue) and a replay that fed the simulation without
/// ticking it would have no timers at all.
///
/// A tape carries more than a [`Fixture`]'s [`Input`] can, and all of it reaches the simulation here:
/// the frame's own `dt` and the pad's twelve pressures, through [`Sim::frame`]. That is the one way
/// riding a tape differs from replaying a fixture of the same session — the fixture's buttons rebuild
/// a report with every press at full strength and simulate at the 30 Hz clamp ([`Sim::tick`]).
#[derive(Clone, Debug)]
pub struct Playback {
    /// The simulation, which is where the input layer lives.
    pub sim: Sim,
}

impl Playback {
    /// A run from a seed, with pad 0 plugged in and pad 1 not — the shape every capture of this game
    /// was taken in.
    pub fn new(seed: u32) -> Playback {
        Playback::from_state(Sim::new(seed))
    }

    /// A run from a state: a session that starts in the middle of a level, from a save state's RAM.
    pub fn from_state(sim: Sim) -> Playback {
        Playback { sim }
    }

    /// One frame, in the original's order: read the pad, then simulate. Both halves are
    /// [`Sim::frame`], over a flat floor — a tape of a session on a *track* is ridden by the shell,
    /// which has the track.
    pub fn advance(&mut self, frame: &PadFrame) {
        self.sim.tick_frame(frame.dt(), &frame.reports);
    }

    /// The input layer this run is feeding, which is the simulation's own.
    pub fn input(&self) -> &Manager {
        &self.sim.input
    }

    /// Which tick the run is on, counting from the state it started at.
    pub fn tick(&self) -> u64 {
        self.sim.ticks
    }

    /// The whole port in 64 bits: the simulation's state hash, which since P5 covers the input layer
    /// too — the pads are part of the state the frame reads, and a divergence in a press timer is a
    /// divergence.
    pub fn hash(&self) -> u64 {
        hash::of(&self.sim)
    }
}

/// Play a whole recording into a fresh run and hand back where it ended up.
pub fn play(tape: &PadTape, seed: u32) -> Playback {
    play_from(tape, Sim::new(seed))
}

/// Play a whole recording into a run that starts from `sim`, with the markers the tape started with
/// when it says (P99). Panics on markers this build cannot read ([`PadTape::start_markers`]).
pub fn play_from(tape: &PadTape, sim: Sim) -> Playback {
    let mut run = Playback::from_state(sim);
    if let Some(markers) = tape.start_markers().expect("the tape's markers") {
        run.sim.markers = markers;
    }
    for (k, frame) in tape.frames.iter().enumerate() {
        if tape.cleared.contains(&k) {
            run.sim.markers.clear();
        }
        run.advance(frame);
    }
    run
}

impl Fixture {
    /// The fixture of a recorded session: its inputs are the tape's, and it expects nothing yet.
    ///
    /// What to add to it, and from where:
    ///
    /// - the **watches**, from the same capture the tape came from ([`Capture::float`] and friends) —
    ///   that is an acceptance fixture, and the only kind that proves the port right;
    /// - the **hashes**, by running the port over [`PadTape::inputs`] with [`record`] — that is a
    ///   regression fixture of a real session's input, which is the next best thing to have while the
    ///   state a capture watches is still being ported.
    pub fn from_tape(tape: &PadTape, seed: u32) -> Fixture {
        Fixture {
            name: tape.name.clone(),
            seed,
            inputs: tape.inputs(),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// A PCSX2 input recording (.p2m2)
// ---------------------------------------------------------------------------------------------------

/// The file version this reader was measured against — the first byte of a `.p2m2`.
pub const P2M2_FILE_VERSION: u8 = 1;

/// How many bytes of header a PCSX2 input recording begins with: the file version (`u8`), the
/// emulator's version string (`char[50]`), the author (`char[255]`), the game's name (`char[255]`),
/// the frame count (`u32`), the undo count (`u32`) and the "started from a save state" flag (`u8`),
/// in that order, little-endian and with no padding.
pub const P2M2_HEADER_BYTES: usize = 570;

/// One pad's entry in one of PCSX2's frames: the SIO2 reply the emulated pad will give, less its three
/// header bytes. Both halves of the button word, the four stick axes, then the twelve pressures.
pub const P2M2_PAD_BYTES: usize = 18;

/// How many pads one frame of the file holds. PCSX2 writes both of them every frame whether anything
/// is plugged into the second port or not, which is why an unused port reads as eighteen zero bytes.
pub const P2M2_PADS: usize = 2;

/// One frame of the file: a pad entry per port.
pub const P2M2_FRAME_BYTES: usize = P2M2_PAD_BYTES * P2M2_PADS;

/// How many of PCSX2's frames go by in one tick of this game.
///
/// PCSX2 counts a frame per video field (59.94 Hz on this disc) and writes the pad it was handed into
/// every one of them. This game simulates once per **two** fields — measured three ways: the recorder's
/// vsync counter rises by 2 while the game's own frame counter rises by 1 (S8, 300 of 300 rows), R6 saw
/// the same in a level, and the engine's `dt` is a thirtieth of a second rather than a sixtieth. So
/// half of a recording's frames were never read by the game, and a tape takes every second one.
pub const P2M2_FIELDS_PER_TICK: usize = 2;

/// Where a PCSX2 input recording could not be read, with the byte it was at — the binary counterpart
/// of [`ParseError`], which carries a line number a file with no lines cannot have.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2m2Error {
    /// The offset in the file, so a hex dump can be opened where the trouble is.
    pub at: usize,
    pub message: String,
}

impl fmt::Display for P2m2Error {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(w, "byte {:#x}: {}", self.at, self.message)
    }
}

impl std::error::Error for P2m2Error {}

fn p2m2_err<T>(at: usize, message: impl Into<String>) -> Result<T, P2m2Error> {
    Err(P2m2Error { at, message: message.into() })
}

/// **A human's PCSX2 input recording**, read as the pad the game was handed.
///
/// A `.p2m2` is what PCSX2's own input recording writes: a fixed header, then one entry per pad per
/// *video field* for as long as the recording ran. It is the cheapest honest source of a long
/// human-played sequence there is — no capture session, no PINE, no kit, just somebody riding — and
/// with its `_SaveState.p2s` beside it, it is reproducible on the original: load the state, play the
/// recording, and the emulator does the same sixty seconds again.
///
/// Three things this reader decides, each measured rather than assumed, and each a silent desync if
/// got wrong:
///
/// - **Which byte is which.** The eighteen bytes of an entry are mapped onto the port's own
///   [`Report`] by [`report_of_p2m2`]. The file proves that mapping itself: for twelve of the sixteen
///   controls the entry carries *both* the button's bit and its pressure byte, and the two must agree
///   on every frame ([`P2m2::check_pressures`]). On the one recording in the project that is 43,452
///   agreements and no disagreement, where either neighbouring pairing of pressure to bit disagrees
///   5,814 times.
/// - **Frame 0 is not a frame.** PCSX2 seeks to `header + frame * 36` and numbers a recording's frames
///   from 1, so the slot before the first one is never written and reads as thirty-six zero bytes —
///   which, mapped literally, would be a pad with every button held and both sticks hard into a
///   corner. [`P2m2::parse`] requires it to be unwritten and drops it, and refuses a file whose slot
///   count is not `totalFrames + 1` rather than replay a recording half a tick out of step.
/// - **Two fields, one tick.** Half the frames of the file were never read by the game
///   ([`P2M2_FIELDS_PER_TICK`]); which half is [`TickPhase`], and it is the one thing the file cannot
///   say.
///
/// The `dt` is not in the file either, and does not need to be: see [`P2m2::tape`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2m2 {
    /// The first byte of the file. [`P2M2_FILE_VERSION`] is what this reader knows.
    pub file_version: u8,
    /// The emulator that wrote it, e.g. `PCSX2-v2.9.127`.
    pub emulator: String,
    /// Who played it.
    pub author: String,
    /// The game, as PCSX2 names it.
    pub game: String,
    /// How many video fields the recording covers, which is the header's own count and the number of
    /// entries [`P2m2::field`] has.
    pub total_frames: u32,
    /// How many times the recording was rewound while it was made. Recorded for provenance; nothing
    /// here reads it.
    pub undo_count: u32,
    /// Whether the recording starts from a save state (the `_SaveState.p2s` beside it) rather than from
    /// power-on. A tape of a recording that starts from a state is only replayable against that state.
    pub from_savestate: bool,
    /// One entry per *written* field, in file order: `fields[0]` is the file's frame 1.
    fields: Vec<[u8; P2M2_FRAME_BYTES]>,
}

/// Which of a tick's two video fields the game's pad read landed on.
///
/// The pad library refreshes one of its two buffer halves per field and the poll copies the fresher
/// half, so the bytes a tick reads are the ones PCSX2 replied with at the field that tick began on —
/// one of the two, consistently, for the whole recording. Which one depends on where in PCSX2's frame
/// the recording's save state was taken, and the file does not say: it is a choice between two
/// sequences that agree on most ticks and not all ([`P2m2::ticks_the_phase_moves`] counts them).
///
/// It can be *measured* for a given recording, by playing it back on the original and comparing
/// against libpad's buffer in EE RAM tick by tick — only the right phase survives past the first
/// press. Done once so far, for `context/recordings/first_recording.p2m2`: `Second`. That is a fact
/// about that file and its save state, not about the game, so it is not this enum's default; the
/// proof is `the_capture_is_the_recording_played_back_and_the_game_reads_the_second_field` in
/// `crates/game-sim/tests/replay.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TickPhase {
    /// The first field of each pair: fields 1, 3, 5… of the file. The default.
    #[default]
    First,
    /// The second field of each pair: fields 2, 4, 6… of the file.
    Second,
}

impl TickPhase {
    /// How many of the file's fields are skipped before the first tick's.
    pub fn offset(self) -> usize {
        match self {
            TickPhase::First => 0,
            TickPhase::Second => 1,
        }
    }

    /// The word a tape's name carries, so a fixture says which phase it was built with.
    pub fn name(self) -> &'static str {
        match self {
            TickPhase::First => "first",
            TickPhase::Second => "second",
        }
    }
}

/// One pad's eighteen recorded bytes as the report the game's own poll reads.
///
/// | report byte | what | from |
/// |---|---|---|
/// | 0 | status, 0 for a good read | this reader: a recorded frame is a read that happened |
/// | 1 | mode | [`input::MODE_PRESSURES`] — it recorded pressures. Never read by the poll |
/// | 2, 3 | the button word, big-endian and active low | entry bytes 0, 1 |
/// | 4..8 | right X, right Y, left X, left Y | entry bytes 2..6 |
/// | 8..20 | the twelve pressures, in the DualShock 2 order | entry bytes 6..18 |
///
/// The pressure order is the same in both, which is what makes the last row a copy rather than a
/// permutation: [`input::CONTROL_MAP`]'s pressure indices were read out of the game's own RAM and the
/// file's order is the DualShock 2's, and they agree byte for byte.
///
/// An entry of eighteen zero bytes is [`Report::missing`] — a port PCSX2 never wrote, which is every
/// frame of port 1 in a one-pad recording. It is not a pad holding all sixteen buttons with its sticks
/// at zero, which is what the bytes would otherwise say.
pub fn report_of_p2m2(entry: &[u8; P2M2_PAD_BYTES]) -> Report {
    if *entry == [0u8; P2M2_PAD_BYTES] {
        return Report::missing();
    }
    let mut bytes = [0u8; input::REPORT_BYTES];
    bytes[1] = input::MODE_PRESSURES;
    bytes[2] = entry[0];
    bytes[3] = entry[1];
    bytes[4..8].copy_from_slice(&entry[2..6]);
    bytes[8..20].copy_from_slice(&entry[6..18]);
    Report::from_bytes(bytes)
}

impl P2m2 {
    /// Read a `.p2m2`.
    ///
    /// Refuses, by the byte it is at: a file too short to hold a header, a frame area that is not a
    /// whole number of frames, a slot count that is not `totalFrames + 1`, a written frame 0, and a
    /// file version this reader has not been measured against. Every one of those is a file whose
    /// frames are not where this reader thinks they are, and a tape built from one would replay
    /// plausible nonsense.
    pub fn parse(bytes: &[u8]) -> Result<P2m2, P2m2Error> {
        if bytes.len() < P2M2_HEADER_BYTES {
            return p2m2_err(
                bytes.len(),
                format!(
                    "{} bytes is not a PCSX2 input recording: the header alone is {P2M2_HEADER_BYTES}",
                    bytes.len(),
                ),
            );
        }
        let text = |at: usize, len: usize| -> String {
            let field = &bytes[at..at + len];
            let end = field.iter().position(|b| *b == 0).unwrap_or(len);
            String::from_utf8_lossy(&field[..end]).trim().to_string()
        };
        let file_version = bytes[0];
        if file_version != P2M2_FILE_VERSION {
            return p2m2_err(
                0,
                format!(
                    "file version {file_version}; this reader was measured against \
                     {P2M2_FILE_VERSION} ({}), and a layout change would move every frame",
                    text(1, 50),
                ),
            );
        }
        let word = |at: usize| -> u32 {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        let total_frames = word(561);
        let frames = &bytes[P2M2_HEADER_BYTES..];
        if frames.len() % P2M2_FRAME_BYTES != 0 {
            return p2m2_err(
                P2M2_HEADER_BYTES + frames.len() - frames.len() % P2M2_FRAME_BYTES,
                format!(
                    "{} bytes of frames is {} short of a whole frame ({P2M2_FRAME_BYTES} bytes: \
                     {P2M2_PAD_BYTES} per pad, {P2M2_PADS} pads) — a truncated recording",
                    frames.len(),
                    P2M2_FRAME_BYTES - frames.len() % P2M2_FRAME_BYTES,
                ),
            );
        }
        let slots = frames.len() / P2M2_FRAME_BYTES;
        if slots as u64 != u64::from(total_frames) + 1 {
            return p2m2_err(
                561,
                format!(
                    "{slots} frame slots for a totalFrames of {total_frames}: PCSX2 numbers a \
                     recording's frames from 1 and leaves the slot before them unwritten, so a file \
                     of {total_frames} frames holds {} slots. Reading this one would put every tick \
                     a field out of step",
                    u64::from(total_frames) + 1,
                ),
            );
        }
        if frames[..P2M2_FRAME_BYTES] != [0u8; P2M2_FRAME_BYTES] {
            return p2m2_err(
                P2M2_HEADER_BYTES,
                "frame slot 0 is written, and this reader assumes PCSX2 never writes it (it numbers \
                 a recording's frames from 1). Either the file is from a build that numbers them \
                 from 0 — in which case every tick of a tape would be a field out of step — or it is \
                 not a recording at all"
                    .to_string(),
            );
        }
        let mut fields = Vec::with_capacity(slots - 1);
        for slot in 1..slots {
            let at = slot * P2M2_FRAME_BYTES;
            let mut entry = [0u8; P2M2_FRAME_BYTES];
            entry.copy_from_slice(&frames[at..at + P2M2_FRAME_BYTES]);
            fields.push(entry);
        }
        Ok(P2m2 {
            file_version,
            emulator: text(1, 50),
            author: text(51, 255),
            game: text(306, 255),
            total_frames,
            undo_count: word(565),
            from_savestate: bytes[569] != 0,
            fields,
        })
    }

    /// How many video fields the recording holds — the header's `totalFrames`, which this reader has
    /// already checked the file's own length against.
    pub fn fields(&self) -> usize {
        self.fields.len()
    }

    /// One pad of one field, as the report the poll reads. `field` is 0-based over the *written*
    /// fields, so field 0 is the file's frame 1.
    pub fn field(&self, field: usize, pad: usize) -> Report {
        let Some(frame) = self.fields.get(field) else {
            return Report::missing();
        };
        if pad >= P2M2_PADS {
            return Report::missing();
        }
        let at = pad * P2M2_PAD_BYTES;
        let mut entry = [0u8; P2M2_PAD_BYTES];
        entry.copy_from_slice(&frame[at..at + P2M2_PAD_BYTES]);
        report_of_p2m2(&entry)
    }

    /// How many ticks a tape of this recording has: the fields of the chosen phase.
    pub fn ticks(&self, phase: TickPhase) -> usize {
        let fields = self.fields();
        match fields.checked_sub(phase.offset()) {
            Some(n) => n.div_ceil(P2M2_FIELDS_PER_TICK),
            None => 0,
        }
    }

    /// **The recording as a tape**, at the step the engine handed the frame.
    ///
    /// `dt` is the one thing a tape needs that a `.p2m2` does not hold, and for a recording of play it
    /// is a constant: [`CAPPED_DT`], the 30 fps cap. The engine measures each frame, averages the last
    /// thirty and then clamps the mean to 0.03333 — and on a machine keeping up with the disc's
    /// 59.94 Hz, two fields are 33.3667 ms, so the mean is over the cap and the clamp bites on every
    /// frame. Three measurements of this recording's own save state say so, and they are the reason
    /// this method takes no `dt` argument:
    ///
    /// - the `dt` the simulation was handed on the frame the state was taken (app+0x344) is
    ///   0.033330001, exactly [`CAPPED_DT`], with 1/`dt` at 30.003;
    /// - the frame's raw measured delta (app+0x358) is 0.034 — 34 ms, over the cap, so the clamp was
    ///   applied rather than coincidentally matched;
    /// - the accumulated *simulated* clock (app+0x330) is 975 ms behind the wall clock it was seeded
    ///   from, over 2,773 frames. That deficit is what clamping 33.37 ms frames to a 33 ms step costs,
    ///   and it says the cap has been biting all session rather than on the one frame that was looked
    ///   at.
    ///
    /// R6 measured the same 0.0333300 across 119 ticks of a level with the cap flag reading 1, and S1
    /// found the flag set by the game object's constructor and never cleared. [`PadTape::from_capture`]
    /// is the other way round: a capture carries the `dt` the engine really used, so a tape made from
    /// one does not have to reason about the cap at all.
    ///
    /// Use [`P2m2::tape_with_dt`] where a measured `dt` is to hand — the front end runs under the cap
    /// (S8 saw 0.032833 there), so a recording of a *menu* is not a recording this method describes.
    pub fn tape(&self, phase: TickPhase) -> PadTape {
        self.tape_with_dt(phase, CAPPED_DT)
    }

    /// [`P2m2::tape`] with a step of your own: for a recording whose frames the engine did not clamp,
    /// or to replay the same recording at two steps and see what the millisecond timers do.
    pub fn tape_with_dt(&self, phase: TickPhase, dt: F) -> PadTape {
        let mut tape = PadTape::new(format!(
            "p2m2-{}-{}f-{}",
            slug(&self.author),
            self.total_frames,
            phase.name(),
        ));
        let mut field = phase.offset();
        while field < self.fields() {
            let mut reports = [Report::missing(); PADS];
            for (pad, report) in reports.iter_mut().enumerate() {
                *report = self.field(field, pad);
            }
            tape.push(PadFrame::new(dt, reports));
            field += P2M2_FIELDS_PER_TICK;
        }
        tape
    }

    /// How much the phase can matter, measured on this recording: how many of its ticks carry
    /// different pad bytes in their two fields, and how many of those differ in the button word.
    ///
    /// The human moved the stick between two fields far more often than they crossed a button's edge
    /// between them, so the second number is the small one — and it is the one that would show up as a
    /// press landing a tick early or late. Neither is zero, which is why [`TickPhase`] is a choice and
    /// not a detail.
    pub fn ticks_the_phase_moves(&self) -> (usize, usize) {
        let mut moved = (0, 0);
        for pair in self.fields.chunks_exact(P2M2_FIELDS_PER_TICK) {
            if pair[0][..P2M2_PAD_BYTES] != pair[1][..P2M2_PAD_BYTES] {
                moved.0 += 1;
                if pair[0][..2] != pair[1][..2] {
                    moved.1 += 1;
                }
            }
        }
        moved
    }

    /// **The file's own proof of the byte mapping**: every pressure byte agrees with the bit of the
    /// same control, on every field, for both pads. Returns how many agreements it checked.
    ///
    /// Twelve of the sixteen controls are read from a pressure byte and not from their bit
    /// ([`input::CONTROL_MAP`]), so the two halves of an entry are redundant — and they were written
    /// by PCSX2 from a host pad, not by this reader. A pairing that is off by one disagrees on
    /// thousands of frames, so this is the check that turns "the DualShock 2 order" from a convention
    /// into a measurement.
    ///
    /// It is also the check that a recording can drive the port *at all*: a `.p2m2` of a host pad that
    /// reported no pressures would leave those twelve controls released however much its button word
    /// says, and the error names the first frame and control where the two disagree.
    pub fn check_pressures(&self) -> Result<usize, P2m2Error> {
        let mut agreements = 0;
        for field in 0..self.fields() {
            for pad in 0..P2M2_PADS {
                let report = self.field(field, pad);
                if report.status() != 0 {
                    continue; // a port PCSX2 never wrote
                }
                let buttons = report.buttons();
                for control in input::Control::ALL {
                    let binding = control.binding();
                    if !binding.is_analogue() {
                        continue; // select, start, L3 and R3 have no pressure byte, in either format
                    }
                    let down = buttons & binding.mask != 0;
                    let pressed = report.pressure(binding.pressure) != 0;
                    if down != pressed {
                        return p2m2_err(
                            P2M2_HEADER_BYTES + (field + 1) * P2M2_FRAME_BYTES + pad * P2M2_PAD_BYTES,
                            format!(
                                "frame {} pad {pad}: {} reads {} by its bit and {} by its pressure \
                                 byte ({:#04x}). The two are written from the same host pad, so \
                                 either the byte mapping is wrong or this recording has no pressures \
                                 — and the game reads this control from the pressure byte only",
                                field + 1,
                                control.name(),
                                if down { "down" } else { "up" },
                                if pressed { "down" } else { "up" },
                                report.pressure(binding.pressure),
                            ),
                        );
                    }
                    agreements += 1;
                }
            }
        }
        Ok(agreements)
    }
}

/// A name safe to write on a tape's `tape` line and read back: a `#` would be a comment and a space
/// would end the name early.
fn slug(text: &str) -> String {
    let kept: String = text
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = kept.trim_matches('-').to_string();
    match trimmed.is_empty() {
        true => "anon".to_string(),
        false => trimmed,
    }
}

impl PadTape {
    /// **A human's PCSX2 recording as a tape, in one call**: read the file, check that its pressure
    /// bytes agree with its button bits, and take one field of every tick.
    ///
    /// This is the whole path from `context/recordings/*.p2m2` to something [`play`] can drive the
    /// port with. [`P2m2::parse`] and [`P2m2::tape`] are the same thing in two steps, for a caller
    /// that wants the header or a step of its own.
    pub fn from_p2m2(bytes: &[u8], phase: TickPhase) -> Result<PadTape, P2m2Error> {
        let movie = P2m2::parse(bytes)?;
        movie.check_pressures()?;
        Ok(movie.tape(phase))
    }
}

// ---------------------------------------------------------------------------------------------------
// The shell's own tape: writing one while a session plays, and riding one instead of a pad
// ---------------------------------------------------------------------------------------------------

/// A tape being written by a session that is being played right now: the app layer hands it every
/// frame it simulates, in the order it simulated them, and it hands back the file.
///
/// The shell's `--record FILE` is this type and nothing else. It exists rather than a bare
/// [`PadTape::push`] so that the two rules a recording has to keep are kept in one place, by code a
/// test can drive without a window:
///
/// - **One entry per simulation pass, not per loop iteration.** A frame the engine did not simulate
///   (a frozen frame, the shutdown frame) has no input the port could replay, and a tape with one in
///   it plays back one tick longer than the session it recorded. So the recorder is fed from inside
///   the simulation pass, where the `dt` and the reports are the ones that frame actually used.
/// - **The `dt` recorded is the `dt` the simulation was handed**, not the wall clock's delta: the
///   engine averages thirty frames and clamps at 30 fps ([`crate::tick::DeltaTime`]), so the measured
///   delta of a frame is not what that frame simulated with, and a tape of the deltas would replay as
///   a different run.
///
/// What comes out is an ordinary [`PadTape`] — the same file [`PadTape::from_capture`] makes of a
/// capture of the original and [`PadTape::from_p2m2`] makes of a human's PCSX2 recording, so a tape of
/// the port and a tape of the original are the same kind of file and [`play`] rides either.
#[derive(Clone, Debug, Default)]
pub struct Recorder {
    /// What has been recorded so far.
    pub tape: PadTape,
}

impl Recorder {
    /// A recorder writing a tape under this name.
    pub fn new(name: impl Into<String>) -> Recorder {
        Recorder { tape: PadTape::new(name) }
    }

    /// Write down the frame the simulation is in the middle of: the `dt` it was handed and the reports
    /// it read.
    pub fn frame(&mut self, dt: F, reports: &[Report; PADS]) {
        self.tape.push(PadFrame::new(dt, *reports));
    }

    /// Write down that the ride's modifiers changed to `mods` (P98): the frames recorded from here on
    /// ride on them. Two changes between the same two frames are one, the last.
    pub fn modifiers(&mut self, mods: crate::modifiers::Mods) {
        let at = self.tape.len();
        if self.tape.changes.last().is_some_and(|(last, _)| *last == at) {
            self.tape.changes.pop();
        }
        self.tape.changes.push((at, mods));
    }

    /// Write down that the first player switched to `machine` (P102, the shell's word for it): the
    /// frames recorded from here on are ridden on it.
    pub fn switch(&mut self, machine: &str) {
        let at = self.tape.len();
        self.tape.switches.push((at, machine.to_string()));
    }

    /// Write down that the first player's markers were cleared (P99): before the frame recorded
    /// next.
    pub fn markers_cleared(&mut self) {
        let at = self.tape.len();
        if self.tape.cleared.last() != Some(&at) {
            self.tape.cleared.push(at);
        }
    }

    /// How many frames have been recorded.
    pub fn len(&self) -> usize {
        self.tape.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tape.is_empty()
    }

    /// The tape as its file, for the app layer to write — this crate does not touch files.
    pub fn to_text(&self) -> String {
        self.tape.to_text()
    }
}

/// The playback head: where a live loop has got to in the tape it is riding.
///
/// [`Playback`] plays a whole tape into the headless simulation in one call. A shell with a window
/// cannot do that — it has a frame to draw and a camera to move between one pad frame and the next —
/// so it holds a head instead and pulls one frame per loop, which is what the shell's `--play FILE`
/// does. The head is the whole of the difference between the two: a tape is still played in order,
/// every frame exactly once, and [`Head::done`] is how the loop knows the ride is over.
///
/// A loop riding a tape must take the frame's `dt` from the tape too, and not from its own clock.
/// Everything the recorded session simulated it simulated with the tape's `dt`, so a playback paced by
/// the wall clock reproduces the buttons and not the run.
#[derive(Clone, Debug, Default)]
pub struct Head {
    /// The recording being ridden.
    pub tape: PadTape,
    /// How many of its frames have been handed out.
    at: usize,
}

impl Head {
    /// A head at the start of a tape.
    pub fn new(tape: PadTape) -> Head {
        Head { tape, at: 0 }
    }

    /// How many frames have been handed out, which is the tick the ride is on.
    pub fn at(&self) -> usize {
        self.at
    }

    /// How many frames the recording is.
    pub fn len(&self) -> usize {
        self.tape.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tape.is_empty()
    }

    /// Whether the tape has run out, which is the loop's cue to stop.
    pub fn done(&self) -> bool {
        self.at >= self.tape.len()
    }

    /// The modifiers change the ride takes before the frame [`Head::next`] hands out next (P98), if
    /// the recording changed them there.
    pub fn change(&self) -> Option<crate::modifiers::Mods> {
        self.tape.change_at(self.at)
    }

    /// The machines the first player is switched to before the frame [`Head::next`] hands out next
    /// (P102), in order, where the recording switched there.
    pub fn switches(&self) -> Vec<String> {
        self.tape.switches_at(self.at).map(str::to_string).collect()
    }

    /// Whether the recording cleared the first player's markers before the frame [`Head::next`]
    /// hands out next (P99).
    pub fn cleared(&self) -> bool {
        self.tape.cleared.contains(&self.at)
    }
}

/// The head hands out the recording's frames in order, one per call, and then stops: a loop can ride a
/// tape with `while let Some(frame) = head.next()` and a test can `collect` one.
impl Iterator for Head {
    type Item = PadFrame;

    fn next(&mut self) -> Option<PadFrame> {
        let frame = *self.tape.frames.get(self.at)?;
        self.at += 1;
        Some(frame)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.tape.len() - self.at;
        (left, Some(left))
    }
}
