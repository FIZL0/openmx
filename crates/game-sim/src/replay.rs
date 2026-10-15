//! The acceptance test for the whole port: same inputs from the same state, same simulation, tick by
//! tick.
//!
//! A **fixture** is one recorded session: where the run starts, the input of every frame, and what the
//! state was expected to be while it ran. [`verify`] replays it and stops at the **first** tick that
//! disagrees, because the first disagreement is the bug and everything after it is noise. What comes
//! back is one line, with a tick number and two values:
//!
//! ```text
//! tick 413: ball.vel[1] -58.94952 (0xc26bcc4f) vs -58.949516 (0xc26bcc4e)
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
//!
//! A fixture is a text file ([`Fixture::parse`], [`Fixture::to_text`]) so a recorded session can be
//! stored and replayed later. Those files hold values read out of the game, so they are game data:
//! they live in the git-ignored `context/`, never in the repository — see `tests/replay.rs` for how a
//! test finds them and what it does when they are absent.

#![deny(clippy::float_arithmetic)] // a comparison does no arithmetic on the values it compares

use core::fmt;

use crate::hash::{self, Dump, Leaf};
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
/// A watch is one column of a capture: `./kit record`'s `ball_pos.y` becomes the port's `ball.pos[1]`,
/// whose name is a path through the state's walk (see [`crate::hash`]). Samples need not be dense —
/// a capture with `every = 4`, or with rows gated by a `when` condition, is sparse and that is fine.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    /// The path of the field in the state's walk, exactly as a [`Dump`] prints it: `ball.vel[1]`,
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
}

impl fmt::Display for Divergence {
    /// `tick 413: ball.vel[1] -58.94952 (0xc26bcc4f) vs -58.949516 (0xc26bcc4e)` — expected first (the
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
        }
    }
}

impl std::error::Error for Divergence {}

/// **Replay a fixture.** Returns the final state, or the first tick that disagreed.
///
/// Every tick of the fixture is checked: the watches that cover it (which name their field when they
/// differ) and then its hash. A tick with neither is simulated and not examined — which is normal,
/// since a capture of the original cannot produce a hash.
///
/// Cost: one [`Dump`] on the ticks that have watch samples, nothing but the hash on the rest, so a
/// fixture with four watched fields and a hash per frame runs at a few hundred thousand ticks a second.
pub fn verify(f: &Fixture) -> Result<Sim, Divergence> {
    let mut sim = f.initial();
    // One cursor per watch into its samples, advanced as the run passes them. Keeps the per-tick cost
    // proportional to the watches, not to the length of the recording.
    let mut cursor = vec![0usize; f.watches.len()];

    check_watches(f, &mut cursor, 0, &sim)?;
    for (k, &input) in f.inputs.iter().enumerate() {
        sim.tick(input);
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
    let mut sim = f.initial();
    for &input in f.inputs.iter().take(tick as usize) {
        sim.tick(input);
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
    let mut sim = Sim::new(seed);
    {
        // A path that is not in the walk is a typo, and a watch of a typo would pass for ever.
        let start = Dump::of(&sim);
        for p in paths {
            if start.get(p).is_none() {
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
        start: None,
        inputs: inputs.to_vec(),
        hashes,
        watches,
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
    /// watch 413 ball.vel[1] float 0x3fc00000  # tick, path, kind, value
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
/// lane (`ball_pos.x`), so turning a capture into a fixture is a list of "this column is that field of
/// my state":
///
/// ```
/// # use game_sim::replay::Capture;
/// let csv = "frame,ball_pos.x,ball_pos.y,laps\n\
///            101234,0.5,3,0\n\
///            101235,1,5.85,0\n";
/// let cap = Capture::parse(csv).expect("the recorder's own format");
/// let watches = vec![
///     cap.float("ball_pos.y", "ball.pos[1]").unwrap(),
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
