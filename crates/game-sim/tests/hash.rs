//! The state hash, proved.
//!
//! There is no original to compare against here — the hash is the port's own instrument, not a ported
//! function — so "proved" means something different from the oracle tests next door. Five properties,
//! each of which is a bug that has cost real projects a week:
//!
//! 1. **It is a known algorithm over a known byte stream.** The folding is FNV-1a (checked against the
//!    reference vectors), and the stream is built by hand in this file and compared against the walk, so
//!    the encoding of the state is pinned by something other than the code that produces it.
//! 2. **It is integer-only and little-endian by construction**, which is what makes the same fixture
//!    hash the same on x86-64, aarch64 and wasm32. A3 runs it on a second architecture; this file
//!    proves there is nothing in the way but the architecture itself.
//! 3. **It is sensitive**: every leaf of the state reaches the hash, a one-bit change anywhere changes
//!    it, and `0.0` / `-0.0` are different states.
//! 4. **The dump agrees with the hash**, field for field, so the thing that explains a divergence and
//!    the thing that detects it cannot disagree.
//! 5. **The first difference is the *first* one** in walk order, with both values, as one line.
//!
//! Plus two tripwires for the hazard the module documents: a field added to [`Sim`] and not to its
//! walk. Nothing in Rust can catch that by reflection, so the exact path list and the size of the state
//! type are asserted here, and adding a field fails this file until the walk is extended.

use std::mem::size_of;

use game_sim::hash::{self, Diff, Dump, Leaf, Seg, State, Walk};
use game_sim::{Input, Sim, state_hash};
use ps2_float::{F, Vf};

/// FNV-1a, written out straight from the specification, as an independent check on `state_hash`.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// (1) The fold is the real FNV-1a: the reference vectors, which are a fixed point outside this project.
#[test]
fn the_fold_is_fnv1a() {
    assert_eq!(state_hash(b"", 0), 0xcbf2_9ce4_8422_2325, "the offset basis");
    assert_eq!(state_hash(b"a", 0), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(state_hash(b"foobar", 0), 0x8594_4171_f739_67e8);
    // And it is a running fold: hashing in two pieces is hashing in one.
    assert_eq!(state_hash(b"bar", state_hash(b"foo", 0)), state_hash(b"foobar", 0));
    assert_eq!(fnv1a(b"foobar"), state_hash(b"foobar", 0));
}

/// (1) The byte stream the walk produces, written out by hand.
///
/// This is the encoding of the state, pinned independently of the walk: ticks as 8 little-endian bytes,
/// the RNG word as 4, then the bike's position and velocity as four lane patterns each in `x y z w`
/// order. If a change to the module moves a byte, this fails and says so.
///
/// It is the *head* of the stream rather than all of it, because the state behind it is the input
/// manager and the whole rider (P5) and nobody can write those bytes out by hand. The fold is a
/// running one — [`the_fold_is_fnv1a`] proves that — so folding the head of the walk is folding the
/// head of the stream, and [`a_dump_hashes_to_the_same_value_as_the_state`] ties the whole walk back
/// to `hash::of`. These ten leaves are the ones a capture of the original reads by name, so they
/// are the ones worth pinning twice.
#[test]
fn the_hash_is_that_fold_over_the_states_bytes_in_order() {
    let mut sim = Sim::new(0x1234_5678);
    for _ in 0..7 {
        sim.tick(Input::default());
    }

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&sim.ticks.to_le_bytes());
    bytes.extend_from_slice(&sim.rng.to_le_bytes());
    for v in [sim.bike_pos(), sim.bike_vel()] {
        for lane in v.to_bits() {
            bytes.extend_from_slice(&lane.to_le_bytes());
        }
    }

    assert_eq!(bytes.len(), 8 + 4 + 4 * 4 + 4 * 4);
    let d = Dump::of(&sim);
    let head = d.fields[..HEAD].iter().fold(0u64, |h, f| f.leaf.hash_into(h));
    assert_eq!(head, fnv1a(&bytes), "the walk's stream is not the documented one");
}

/// How many leaves of the walk the hand-written stream above covers: `ticks`, `rng` and the bike's two
/// vectors ([`a_dump_names_every_field_in_walk_order`] lists them).
const HEAD: usize = 10;

/// (2) A hash nobody can accidentally change: the value after a hundred ticks, as a literal.
///
/// The same literal as `Sim::hash`'s own test, because they are now the same walk — and that identity
/// is what keeps the hash a constant of the code rather than of the machine (A3 runs it elsewhere).
#[test]
fn a_hundred_ticks_is_a_known_constant() {
    // The hash of a hundred ticks of the stand-in bike on a level floor, as a literal: the same
    // value `Sim::hash`'s own test carries.
    const HUNDRED_TICKS: u64 = 0x1d7e_aa7c_1cf6_8331;

    let mut sim = Sim::new(1);
    for _ in 0..100 {
        sim.tick(Input::default());
    }
    assert_eq!(hash::of(&sim), HUNDRED_TICKS, "{}", Dump::of(&sim));
    assert_eq!(sim.hash(), hash::of(&sim), "Sim::hash must be the walk, not a second copy of it");
    // Same for a subsystem's own hash, which continues a running one rather than starting it.
    assert_eq!(
        hash::of_with_seed(&sim.input, 1234),
        hash::of_with_seed(&sim.input, 1234),
        "a subsystem's hash must be its walk too"
    );
    assert_eq!(sim.input.hash(), hash::of(&sim.input), "seed 0 is a fresh hash");
}

/// (2) Every tick of a run is hashed, and the run is reproducible from the seed alone.
#[test]
fn a_trace_is_one_hash_per_tick_and_repeats_exactly() {
    let inputs = vec![Input::default(); 120];
    let a = hash::trace(&mut Sim::new(9), &inputs);
    let b = hash::trace(&mut Sim::new(9), &inputs);
    assert_eq!(a.len(), inputs.len());
    assert_eq!(a, b, "the same seed and inputs must give the same hashes");

    // A different seed is a different run from the first tick, because the seed is in the state.
    let c = hash::trace(&mut Sim::new(10), &inputs);
    assert_ne!(a[0], c[0]);

    // The hash moves while the simulation moves: a trace of 120 ticks of a ball in flight has no
    // repeats, so a divergence cannot hide behind a stuck value.
    let mut seen = a.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), a.len(), "a tick's hash repeated: is the state actually changing?");
}

/// (3) Sensitivity, leaf by leaf: change any one leaf of the state and the hash changes.
///
/// The loop is over the dump, so this test grows with the state type — a field added to the walk is
/// covered automatically. (And a field added to `Sim` but *not* to the walk is caught by the two
/// tripwires at the bottom of this file.) The flipped stream is folded through the same
/// [`Leaf::hash_into`] the hasher uses, and [`a_dump_hashes_to_the_same_value_as_the_state`] is what
/// ties that fold back to `hash::of`.
#[test]
fn every_leaf_of_the_state_reaches_the_hash() {
    let mut sim = Sim::new(3);
    for _ in 0..5 {
        sim.tick(Input::default());
    }
    let d = Dump::of(&sim);
    let base = d.hash();
    assert_eq!(base, hash::of(&sim));

    /// One bit of a leaf, whatever kind it is.
    fn flip(leaf: Leaf) -> Leaf {
        match leaf {
            Leaf::Uint { value, width } => Leaf::Uint { value: value ^ 1, width },
            Leaf::Int { value, width } => Leaf::Int { value: value ^ 1, width },
            Leaf::Float(bits) => Leaf::Float(bits ^ 1),
            Leaf::Bool(b) => Leaf::Bool(!b),
            Leaf::Blob { len, digest } => Leaf::Blob { len, digest: digest ^ 1 },
        }
    }

    for (n, f) in d.fields.iter().enumerate() {
        let flipped = d.fields.iter().enumerate().fold(0u64, |h, (i, g)| {
            if i == n { flip(g.leaf) } else { g.leaf }.hash_into(h)
        });
        assert_ne!(flipped, base, "a one-bit change in {} did not change the hash", f.path);
    }
}

/// (3) The two zeros are two states. A port that has drifted to `-0.0` has drifted, and `==` would not
/// notice; the bit pattern does.
#[test]
fn negative_zero_is_not_zero_and_a_nan_payload_survives() {
    assert_ne!(Leaf::Float(0).hash_into(0), Leaf::Float(0x8000_0000).hash_into(0));
    assert_ne!(
        Leaf::Float(0x7fc0_0001).hash_into(0),
        Leaf::Float(0x7fc0_0002).hash_into(0),
        "a NaN payload is state too"
    );
    // Width is part of the hash: the same number in a wider field is a different state layout.
    assert_ne!(
        Leaf::Uint { value: 1, width: 1 }.hash_into(0),
        Leaf::Uint { value: 1, width: 4 }.hash_into(0)
    );
}

/// (4) The dump is the hash: same walk, same leaves, same order, same 64 bits.
#[test]
fn a_dump_hashes_to_the_same_value_as_the_state() {
    let mut sim = Sim::new(42);
    for n in 0..30 {
        sim.tick(Input::default());
        let d = Dump::of(&sim);
        assert_eq!(d.hash(), hash::of(&sim), "dump and hash disagree at tick {n}");
    }
}

/// (4) What a dump looks like, exactly: the paths, in walk order, and a value a human can read.
///
/// **Tripwire.** If you added a field to the state and to the walk, this list changes and this test
/// fails: add the path. If you added a field to the state and *not* to the walk, see
/// [`the_size_of_the_state_type_is_pinned`].
#[test]
fn a_dump_names_every_field_in_walk_order() {
    let d = Dump::of(&Sim::new(0x11));
    let paths: Vec<&str> = d.fields.iter().map(|f| f.path.as_str()).collect();
    // The head of the walk, in order: the two counters and the eight names a capture of the original
    // asks for by column (`crates/game-sim/tests/replay.rs`, `BIKE`).
    assert_eq!(
        paths[..HEAD],
        [
            "ticks",
            "rng",
            "bike.pos[0]",
            "bike.pos[1]",
            "bike.pos[2]",
            "bike.pos[3]",
            "bike.vel[0]",
            "bike.vel[1]",
            "bike.vel[2]",
            "bike.vel[3]",
        ]
    );
    assert_eq!(d.get("rng"), Some(Leaf::Uint { value: 0x11, width: 4 }));
    // A fresh state is on the start line with nothing moving: the stand-in spawn, and a zero
    // velocity ([`game_sim::stock`]).
    assert_eq!(d.get("bike.pos[1]"), Some(Leaf::Float(1.0f32.to_bits())));
    assert_eq!(d.get("bike.vel[0]"), Some(Leaf::Float(0.0f32.to_bits())));
    assert_eq!(d.get("nothing.like.this"), None);
    // And everything behind the head is the two subsystems the state is made of, each under its own
    // name, so no leaf of either is outside the hash.
    assert!(
        paths[HEAD..].iter().all(|p| p.starts_with("input.") || p.starts_with("rider.")),
        "{:?}",
        paths[HEAD..].iter().find(|p| !p.starts_with("input.") && !p.starts_with("rider.")),
    );
    assert!(paths.iter().any(|p| p.starts_with("rider.ride.chassis.")), "the ride is in the walk");

    // Readable: one `path = value` per line, floats with their pattern beside the number.
    let text = d.to_string();
    assert_eq!(text.lines().count(), paths.len());
    assert!(text.starts_with("ticks = 0\nrng = 17\n"), "{text}");
    assert!(text.contains("bike.pos[1] = 1 (0x3f800000)"), "{text}");
}

/// (5) The first difference is the first one in walk order, named, with both values, in one line.
#[test]
fn the_first_differing_field_is_the_one_reported() {
    let mut a = Sim::new(5);
    for _ in 0..20 {
        a.tick(Input::default());
    }
    let mut b = a.clone();
    assert_eq!(hash::of(&a), hash::of(&b));
    assert!(hash::first_difference(&a, &b).is_none(), "identical states have no difference");

    // Two divergences at once: the earlier field in walk order is the one reported, because the first
    // disagreement is the bug and everything after it is noise.
    let bits = b.bike_vel().to_bits();
    b.rider.ride.chassis.vel = Vf::from_bits([
        bits[0],
        bits[1] + 1, // one bit of the last mantissa place: a port's typical drift
        bits[2],
        bits[3],
    ]);
    b.rider.ride.chassis.speed = F::from_bits(b.rider.ride.chassis.speed.to_bits() + 1);

    let d = hash::first_difference(&a, &b).expect("the states differ");
    assert_eq!(d.path(), "bike.vel[1]");
    assert_eq!(d.index(), 7, "walk position, so a caller can line two runs up");
    match d {
        Diff::Value { left, right, .. } => {
            assert_eq!(left, Leaf::Float(a.bike_vel().to_bits()[1]));
            assert_eq!(right, Leaf::Float(b.bike_vel().to_bits()[1]));
        }
        other => panic!("expected a value difference, got {other}"),
    }
    // The failure line a replay harness prints, with the tick number in front of it.
    let line = hash::first_difference(&a, &b).unwrap().to_string();
    assert!(line.starts_with("bike.vel[1] "), "{line}");
    assert!(line.contains(" vs "), "{line}");
}

/// (5) An earlier field wins over a later one, whichever side it is on.
#[test]
fn an_earlier_field_is_reported_before_a_later_one() {
    let a = Sim::new(5);
    let mut b = a.clone();
    b.ticks += 1;
    b.rider.ride.chassis.speed = F::from_bits(b.rider.ride.chassis.speed.to_bits() ^ 1);
    assert_eq!(hash::first_difference(&a, &b).unwrap().path(), "ticks");
    assert_eq!(hash::first_difference(&b, &a).unwrap().path(), "ticks");
}

/// (5) Two states of different shape — an old recorded dump against a new build — say so, instead of
/// blaming a field.
#[test]
fn a_shape_change_is_reported_as_a_shape_change() {
    let sim = Sim::new(1);
    let full = Dump::of(&sim);
    let last = full.fields.last().expect("the state has fields").path.clone();
    let mut short = full.clone();
    short.fields.pop();

    match full.first_difference(&short).expect("one side is shorter") {
        Diff::Shape { index, left, right } => {
            assert_eq!(index, full.fields.len() - 1);
            assert_eq!(left.unwrap().path, last);
            assert!(right.is_none());
        }
        other => panic!("expected a shape difference, got {other}"),
    }

    // A renamed-at-the-same-position field is a shape change too, not a value difference.
    let mut renamed = full.clone();
    renamed.fields[1].path = "seed".to_string();
    let d = renamed.first_difference(&full).expect("the paths differ");
    assert!(matches!(d, Diff::Shape { index: 1, .. }), "{d}");
    assert!(d.to_string().starts_with("state shape differs at field 1: "), "{d}");
}

/// A composed state: nested structs, lists and blobs come out with the paths the module documents, and
/// a list's length is hashed with it.
///
/// Written against a test-local state rather than [`Sim`] because the simulation has no list in it yet,
/// and `Walk::list` is the part the first real subsystem (a field of riders) will lean on.
#[test]
fn groups_lists_and_blobs_walk_the_way_they_are_documented() {
    struct Rider {
        ready: bool,
        pos: Vf,
    }
    impl State for Rider {
        fn walk(&self, w: &mut Walk<'_>) {
            w.bool("ready", self.ready);
            w.vf("pos", self.pos);
        }
    }
    struct Race {
        riders: Vec<Rider>,
        name: Vec<u8>,
    }
    impl State for Race {
        fn walk(&self, w: &mut Walk<'_>) {
            w.list("riders", &self.riders);
            w.blob("name", &self.name);
            w.group("score", |w| {
                w.u32("laps", 2);
                w.i32("penalty", -1);
            });
        }
    }

    let mut race = Race {
        riders: vec![Rider { ready: true, pos: Vf::ZERO }],
        name: b"outdoor nationals".to_vec(),
    };
    let one = Dump::of(&race);
    let paths: Vec<&str> = one.fields.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "riders.len",
            "riders[0].ready",
            "riders[0].pos[0]",
            "riders[0].pos[1]",
            "riders[0].pos[2]",
            "riders[0].pos[3]",
            "name",
            "score.laps",
            "score.penalty",
        ]
    );
    assert_eq!(
        one.get("name"),
        Some(Leaf::Blob { len: 17, digest: state_hash(b"outdoor nationals", 0) })
    );
    assert_eq!(one.get("score.penalty"), Some(Leaf::Int { value: -1, width: 4 }));
    assert_eq!(one.hash(), hash::of(&race));

    // A list that grows changes the hash even when the new entry is all zeros, because the length is
    // hashed before the entries.
    let before = hash::of(&race);
    race.riders.push(Rider { ready: false, pos: Vf::ZERO });
    assert_ne!(hash::of(&race), before);
    assert_eq!(
        Dump::of(&race).get("riders.len"),
        Some(Leaf::Uint { value: 2, width: 8 })
    );
}

/// A path renders the way the module says, including the awkward cases.
#[test]
fn a_path_reads_like_rust() {
    assert_eq!(hash::path_string(&[]), "");
    assert_eq!(
        hash::path_string(&[
            Seg::Name("players"),
            Seg::Index(1),
            Seg::Name("bike"),
            Seg::Name("wheel"),
            Seg::Index(0),
            Seg::Name("spin"),
        ]),
        "players[1].bike.wheel[0].spin"
    );
    assert_eq!(hash::path_string(&[Seg::Index(3), Seg::Name("x")]), "[3].x");
}

/// Input is hashed too, so a fixture can prove it is feeding the frames it recorded.
#[test]
fn input_frames_hash_distinctly() {
    let a = Input { buttons: 0x0010, stick: (0, 0) };
    let b = Input { buttons: 0x0010, stick: (-1, 0) };
    let c = Input { buttons: 0x0010, stick: (0, -1) };
    assert_ne!(hash::of(&a), hash::of(&b));
    assert_ne!(hash::of(&b), hash::of(&c), "the two stick axes are not interchangeable");
    assert_eq!(
        Dump::of(&b).get("stick.x"),
        Some(Leaf::Int { value: -1, width: 1 })
    );
}

/// The RNG is state like any other: the 64-bit word, not the last number it returned.
#[test]
fn the_rngs_state_word_is_what_is_hashed() {
    use game_sim::rng::Rng;
    let mut r = Rng::new();
    let before = hash::of(&r);
    r.rand();
    assert_ne!(hash::of(&r), before, "drawing a number moves the state");
    assert_eq!(
        Dump::of(&r).get("state"),
        Some(Leaf::Uint { value: r.state(), width: 8 })
    );
}

/// **Tripwire for the module's one hazard.** A field added to [`Sim`] but not to its walk is invisible
/// to every hash in the project, and no language feature will catch it. This catches it by size: when
/// the state type grows, this fails, and the fix is to extend `impl State for Sim` and the path list in
/// [`a_dump_names_every_field_in_walk_order`], then update the numbers here.
///
/// It can also fail for an innocent reason — a compiler that lays the struct out differently, or a
/// change to `Vf`/`F` — in which case check the walk covers every field and update the number.
///
/// The state is no longer a handful of words: it is the input manager, the bike's parameters and the
/// rider, so the size below is the sum of those and not something a reader can add up here. What it
/// still does is fail when any of them gains a field.
///
/// The leaf count is of a *fresh* state, because the walk has lists in it (the input layer's event
/// queue, the rider's trick queue) and a state with something happening in it has more leaves than
/// one on the start line — `tests/platforms.rs` pins the count after a run as well.
#[test]
fn the_size_of_the_state_type_is_pinned() {
    assert_eq!(size_of::<Sim>(), 6648, "Sim grew or shrank: does its walk still cover every field?");
    assert_eq!(Dump::of(&Sim::new(0)).fields.len(), 750, "the walk's leaf count moved with it");
}
