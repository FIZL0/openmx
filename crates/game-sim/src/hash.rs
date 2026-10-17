//! The state hash: one walk over the whole simulation state, read by two different sinks.
//!
//! Every port after this one is checked by comparing hashes, so this module has exactly one job: turn a
//! simulation state into 64 bits in a way that is **the same on every machine** and, when two states
//! disagree, say *which field* disagreed first.
//!
//! # The two modes
//!
//! A state type describes itself once, by implementing [`State::walk`] — a list of named leaves in a
//! fixed order. That one description feeds both modes, so they can never drift apart:
//!
//! ```
//! use game_sim::{Input, Sim, hash};
//! use ps2_float::Vf;
//!
//! let mut a = Sim::new(1);
//! let mut b = Sim::new(1);
//! for _ in 0..10 {
//!     a.tick(Input::default());
//!     b.tick(Input::default());
//! }
//! assert_eq!(hash::of(&a), hash::of(&b));          // fast mode: 64 bits, per tick
//! assert!(hash::first_difference(&a, &b).is_none());
//!
//! // A divergence, as a port would produce one: half a unit of sideways drift on the bike.
//! b.rider.ride.chassis.vel = Vf::new(0.5, 0.0, 0.0, 0.0);
//! let why = hash::first_difference(&a, &b).expect("the hashes no longer agree");
//! assert_eq!(why.path(), "bike.vel[0]");           // dump mode: which field, and both values
//! println!("{why}");
//! ```
//!
//! The intended use is: hash every tick (cheap, no allocation), and the moment a tick's hash differs
//! from the recorded one, run [`first_difference`] on that tick's two states to get the one-line reason.
//!
//! # Rules this module follows, and why
//!
//! - **Integers only.** FNV-1a over little-endian bytes, via [`crate::state_hash`]. No float
//!   arithmetic (the module denies the lint), no `f64`, no host `libm`: those are exactly the things
//!   that differ between x86-64, aarch64 and wasm32, and the hash is worthless the day it differs
//!   between two machines running the same port.
//! - **Floats are hashed as bit patterns.** `0.0` and `-0.0` are different states, and a NaN payload is
//!   part of the state, because on the PS2 it is: the FPU has no NaN but the VU lanes carry whatever
//!   bits were loaded. A port that has drifted to `-0.0` has drifted.
//! - **No `HashMap`, no pointers, no lengths that depend on an allocation.** Anything whose order or
//!   value varies between two runs of the same binary must not reach a sink. Lists are walked in index
//!   order and their length is hashed with them ([`Walk::list`]).
//! - **Field names are *not* in the hash.** Only values, in walk order. So renaming a field keeps the
//!   hashes valid (and the dumps readable), while adding, removing or reordering one changes them —
//!   which is correct, because that is a change of state, and the per-tick hash constants in the tests
//!   are then expected to move.
//!
//! # The one hazard
//!
//! A field that is in [`Sim`] but not in its `walk` is invisible to every test in the project. There is
//! no way to catch that by reflection in Rust, so it is caught two cheaper ways: `tests/hash.rs` asserts
//! the exact list of paths the walk produces, and asserts `size_of::<Sim>()`, so growing the state
//! without extending the walk fails a test instead of silently hashing to the same value.

#![deny(clippy::float_arithmetic)] // a hash does no arithmetic on the values it hashes

use core::fmt;

use ps2_float::{F, Vf};

use crate::{Input, Sim, state_hash};

/// A type whose entire state goes into the hash.
///
/// Implement it by listing the fields in a fixed order — the order is part of the hash, so keep it the
/// declaration order of the struct and the port will read the same way the dump does.
pub trait State {
    /// Describe every field, once, to whichever sink is walking.
    fn walk(&self, w: &mut Walk<'_>);
}

/// One leaf of the state: the smallest thing that is hashed and printed as a unit.
///
/// Integers carry their width because the width is part of the hash: a `u8` field that becomes a `u32`
/// field is a different state layout and must not hash the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    /// An unsigned integer, hashed as its low `width` little-endian bytes.
    Uint { value: u64, width: u8 },
    /// A signed integer, hashed as the low `width` little-endian bytes of its two's complement.
    Int { value: i64, width: u8 },
    /// A single-precision float, hashed and printed from its raw pattern. Never from its value.
    Float(u32),
    /// A flag, hashed as one byte.
    Bool(bool),
    /// A run of bytes that is not a number — a buffer, a name, packed flags. Hashed in full; summarised
    /// in a dump by its length and its own digest, so a dump of a state with a 2 KiB buffer in it is
    /// still something a human can read.
    Blob { len: usize, digest: u64 },
}

impl Leaf {
    /// Fold this leaf into a running hash. The only place the byte encoding of the state is decided.
    pub fn hash_into(self, h: u64) -> u64 {
        match self {
            Leaf::Uint { value, width } => state_hash(&value.to_le_bytes()[..width as usize], h),
            Leaf::Int { value, width } => state_hash(&value.to_le_bytes()[..width as usize], h),
            Leaf::Float(bits) => state_hash(&bits.to_le_bytes(), h),
            Leaf::Bool(b) => state_hash(&[b as u8], h),
            // Length first, so a blob cannot be confused with a shorter one that shares a digest.
            Leaf::Blob { len, digest } => {
                let h = state_hash(&(len as u64).to_le_bytes(), h);
                state_hash(&digest.to_le_bytes(), h)
            }
        }
    }
}

impl fmt::Display for Leaf {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Leaf::Uint { value, .. } => write!(w, "{value}"),
            Leaf::Int { value, .. } => write!(w, "{value}"),
            // Both forms: the decimal is what a human compares, the pattern is what actually differed.
            Leaf::Float(bits) => write!(w, "{} (0x{bits:08x})", f32::from_bits(bits)),
            Leaf::Bool(b) => write!(w, "{b}"),
            Leaf::Blob { len, digest } => write!(w, "{len} bytes (0x{digest:016x})"),
        }
    }
}

/// One step of a field's path: `bike.vel[2]` is `[Name("bike"), Name("vel"), Index(2)]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seg {
    Name(&'static str),
    Index(usize),
}

/// A path as a human reads it: `players[1].bike.wheel[0].spin`.
pub fn path_string(path: &[Seg]) -> String {
    let mut s = String::new();
    for seg in path {
        match *seg {
            Seg::Name(n) => {
                if !s.is_empty() {
                    s.push('.');
                }
                s.push_str(n);
            }
            Seg::Index(i) => {
                use fmt::Write;
                let _ = write!(s, "[{i}]");
            }
        }
    }
    s
}

/// Where a walk's leaves go. Two implementations: the hash, and the dump.
///
/// A sink sees the path of every leaf but is free to ignore it — the hash does, which is why hashing
/// allocates nothing and costs one FNV round per byte.
pub trait Sink {
    fn leaf(&mut self, path: &[Seg], leaf: Leaf);
}

/// The walk a [`State`] talks to: it holds the current path and the sink.
pub struct Walk<'a> {
    sink: &'a mut dyn Sink,
    path: Vec<Seg>,
}

impl<'a> Walk<'a> {
    /// Walk `state` into `sink`.
    pub fn new(sink: &'a mut dyn Sink) -> Walk<'a> {
        Walk {
            sink,
            // Deep enough for the nesting a PS2 game's state reaches, so a walk allocates once.
            path: Vec::with_capacity(8),
        }
    }

    fn push(&mut self, name: &'static str, leaf: Leaf) {
        self.path.push(Seg::Name(name));
        self.sink.leaf(&self.path, leaf);
        self.path.pop();
    }

    pub fn u8(&mut self, name: &'static str, v: u8) {
        self.push(name, Leaf::Uint { value: v as u64, width: 1 });
    }
    pub fn u16(&mut self, name: &'static str, v: u16) {
        self.push(name, Leaf::Uint { value: v as u64, width: 2 });
    }
    pub fn u32(&mut self, name: &'static str, v: u32) {
        self.push(name, Leaf::Uint { value: v as u64, width: 4 });
    }
    pub fn u64(&mut self, name: &'static str, v: u64) {
        self.push(name, Leaf::Uint { value: v, width: 8 });
    }
    pub fn i8(&mut self, name: &'static str, v: i8) {
        self.push(name, Leaf::Int { value: v as i64, width: 1 });
    }
    pub fn i16(&mut self, name: &'static str, v: i16) {
        self.push(name, Leaf::Int { value: v as i64, width: 2 });
    }
    pub fn i32(&mut self, name: &'static str, v: i32) {
        self.push(name, Leaf::Int { value: v as i64, width: 4 });
    }
    pub fn i64(&mut self, name: &'static str, v: i64) {
        self.push(name, Leaf::Int { value: v, width: 8 });
    }
    pub fn bool(&mut self, name: &'static str, v: bool) {
        self.push(name, Leaf::Bool(v));
    }

    /// An EE FPU scalar.
    pub fn f(&mut self, name: &'static str, v: F) {
        self.push(name, Leaf::Float(v.to_bits()));
    }

    /// A VU0 vector: four leaves, `name[0]`..`name[3]`, in register order `x y z w`. All four lanes
    /// always, including a `w` the simulation ignores — an ignored lane that differs is still a
    /// divergence, and finding out early is the whole point.
    pub fn vf(&mut self, name: &'static str, v: Vf) {
        self.path.push(Seg::Name(name));
        for (i, bits) in v.to_bits().into_iter().enumerate() {
            self.path.push(Seg::Index(i));
            self.sink.leaf(&self.path, Leaf::Float(bits));
            self.path.pop();
        }
        self.path.pop();
    }

    /// Raw bytes, hashed in full and summarised in a dump (see [`Leaf::Blob`]).
    pub fn blob(&mut self, name: &'static str, bytes: &[u8]) {
        self.push(
            name,
            Leaf::Blob { len: bytes.len(), digest: state_hash(bytes, 0) },
        );
    }

    /// A nested state value: its fields come out as `name.<field>`.
    pub fn field(&mut self, name: &'static str, v: &impl State) {
        self.path.push(Seg::Name(name));
        v.walk(self);
        self.path.pop();
    }

    /// A nested group written inline, for a few fields that do not deserve their own type:
    /// `w.group("score", |w| { w.u32("laps", laps); w.u32("points", points) })`.
    pub fn group(&mut self, name: &'static str, body: impl FnOnce(&mut Walk<'a>)) {
        self.path.push(Seg::Name(name));
        body(self);
        self.path.pop();
    }

    /// A list of states: `name.len`, then `name[0]`, `name[1]`, …
    ///
    /// The length is hashed first, so a list that grew cannot hash the same as one whose extra entries
    /// happen to be zero.
    pub fn list(&mut self, name: &'static str, items: &[impl State]) {
        self.path.push(Seg::Name(name));
        self.push("len", Leaf::Uint { value: items.len() as u64, width: 8 });
        for (i, it) in items.iter().enumerate() {
            self.path.push(Seg::Index(i));
            it.walk(self);
            self.path.pop();
        }
        self.path.pop();
    }
}

/// The fast sink: folds every leaf into one 64-bit value and keeps no path.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hasher {
    h: u64,
}

impl Hasher {
    /// A hasher continuing from `seed` (`0` starts at FNV's offset basis — see [`crate::state_hash`]).
    pub fn with_seed(seed: u64) -> Hasher {
        Hasher { h: seed }
    }
    /// The hash so far.
    pub fn finish(&self) -> u64 {
        self.h
    }
}

impl Sink for Hasher {
    fn leaf(&mut self, _path: &[Seg], leaf: Leaf) {
        self.h = leaf.hash_into(self.h);
    }
}

/// **The state in 64 bits.** The one function everything after this task is checked with.
pub fn of(state: &impl State) -> u64 {
    let mut h = Hasher::default();
    state.walk(&mut Walk::new(&mut h));
    h.finish()
}

/// Like [`of`], but continuing a running hash — for hashing a state into a sequence of them.
pub fn of_with_seed(state: &impl State, seed: u64) -> u64 {
    let mut h = Hasher::with_seed(seed);
    state.walk(&mut Walk::new(&mut h));
    h.finish()
}

/// One leaf of a dump: where it is, and what it holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub path: String,
    pub leaf: Leaf,
}

impl fmt::Display for Field {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(w, "{} = {}", self.path, self.leaf)
    }
}

/// The slow sink: every field with its path, in walk order. This is the dump mode.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dump {
    pub fields: Vec<Field>,
}

impl Sink for Dump {
    fn leaf(&mut self, path: &[Seg], leaf: Leaf) {
        self.fields.push(Field { path: path_string(path), leaf });
    }
}

impl Dump {
    /// Every field of `state`, in walk order.
    pub fn of(state: &impl State) -> Dump {
        let mut d = Dump::default();
        state.walk(&mut Walk::new(&mut d));
        d
    }

    /// The hash of the state this dump came from. Identical to [`of`] on the same state, by
    /// construction: same walk, same leaves, same order.
    pub fn hash(&self) -> u64 {
        self.fields.iter().fold(0, |h, f| f.leaf.hash_into(h))
    }

    /// One field by path, for a test or a watch spec that cares about a single value.
    pub fn get(&self, path: &str) -> Option<Leaf> {
        self.fields.iter().find(|f| f.path == path).map(|f| f.leaf)
    }

    /// The first field that differs, in walk order, or `None` if the two dumps are identical.
    ///
    /// Walk order matters: the first disagreement is the bug and everything after it is noise, which is
    /// why this returns one field and not a diff of two files.
    pub fn first_difference(&self, other: &Dump) -> Option<Diff> {
        let n = self.fields.len().max(other.fields.len());
        for i in 0..n {
            match (self.fields.get(i), other.fields.get(i)) {
                (Some(a), Some(b)) if a == b => continue,
                (Some(a), Some(b)) if a.path == b.path => {
                    return Some(Diff::Value { index: i, path: a.path.clone(), left: a.leaf, right: b.leaf });
                }
                (a, b) => {
                    return Some(Diff::Shape { index: i, left: a.cloned(), right: b.cloned() });
                }
            }
        }
        None
    }
}

impl fmt::Display for Dump {
    /// One `path = value` per line: what you print when a replay fails and you want to read the state.
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, f) in self.fields.iter().enumerate() {
            if i > 0 {
                writeln!(w)?;
            }
            write!(w, "{f}")?;
        }
        Ok(())
    }
}

/// Why two states hash differently, as one line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Diff {
    /// The same field, two values. The ordinary case, and the one a port produces.
    Value {
        /// Position in walk order, so two runs can be compared field by field.
        index: usize,
        path: String,
        left: Leaf,
        right: Leaf,
    },
    /// The two states are not the same shape: one has a field the other does not, or they are in a
    /// different order. Not a divergence in the simulation — a divergence in the walk, which happens
    /// when a dump recorded by an older build is compared against a newer one.
    Shape {
        index: usize,
        left: Option<Field>,
        right: Option<Field>,
    },
}

impl Diff {
    /// The path of the field that differs, or of whichever side has one.
    pub fn path(&self) -> &str {
        match self {
            Diff::Value { path, .. } => path,
            Diff::Shape { left, right, .. } => match (left, right) {
                (Some(f), _) | (None, Some(f)) => &f.path,
                (None, None) => "",
            },
        }
    }

    /// Position in walk order.
    pub fn index(&self) -> usize {
        match *self {
            Diff::Value { index, .. } | Diff::Shape { index, .. } => index,
        }
    }
}

impl fmt::Display for Diff {
    /// `bike.vel[1] 2.85 (0x4036...) vs 2.8500001 (0x4036...)` — the whole failure, on one line, which
    /// is what a replay harness prints with the tick number in front of it.
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Diff::Value { path, left, right, .. } => write!(w, "{path} {left} vs {right}"),
            Diff::Shape { index, left, right } => {
                write!(w, "state shape differs at field {index}: ")?;
                match (left, right) {
                    (Some(a), Some(b)) => write!(w, "{a} vs {b}"),
                    (Some(a), None) => write!(w, "{a} vs nothing"),
                    (None, Some(b)) => write!(w, "nothing vs {b}"),
                    (None, None) => write!(w, "neither side has a field"),
                }
            }
        }
    }
}

/// **The dump mode:** the first field in which two states differ, or `None` if they are identical.
///
/// Two states whose hashes agree have no difference here (barring a 2⁻⁶⁴ collision), so the normal use
/// is to compare hashes every tick and call this only on the tick that failed.
pub fn first_difference(left: &impl State, right: &impl State) -> Option<Diff> {
    Dump::of(left).first_difference(&Dump::of(right))
}

/// The hash after every tick of a run: one entry per input frame, which is exactly what a replay
/// fixture holds and what a recording is compared against.
pub fn trace(sim: &mut Sim, inputs: &[Input]) -> Vec<u64> {
    inputs
        .iter()
        .map(|&input| {
            sim.tick(input);
            of(sim)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------
// The port's own states. Each walk lists the struct's fields in declaration order; keep it that way, so
// that reading the struct and reading the dump are the same exercise.
// ---------------------------------------------------------------------------------------------------

impl State for Sim {
    /// The simulation's own walk, which is what every fixture in the project is compared through.
    ///
    /// `bike.pos` and `bike.vel` are first because they are the two quads a capture of the original
    /// records — row 3 of the vehicle's world transform and the body's velocity — and a watch spec
    /// written against a CSV cannot be asked to know where in the ride they live. They are
    /// deliberately the *same words* the ride's own walk carries further down
    /// (`rider.ride.origin` and `rider.ride.chassis.vel`): two leaves of the walk read one field of
    /// the state, which costs two FNV rounds and buys a name a recording can be written against.
    fn walk(&self, w: &mut Walk<'_>) {
        w.u64("ticks", self.ticks);
        w.u32("rng", self.rng);
        w.group("bike", |w| {
            w.vf("pos", self.bike_pos());
            w.vf("vel", self.bike_vel());
        });
        w.field("input", &self.input);
        w.field("rider", &self.rider);
    }
}

impl State for crate::physics::Ball {
    fn walk(&self, w: &mut Walk<'_>) {
        w.vf("pos", self.pos);
        w.vf("vel", self.vel);
        w.f("spin", self.spin);
    }
}

impl State for crate::rng::Rng {
    /// One 64-bit word. The generator is state like any other: a port that produces the right numbers
    /// from the wrong state word diverges on the next call, not this one.
    fn walk(&self, w: &mut Walk<'_>) {
        w.u64("state", self.state());
    }
}

impl State for Input {
    /// Not simulation state, but hashed the same way so a fixture can prove it is feeding the run the
    /// frames it recorded, rather than frames that shifted by one.
    fn walk(&self, w: &mut Walk<'_>) {
        w.u16("buttons", self.buttons);
        w.i8("stick.x", self.stick.0);
        w.i8("stick.y", self.stick.1);
    }
}
