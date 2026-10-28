//! The replay harness, proved.
//!
//! Like the state hash next door, the harness is the port's own instrument rather than a ported
//! function, so there is no original to run it against — "proved" here means the properties that make
//! a replay test *trustworthy*. Every one of them is a way a replay harness can appear to work while
//! checking nothing, and most of them have happened to a real project:
//!
//! 1. **A fixture that agrees passes, and one that disagrees fails at the right tick** — with the
//!    first disagreement, not some later one.
//! 2. **The failure is one line**, with the tick, the field's path and both values, which is the whole
//!    point of the module: `tick 413: bike.vel[1] -58.94952 vs -58.949516`.
//! 3. **A watched field is named.** Where a tick has both a hash and a watch, the watch's field is
//!    reported, because `state hash a vs b` is a true statement that helps nobody.
//! 4. **Nothing passes vacuously.** A watch on a field that does not exist, a watch of the wrong kind,
//!    samples past the end of the run and a hash list longer than the run are all reported as the
//!    fixture's own problem rather than skipped — a fixture that checks less than it claims is the one
//!    failure a green test cannot show you.
//! 5. **The fixture file round-trips exactly**, floats included, so a recorded session replays the
//!    same a year later.
//! 6. **A `./kit record` CSV reads as watches**, with its sparse rows, its empty cells (which mean
//!    "not sampled", never zero) and its frame counter mapped to ticks.
//! 7. **An off-by-one in frame alignment is recognisable**: it fails at tick 1, not at tick 400.
//! 8. **A recorded pad sequence plays back as the pad it was.** A tape carries the report bytes and
//!    the measured `dt` of each frame rather than a button mask, so the port's own poll derives the
//!    controls, their strengths and the press timers from it — and a capture of real play becomes the
//!    frames the game read, in the order it read them, through the stateful input layer.
//!
//! 9. **A human's PCSX2 input recording reads as the pad the game was handed.** A `.p2m2` holds one
//!    pad entry per video field and no `dt`; the reader maps its eighteen bytes onto the port's report
//!    bytes (which the file's own pressure/bit redundancy proves), takes one field of each of the
//!    game's two-field ticks, and runs at the engine's clamped step.
//!
//! Plus the four tests that hook into the private side: every fixture file in `context/fixtures/` is
//! replayed, every capture in `context/capture/` is parsed, every capture that carries the pad
//! columns is played back through the port and compared against the controls the game recorded, and
//! every `.p2m2` in `context/recordings/` is played through the port. Those directories hold data read out
//! of the game, so they are never committed — the tests say what they found and pass when there is
//! nothing there, which is the only honest thing a test can do about a file it is not allowed to ship.
//! Run the long ones with:
//!
//! ```sh
//! cargo test -p game-sim --test replay -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use game_sim::archive::{Psr, Res};
use game_sim::hash::Leaf;
use game_sim::input::{Control, Report, CONTROL_MAP, CONTROLS as PORT_CONTROLS};
use game_sim::math::{self, Mat4};
use game_sim::motion::{Clips, Driver, Params, Pose, Request};
use game_sim::phy::Phy as BikePhy;
use game_sim::replay::{
    self, Capture, Fixture, PadFrame, PadTape, Playback, Reason, Sample, Value, Watch, Where,
};
use game_sim::rider::{Rider, Tables as RiderTables};
use game_sim::track::{Scene, Terrain};
use game_sim::trick::{Set, Trick};
use game_sim::world::{Ground, Place, Props, World};
use game_sim::noise::SmoothNoise;
use game_sim::{Input, Sim};
use ps2_float::{Acc, F, Vf};

/// A run with something happening in it: the stick wandering so the inputs are not all identical, long
/// enough that a divergence in the middle is a middle.
fn inputs(n: usize) -> Vec<Input> {
    (0..n)
        .map(|k| Input {
            buttons: (k as u16).wrapping_mul(0x1234) & 0x3fff,
            stick: ((k % 97) as i8 - 48, ((k * 7) % 61) as i8 - 30),
        })
        .collect()
}

/// The fields a fixture of this simulation watches: the bike's height, two lanes of its velocity and
/// the tick counter, which is the shape a capture of the original gives — a handful of columns, not
/// the whole state. They are four of the eight names `BIKE` maps a capture's columns onto.
const WATCHED: [&str; 4] = ["bike.pos[1]", "bike.vel[1]", "bike.vel[2]", "ticks"];

fn recorded(frames: usize) -> Fixture {
    replay::record("self-recorded", 0x1234_5678, &inputs(frames), &WATCHED)
        .expect("every watched path is in the walk")
}

// ---------------------------------------------------------------------------------------------------
// (1) It passes when it agrees, and fails at the first tick that does not
// ---------------------------------------------------------------------------------------------------

/// The quick fixture: a few hundred frames of the port against itself, run on every `./kit check`.
/// It proves no part of the original — only that the simulation still does what it did when this was
/// written, which is what catches an accidental change to a shared constant.
#[test]
fn a_fixture_the_port_recorded_replays_exactly() {
    let f = recorded(600);
    assert_eq!(f.hashes.len(), 600, "a hash per frame");
    assert_eq!(f.checks(), 600 + 4 * 601, "a hash per frame, and four fields per tick plus tick 0");
    let end = replay::verify(&f).expect("the port must agree with its own recording");
    assert_eq!(end.ticks, 600);
}

#[test]
fn a_changed_hash_fails_at_its_own_tick() {
    let mut f = recorded(600);
    f.watches.clear(); // hashes only: the state moved, and the harness cannot say which part
    f.hashes[412] ^= 1;
    let d = replay::verify(&f).expect_err("a changed hash must be noticed");
    assert_eq!(d.tick, 413, "hashes[k - 1] is the hash of tick k");
    assert!(matches!(d.reason, Reason::Hash { .. }), "{d}");
}

/// The first disagreement, not a later one: three ticks are wrong and the harness reports the earliest.
#[test]
fn the_first_disagreement_is_the_one_reported() {
    let mut f = recorded(600);
    f.watches.clear();
    for k in [500, 412, 7] {
        f.hashes[k] ^= 1;
    }
    let d = replay::verify(&f).expect_err("three changed hashes, one report");
    assert_eq!(d.tick, 8);
}

/// Nothing in the state may be invisible to a replay: a one-bit change to any *leaf* of the simulation
/// has to make some fixture fail. Checked by hashing the port's own run after perturbing each field in
/// turn — the mirror of `tests/hash.rs`'s sensitivity test, but through the harness.
#[test]
fn a_divergence_in_any_field_fails_the_replay() {
    let f = recorded(20);
    // A field the fixture watches is caught at tick 0, before a single input; a field only the hash
    // covers is caught by the first hash, at tick 1. Both are caught, which is the property — and the
    // difference is the argument for watching a few fields rather than none.
    /// What was perturbed, the tick the harness should notice it on, and how to perturb it.
    type Case = (&'static str, u64, fn(&mut Sim));
    /// One bit of one lane of a vector, leaving the other three alone.
    fn flip(v: Vf, lane: usize, bit: u32) -> Vf {
        let mut bits = v.to_bits();
        bits[lane] ^= 1 << bit;
        Vf::from_bits(bits)
    }
    let cases: [Case; 3] = [
        // The sideways lane of the velocity, which no watch names — not a lane of `bike.pos`, because
        // the transform's origin is written out of the chassis every frame and a bit put into it by
        // hand is gone before the first hash, while the same bit in the velocity is integrated. (The
        // watched case below catches `bike.pos` at tick 0, where nothing has run yet.) And bit 30
        // rather than bit 0: the bike starts at rest, and the last bit of a zero is a denormal the
        // PS2's FPU flushes back to zero, so that one perturbation the hardware itself deletes.
        ("bike.vel[0], a lane no watch names", 1, |s| {
            s.rider.ride.chassis.vel = flip(s.rider.ride.chassis.vel, 0, 30);
        }),
        ("the RNG word", 1, |s| s.rng ^= 1),
        ("bike.pos[1], which is watched", 0, |s| {
            s.rider.ride.origin = flip(s.rider.ride.origin, 1, 0);
        }),
    ];
    for (what, tick, perturb) in cases {
        let mut start = Sim::new(f.seed);
        perturb(&mut start);
        let moved = Fixture { start: Some(start), ..f.clone() };
        let Err(d) = replay::verify(&moved) else {
            panic!("a one-bit change to {what} must diverge");
        };
        assert_eq!(d.tick, tick, "{what}: {d}");
    }
}

// ---------------------------------------------------------------------------------------------------
// (2, 3) The failure is one line, and it names the field
// ---------------------------------------------------------------------------------------------------

/// The shape of a failure, which is the module's entire reason to exist:
///
/// ```text
/// tick 413: bike.vel[1] -58.94952 (0xc26bcc4f) vs -58.949516 (0xc26bcc4e)
/// ```
#[test]
fn a_watched_field_fails_as_one_readable_line() {
    let mut f = recorded(600);
    let w = f.watches.iter_mut().find(|w| w.path == "bike.vel[1]").expect("the watch is there");
    let s = &mut w.samples[413];
    assert_eq!(s.tick, 413);
    let Value::Float(bits) = s.value else { panic!("a vector lane is a float") };
    let want = Value::Float(bits ^ 1); // a last-bit drift, which is what a real port produces
    s.value = want;

    let d = replay::verify(&f).expect_err("a changed watch value must be noticed");
    let line = d.to_string();
    assert_eq!(line.lines().count(), 1, "one line, not a diff of two files: {line}");
    assert_eq!(
        line,
        format!("tick 413: bike.vel[1] {want} vs {}", Leaf::Float(bits)),
        "the tick, the path, the recording's value, then the port's",
    );
    assert!(line.starts_with("tick 413: bike.vel[1] "), "{line}");
    assert!(line.contains(" vs "), "{line}");
}

/// A tick covered by both a hash and a watch reports the watch, because a named field is an answer and
/// a hash is only a question.
#[test]
fn a_named_field_beats_a_bare_hash() {
    let mut f = recorded(600);
    // Break both at the same tick, in the way a real divergence does: the hash of a moved state
    // differs too.
    f.hashes[413] ^= 1;
    let w = f.watches.iter_mut().find(|w| w.path == "bike.vel[2]").unwrap();
    let Value::Float(bits) = w.samples[414].value else { panic!() };
    w.samples[414].value = Value::Float(bits ^ 1);

    let d = replay::verify(&f).expect_err("both checks fail at tick 414");
    assert_eq!(d.tick, 414);
    assert!(matches!(&d.reason, Reason::Field { path, .. } if path == "bike.vel[2]"), "{d}");
}

// ---------------------------------------------------------------------------------------------------
// (4) Nothing passes vacuously
// ---------------------------------------------------------------------------------------------------

#[test]
fn a_watch_on_a_field_that_does_not_exist_fails() {
    let mut f = recorded(10);
    f.watches.push(Watch::new(
        "player.vel.x", // the field this harness will watch one day, and does not have yet
        vec![Sample { tick: 3, value: Value::Float(0) }],
    ));
    let d = replay::verify(&f).expect_err("a path outside the walk is the fixture's problem");
    assert_eq!(d.tick, 3);
    assert!(matches!(&d.reason, Reason::NoSuchField { path } if path == "player.vel.x"), "{d}");
    assert!(d.to_string().contains("no field 'player.vel.x'"), "{d}");

    // And `record` refuses to make such a fixture in the first place.
    let e = replay::record("typo", 1, &inputs(4), &["bike.vel[9]"]).expect_err("a typo must not record");
    assert!(matches!(e, Reason::NoSuchField { .. }), "{e:?}");
}

#[test]
fn a_watch_of_the_wrong_kind_fails() {
    let mut f = recorded(10);
    f.watches.push(Watch::new("bike.vel[2]", vec![Sample { tick: 2, value: Value::Uint(0) }]));
    let d = replay::verify(&f).expect_err("an integer recorded against a float field");
    assert_eq!(d.tick, 2);
    assert!(matches!(d.reason, Reason::WrongKind { .. }), "{d}");
    assert!(d.to_string().contains("wrong kind of field"), "{d}");
}

/// An integer's *width* is the recorder's choice, not the game's, so a capture that read a counter as
/// `u32` matches a `u64` field. Signedness crosses too, as long as the value does.
#[test]
fn integer_width_and_signedness_may_differ_but_the_value_may_not() {
    assert_eq!(Value::Uint(7).compare(Leaf::Uint { value: 7, width: 2 }), replay::Verdict::Same);
    assert_eq!(Value::Uint(7).compare(Leaf::Int { value: 7, width: 1 }), replay::Verdict::Same);
    assert_eq!(Value::Int(-7).compare(Leaf::Int { value: -7, width: 8 }), replay::Verdict::Same);
    assert_eq!(Value::Uint(8).compare(Leaf::Uint { value: 7, width: 2 }), replay::Verdict::Different);
    assert_eq!(Value::Bool(true).compare(Leaf::Uint { value: 1, width: 1 }), replay::Verdict::Same);
    // A float never crosses: `1` from an integer column against a 1.0 field is a wrongly aimed watch.
    assert_eq!(
        Value::Uint(1).compare(Leaf::Float(f32::to_bits(1.0))),
        replay::Verdict::WrongKind
    );
    // -0.0 is not 0.0, and a NaN's payload is part of the state.
    assert_eq!(
        Value::Float(f32::to_bits(-0.0)).compare(Leaf::Float(f32::to_bits(0.0))),
        replay::Verdict::Different
    );
}

#[test]
fn a_fixture_that_expects_more_than_the_run_reaches_fails() {
    // Samples past the last input frame: a truncated input list with a full recording beside it.
    let mut f = recorded(10);
    f.watches[0].samples.push(Sample { tick: 99, value: Value::Float(0) });
    let d = replay::verify(&f).expect_err("a sample the run never reaches must be reported");
    assert!(matches!(&d.reason, Reason::Unreached { tick: 99, .. }), "{d}");
    assert!(d.to_string().contains("unchecked"), "{d}");

    // More hashes than frames, which is what a fixture edited by hand produces.
    let mut f = recorded(10);
    f.watches.clear();
    f.hashes.push(0);
    let d = replay::verify(&f).expect_err("an unchecked hash must be reported");
    assert!(matches!(&d.reason, Reason::Unreached { .. }), "{d}");

    // Samples out of tick order are never compared, so they are reported the same way rather than
    // silently skipped.
    let mut f = recorded(10);
    f.watches[0].samples.reverse();
    let d = replay::verify(&f).expect_err("samples out of order must be reported");
    assert!(matches!(&d.reason, Reason::Unreached { .. }), "{d}");
}

// ---------------------------------------------------------------------------------------------------
// (5) The fixture file
// ---------------------------------------------------------------------------------------------------

#[test]
fn a_fixture_round_trips_through_its_text_form() {
    let f = recorded(120);
    let text = f.to_text();
    let back = Fixture::parse(&text).expect("what we wrote is what we read");

    assert_eq!(back.name, f.name);
    assert_eq!(back.seed, f.seed);
    assert_eq!(back.inputs, f.inputs, "every button and both stick axes");
    assert_eq!(back.hashes, f.hashes);
    assert_eq!(back.watches, f.watches, "every sample, float patterns included");
    assert_eq!(back.checks(), f.checks());
    replay::verify(&back).expect("a round-tripped fixture still replays");
    // And writing it again is byte for byte the same file, so a fixture in version control (or in
    // `context/`) does not churn.
    assert_eq!(back.to_text(), text);
}

#[test]
fn a_hand_written_fixture_reads_as_it_looks() {
    let f = Fixture::parse(
        "# a fixture as a human would type one\n\
         format 1\n\
         fixture hand-written\n\
         seed 0x0000002a\n\
         \n\
         frame 0x0000 0 0\n\
         frame 0x0010 -64 0    # the stick hard left\n\
         watch 2 bike.vel[1] float -2.41325\n\
         watch 1 bike.vel[1] float 0xbfabb755\n\
         watch 2 ticks uint 2\n",
    )
    .expect("the documented format");

    assert_eq!(f.name, "hand-written");
    assert_eq!(f.seed, 42);
    assert_eq!(f.inputs.len(), 2);
    assert_eq!(f.inputs[1], Input { buttons: 0x10, stick: (-64, 0) });
    assert!(f.hashes.is_empty(), "no hashes is watches-only, not an error");
    assert_eq!(f.watches.len(), 2, "two paths, in the order they first appear");
    assert_eq!(
        f.watches[0].samples.iter().map(|s| s.tick).collect::<Vec<_>>(),
        vec![1, 2],
        "samples are sorted by tick however the file ordered them",
    );
    // 0xbfabb755 is the port's own vel.y after one frame of the bike falling, written as the pattern,
    // so tick 1 agrees exactly. Tick 2 is written as a decimal, and that is the whole argument for
    // patterns: the port is at -2.4132490 (0xc01a72ac) and the rounded `-2.41325` a human would type
    // reads as 0xc01a72b0, four bits along and not the same number.
    let d = replay::verify(&f).expect_err("-2.41325 is not the port's value to the last bit");
    assert_eq!(d.tick, 2);
    assert!(matches!(&d.reason, Reason::Field { path, .. } if path == "bike.vel[1]"), "{d}");
}

/// A typo in a fixture must never silently check less than it looks like it does.
#[test]
fn a_broken_fixture_file_says_which_line() {
    let cases = [
        ("seed 1\nhashes 3\n", 2, "unknown keyword"),
        ("seed 1\nframe 0 0\n", 2, "frame wants"),
        ("seed 1\nframe 0 0 0 0x1\nframe 0 0 0\n", 3, "no hash but an earlier one does"),
        ("seed 1\nframe 0 0 0\nframe 0 0 0 0x1\n", 3, "has a hash but an earlier one does not"),
        ("seed 1\nframe 0 999 0\n", 2, "stick wants"),
        ("seed 1\nwatch 3 bike.vel[2] float\n", 2, "watch wants"),
        ("seed 1\nwatch 3 bike.vel[2] quux 1\n", 2, "is not a value"),
        ("seed 0x1ffffffff\n", 1, "32-bit"),
        ("format 99\n", 1, "this build reads"),
    ];
    for (text, line, needle) in cases {
        let e = Fixture::parse(text).unwrap_err();
        assert_eq!(e.line, line, "{text:?} -> {e}");
        assert!(e.to_string().contains(needle), "{text:?} -> {e}");
    }
}

// ---------------------------------------------------------------------------------------------------
// (6) A capture of the original, read as watches
// ---------------------------------------------------------------------------------------------------

/// The acceptance path end to end, with a CSV in exactly the shape `./kit record` writes: a `frame`
/// column of the game's own counter, one column per field with a lane suffix, floats as a `repr()`
/// that round-trips, and an empty cell where a pointer chain went null.
///
/// The numbers here are the port's own first three ticks, which is what a capture of the original
/// *would* hold if the port is right — the only thing this file can assert without the disc.
#[test]
fn a_recorder_csv_becomes_watches() {
    let port = recorded(3);
    let lane = |path: &str, tick: usize| match port
        .watches
        .iter()
        .find(|w| w.path == path)
        .unwrap()
        .samples[tick]
        .value
    {
        Value::Float(bits) => f32::from_bits(bits),
        v => panic!("{path} is not a float: {v}"),
    };

    let csv = format!(
        "frame,bike_pos.y,bike_vel.z,gear,flags\n\
         101234,{},{},0,0x00\n\
         101235,{},{},1,0x01\n\
         101236,,{},2,0x02\n\
         101237,{},{},3,0x03\n",
        lane("bike.pos[1]", 0),
        lane("bike.vel[2]", 0),
        lane("bike.pos[1]", 1),
        lane("bike.vel[2]", 1),
        lane("bike.vel[2]", 2),
        lane("bike.pos[1]", 3),
        lane("bike.vel[2]", 3),
    );

    let cap = Capture::parse(&csv).expect("the recorder's format");
    assert_eq!(cap.columns.len(), 5);
    assert_eq!(cap.frames, vec![101234, 101235, 101236, 101237]);
    assert_eq!(cap.tick_of(0), 0, "the first row is the state the run starts from");
    assert_eq!(cap.span(), 3);

    let pos = cap.float("bike_pos.y", "bike.pos[1]").unwrap();
    assert_eq!(
        pos.samples.iter().map(|s| s.tick).collect::<Vec<_>>(),
        vec![0, 1, 3],
        "the empty cell is a frame that was not sampled, not a zero",
    );
    let f = Fixture {
        name: "capture".to_string(),
        seed: port.seed,
        inputs: port.inputs.clone(),
        watches: vec![
            pos,
            cap.float("bike_vel.z", "bike.vel[2]").unwrap(),
            cap.uint("flags", "ticks").unwrap(), // a hex column, read as the tick counter it mirrors
        ],
        ..Default::default()
    };
    assert_eq!(f.checks(), 3 + 4 + 4);
    replay::verify(&f).expect("the port agrees with a capture of its own numbers");

    // And one wrong cell fails at that row's tick, naming the field.
    let bad = csv.replace("101236,,", "101236,1.0,");
    let mut f2 = f.clone();
    f2.watches[0] = Capture::parse(&bad).unwrap().float("bike_pos.y", "bike.pos[1]").unwrap();
    let d = replay::verify(&f2).expect_err("1.0 is not where the bike is at tick 2");
    assert_eq!(d.tick, 2);
    assert!(d.to_string().starts_with("tick 2: bike.pos[1] 1 "), "{d}");
}

/// A sparse capture — `every = 4` in the watch spec — maps its frame counter to ticks, so the port is
/// compared on the frames that were sampled and simulated through the rest.
#[test]
fn a_sparse_capture_checks_the_frames_it_sampled() {
    let port = recorded(40);
    let spin = port.watches.iter().find(|w| w.path == "bike.vel[2]").unwrap();
    let mut csv = String::from("frame,spin\n");
    for tick in (0..=40).step_by(4) {
        let Value::Float(bits) = spin.samples[tick].value else { panic!() };
        csv += &format!("{},{}\n", 900 + tick, f32::from_bits(bits));
    }
    let cap = Capture::parse(&csv).unwrap();
    let f = Fixture {
        seed: port.seed,
        inputs: port.inputs.clone(),
        watches: vec![cap.float("spin", "bike.vel[2]").unwrap()],
        ..Default::default()
    };
    assert_eq!(f.checks(), 11);
    replay::verify(&f).expect("sampled every fourth frame, agreeing on every one of them");
}

#[test]
fn a_broken_capture_says_which_line() {
    let cases = [
        ("", 1, "empty"),
        ("vsync,pos.x\n1,2\n", 1, "first column must be 'frame'"),
        ("frame,pos.x\n", 1, "header and no rows"),
        ("frame,pos.x\n1,2\n2,3,4\n", 3, "3 cells for 2 columns"),
        ("frame,pos.x\nlater,2\n", 2, "is not a number"),
    ];
    for (text, line, needle) in cases {
        let e = Capture::parse(text).unwrap_err();
        assert_eq!(e.line, line, "{text:?} -> {e}");
        assert!(e.to_string().contains(needle), "{text:?} -> {e}");
    }

    let cap = Capture::parse("frame,pos.x\n1,oops\n").unwrap();
    let e = cap.float("pos.x", "bike.pos[0]").unwrap_err();
    assert_eq!(e.line, 2);
    assert!(e.to_string().contains("not float"), "{e}");
    let e = cap.float("pos.z", "bike.pos[2]").unwrap_err();
    assert!(e.to_string().contains("no column 'pos.z'"), "{e}");
}

// ---------------------------------------------------------------------------------------------------
// (7) Frame alignment, the mistake every project makes once
// ---------------------------------------------------------------------------------------------------

/// A capture sampled one frame later than the port's tick ends does not fail *somewhere*: it fails at
/// tick 1 and keeps failing, because every value is one frame of motion out. That signature is worth
/// recognising on sight — and [`Watch::shifted`] is the fix, not a tolerance.
#[test]
fn an_off_by_one_in_alignment_fails_at_the_first_tick() {
    let port = recorded(50);
    let shifted: Vec<Watch> = port.watches.iter().cloned().map(|w| w.shifted(-1)).collect();
    let f = Fixture { seed: port.seed, inputs: port.inputs.clone(), watches: shifted, ..Default::default() };
    let d = replay::verify(&f).expect_err("a capture read one frame early cannot match");
    assert_eq!(d.tick, 0, "the very first comparison is already wrong");

    // Shifted back again, it agrees everywhere — the samples that fell off the front are simply not
    // checked, since the run never had that state.
    let back: Vec<Watch> = f.watches.iter().cloned().map(|w| w.shifted(1)).collect();
    let n: usize = back.iter().map(|w| w.samples.len()).sum();
    let f = Fixture { watches: back, ..f };
    assert_eq!(f.checks(), n);
    replay::verify(&f).expect("aligned again");
}

/// `state_at` re-runs a fixture to the tick a hash divergence was reported on, which is how the bare
/// `state hash a vs b` becomes a field name.
#[test]
fn a_hash_divergence_can_be_explained_after_the_fact() {
    let mut f = recorded(600);
    f.watches.clear();
    f.hashes[412] ^= 1;
    let d = replay::verify(&f).expect_err("a changed hash");

    // The state the port was in at that tick, and a reference that differs in one field — which is
    // what a RAM image of the original at that frame gives you.
    let mine = replay::state_at(&f, d.tick);
    assert_eq!(mine.ticks, 413);
    let mut reference = mine.clone();
    let mut bits = mine.bike_vel().to_bits();
    bits[1] ^= 1;
    reference.rider.ride.chassis.vel = Vf::from_bits(bits);
    let why = game_sim::hash::first_difference(&reference, &mine).expect("they differ");
    assert_eq!(why.path(), "bike.vel[1]");
    assert_eq!(
        format!("tick {}: {why}", d.tick).lines().count(),
        1,
        "the explained form is still one line",
    );
}

// ---------------------------------------------------------------------------------------------------
// The private fixtures: data read out of the game, so never in the repository
// ---------------------------------------------------------------------------------------------------

/// Where recorded fixtures and captures live: `context/` at the repository root, which `.gitignore`
/// keeps out of git, or `$GAME_SIM_FIXTURES` when they are somewhere else — which they are whenever
/// `kit.toml`'s `[paths] context` points outside this checkout (a worktree sharing one context
/// directory, say). A test cannot read `kit.toml`, so the environment variable is the seam.
fn private_dir(sub: &str) -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir).join(sub),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("context")
            .join(sub),
    }
}

fn files_in(dir: &PathBuf, extension: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == extension))
        .collect();
    out.sort();
    out
}

/// Every `*.fixture` in `context/fixtures/` replays tick-exact. This is the acceptance test the plan
/// builds towards: when a recording of real play is in there, this test is the port's verdict.
///
/// Keep the minutes-long ones in `context/fixtures/slow/`, which this does not descend into — they are
/// run by `replaying_the_slow_fixtures`, behind `--ignored`.
#[test]
fn replaying_every_fixture_in_context() {
    replay_dir(&private_dir("fixtures"));
}

/// The same, for `context/fixtures/slow/`:
/// `cargo test -p game-sim --test replay -- --ignored --nocapture`.
#[test]
#[ignore = "minutes-long fixtures; run with --ignored"]
fn replaying_the_slow_fixtures() {
    replay_dir(&private_dir("fixtures").join("slow"));
}

fn replay_dir(dir: &PathBuf) {
    let files = files_in(dir, "fixture");
    if files.is_empty() {
        println!("no *.fixture in {} — nothing to replay", dir.display());
        return;
    }
    for path in files {
        let text = std::fs::read_to_string(&path).expect("a readable fixture");
        let f = Fixture::parse(&text)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        match replay::verify(&f) {
            // The fixture's own name is in the message because a failing run names a file, and the
            // file is not in the repository for anyone else to look at.
            Err(d) => panic!("{} [{}]: {d}", path.display(), f.name),
            Ok(_) => println!(
                "{}: {} frames, {} checks, all agreeing",
                path.display(),
                f.inputs.len(),
                f.checks()
            ),
        }
    }
}

/// Every CSV in `context/capture/` is readable by [`Capture`]. The point is not the numbers — this
/// test does not know what the columns mean — but the *format*: that the reader in this crate still
/// agrees with what `./kit record` actually writes, rather than with how its documentation reads.
#[test]
fn every_capture_in_context_parses() {
    let dir = private_dir("capture");
    let files = files_in(&dir, "csv");
    if files.is_empty() {
        println!("no *.csv in {} — nothing to read", dir.display());
        return;
    }
    for path in files {
        let text = std::fs::read_to_string(&path).expect("a readable capture");
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(cap.columns[0], "frame");
        assert!(
            cap.frames.windows(2).all(|w| w[0] < w[1]),
            "{}: the frame counter must rise; a repeated frame is a torn capture",
            path.display(),
        );
        // Every column must be readable by one of the typed readers: the recorder writes an integer as
        // decimal, a `fmt = "hex"` column as `0x…`, a float with a `repr()` that round-trips, and a
        // `bytes:N` field as a hex string, and all four have to come back as values this harness can
        // compare. Which reader a column wants is the watch spec's business, not this test's — the
        // property here is that none of them is unreadable.
        let mut kinds = Vec::new();
        for column in cap.columns.iter().skip(1) {
            let readers: [(&str, fn(&Capture, &str, &str) -> Result<Watch, _>); 4] = [
                ("uint", Capture::uint),
                ("int", Capture::int),
                ("float", Capture::float),
                ("blob", Capture::blob),
            ];
            let kind = readers
                .iter()
                // The path is irrelevant here — the watch is thrown away and only the cells matter.
                .find(|(_, read)| read(&cap, column, column).is_ok())
                .unwrap_or_else(|| {
                    panic!("{}: no reader accepts column '{column}'", path.display())
                })
                .0;
            kinds.push(format!("{column}:{kind}"));
        }
        println!(
            "{}: {} rows x {} columns, frames {}..{} [{}]",
            path.display(),
            cap.frames.len(),
            cap.columns.len(),
            cap.frames[0],
            cap.frames[cap.frames.len() - 1],
            kinds.join(" "),
        );
    }
}

// ===================================================================================================
// S8 — the acceptance run
// ===================================================================================================
//
// The acceptance run is one recording of real play — several minutes of riding, jumping, landing and
// crashing — replayed tick by tick against the port. Everything above this line proves the *harness*;
// this section builds the thing the harness is for, and it is the only test in the project that can
// say the simulation is right rather than merely unchanged.
//
// It has three parts, and two of them are here in full:
//
//  1. **The recipe**: `watch/acceptance.toml`, the watch spec whose every row carries both halves of a
//     frame — the input the game read, and the bike it moved. `the_watch_spec_and_this_reader_agree`
//     keeps the spec and the code below from drifting apart.
//  2. **The reader**: a capture of that spec becomes a complete [`Fixture`] — *inputs* from the control
//     columns (which A2's `Capture` cannot do: it only reads watches), watches from the bike columns.
//     Proved below on captures built to the spec's exact shape, including the half-stamp trap and every
//     way a capture can fail to be an acceptance recording.
//  3. **The recording itself**, which is game data and therefore not in this repository, and which is
//     absent in every worktree but the one that recorded it — see this task's journal entry and
//     `plan/HUMAN.md`. `the_acceptance_run` is where it lands: drop the capture in
//     `context/capture/acceptance/` (or loose in `context/capture/`, where `./kit record` writes it
//     by default — the reader looks in both) and that test is the port's verdict, with no code to
//     write. Since P5 that verdict is a real comparison: the eight lanes a capture carries are eight
//     fields of the state the harness drives, so the answer is a tick and a field rather than
//     `NoSuchField` at tick 0; and since S8 the run starts from the capture's own first row, so tick
//     0 is an agreement rather than two different spawns.
//
// Plus the scale the word "acceptance" implies: the tests at the end of this file run a fixture of
// several minutes rather than several hundred frames, because a harness that is exact over 600 ticks
// and quietly wrong over 9,000 would pass everything above and fail the one run that matters.

/// The acceptance recording's name, and so the name of its capture (`context/capture/<name>.csv`) and
/// of the fixture built from it (`context/fixtures/slow/<name>.fixture`).
const ACCEPTANCE: &str = "acceptance-freeride";

/// The bike columns `watch/acceptance.toml` records, and the path in the port's state each is compared
/// against. All four lanes of each quad: the `w` lane of a PS2 vector is either meaningful or usefully
/// garbage, and a port that leaves it undefined is a port that will diverge the first time something
/// reads it.
///
/// **Every one of these paths exists in the state the harness drives** (P5): [`game_sim::Sim`] *is* the
/// ride, the rider and the world the shell builds, and its walk names the bike's transform origin and
/// the body's velocity with exactly the eight names on the right — which is why the right column is
/// also [`AS_THE_PORT_HAS_THEM`]. Before P5 the harness drove A1's example ball and the verdict of a
/// real acceptance capture was `NoSuchField 'bike.pos[0]'` at tick 0: the fixture's own fault in A2's
/// taxonomy, and a run that compared nothing. What is still unproven is the *agreement* — that the
/// port's numbers in these lanes are the original's — because that needs a capture of the disc.
const BIKE: [(&str, &str); 8] = [
    // +0x1b0, row 3 of the world transform, is the position
    ("bike_xform.m30", "bike.pos[0]"),
    ("bike_xform.m31", "bike.pos[1]"),
    ("bike_xform.m32", "bike.pos[2]"),
    ("bike_xform.m33", "bike.pos[3]"),
    // +0x130
    ("bike_vel.x", "bike.vel[0]"),
    ("bike_vel.y", "bike.vel[1]"),
    ("bike_vel.z", "bike.vel[2]"),
    ("bike_vel.w", "bike.vel[3]"),
];

/// How many controls the game's input manager keeps for one pad.
const CONTROLS: usize = 16;

/// Which bit of libpad's button word each of the game's sixteen controls comes from — the game's own
/// control map, which is **not** the identity: control 0 is triangle (0x0010) and control 6 is L2
/// (0x0001). C5 read the map out of RAM; eight of the sixteen were re-measured here from two 300-frame
/// captures (up, down, L1, R1, L2, R2, triangle, square — every button `./kit pad` can send that the
/// front end ignores) and agreed with it exactly.
///
/// It is here only to cross-check the decode against libpad's word in
/// `every_input_capture_in_context_agrees_with_the_wire`. The fixture's own buttons are indexed by
/// *control*, not by wire bit, because the control is what the simulation reads.
const CONTROL_MASK: [u16; CONTROLS] = [
    0x0010, // 0  triangle
    0x0020, // 1  circle
    0x0040, // 2  cross
    0x0080, // 3  square
    0x0004, // 4  L1
    0x0008, // 5  R1
    0x0001, // 6  L2
    0x0002, // 7  R2
    0x0100, // 8  select
    0x0800, // 9  start
    0x0400, // 10 R3
    0x0200, // 11 L3
    0x1000, // 12 up
    0x2000, // 13 right
    0x4000, // 14 down
    0x8000, // 15 left
];

/// The other half of the game's own control map: which report byte each control's **pressure** is in,
/// and `None` for the four a DualShock 2 reports no pressure for.
///
/// It matters to a recording because the game reads those twelve controls from the pressure byte and
/// never from the bit (I1 measured that; `the_synthetic_capture_is_the_map_the_port_measured` below
/// checks this table against the port's own). A synthetic capture therefore has to write the pressures
/// as well as the word, or it describes a frame the game would have read as nothing held.
const CONTROL_PRESSURE: [Option<usize>; CONTROLS] = [
    Some(12), // 0  triangle
    Some(13), // 1  circle
    Some(14), // 2  cross
    Some(15), // 3  square
    Some(16), // 4  L1
    Some(17), // 5  R1
    Some(18), // 6  L2
    Some(19), // 7  R2
    None,     // 8  select
    None,     // 9  start
    None,     // 10 R3
    None,     // 11 L3
    Some(10), // 12 up
    Some(8),  // 13 right
    Some(11), // 14 down
    Some(9),  // 15 left
];

/// A set of controls as the port's own bitmask: the wire bits those controls sit on.
///
/// The two mask spaces of this file, and the one thing to keep straight in it. A capture's `ctlN.held`
/// columns are indexed by **control** (0 is triangle, 6 is L2); [`Input::buttons`] and everything in
/// `game_sim::input` is the game's own **bitmask** (triangle is 0x0010, L2 is 0x0001). Before I1 landed
/// its mapping the fixtures here carried the former, which would have pressed the wrong buttons the
/// first time one was replayed through the input layer; they carry the latter now, and this is the
/// conversion.
fn wire_of(controls: u16) -> u16 {
    (0..CONTROLS).filter(|n| controls & (1 << n) != 0).fold(0, |w, n| w | CONTROL_MASK[n])
}

fn column(cap: &Capture, name: &str) -> Result<usize, String> {
    cap.columns
        .iter()
        .position(|c| c == name)
        .ok_or_else(|| format!("no column '{name}' in the capture ({})", cap.columns.join(", ")))
}

fn cell<'a>(cap: &'a Capture, row: usize, at: usize) -> &'a str {
    cap.rows[row][at].as_str()
}

/// The tick of every row, taken from the game's **own** frame counter (`frame_counter`, app+0x374),
/// and the check that there is one row per tick with no gaps.
///
/// **Not from the recorder's `frame` column**, which is the vsync counter: this game ticks at half the
/// video rate, so a capture of real play has `frame` rising by 2 and `frame_counter` by 1 on every row
/// (measured: 300 of 300 rows, every delta exactly 2 and 1). `Capture::tick_of` — which knows only
/// about `frame` — therefore says *twice* the tick for this game, and a fixture built on it would
/// compare the original's frame 1 against the port's tick 2 and be wrong by a factor of two for the
/// whole run. Every watch below is re-ticked through here instead.
///
/// The density matters because an acceptance fixture needs the input of *every* tick — the port has
/// nothing to simulate a skipped tick with — so a capture written with `every = 2`, gated by a `when`,
/// or missing a frame the recorder dropped as torn, is not an acceptance recording. This is what turns
/// that from a silent desync into a sentence.
fn ticks_of(cap: &Capture) -> Result<Vec<u64>, String> {
    // Either name the specs give that one word: `frame_counter` in `watch/acceptance.toml`,
    // `engine_frame` in `watch/input.toml`. Both are app+0x374, and a reader that insisted on one of
    // them would refuse half the recordings in the project.
    let name = ["frame_counter", "engine_frame"]
        .into_iter()
        .find(|n| column(cap, n).is_ok())
        .ok_or_else(|| column(cap, "frame_counter").unwrap_err())?;
    let at = column(cap, name)?;
    let mut counters = Vec::with_capacity(cap.rows.len());
    for row in 0..cap.rows.len() {
        let text = cell(cap, row, at);
        counters
            .push(text.parse::<u64>().map_err(|_| format!("row {row}: {name} '{text}' is not a number"))?);
    }
    for (row, pair) in counters.windows(2).enumerate() {
        if pair[1] != pair[0] + 1 {
            return Err(format!(
                "{name} {} follows {} (row {}): an acceptance recording needs every tick's \
                 input, so it must be recorded with every = 1 and no skipped rows",
                pair[1],
                pair[0],
                row + 1,
            ));
        }
    }
    let first = counters[0];
    Ok(counters.iter().map(|c| c - first).collect())
}

/// The spread of the `dt` the original was handed over the run, as (least, greatest).
///
/// This is **not** constant in a running game: three distinct values in 300 frames of the front end
/// (0.03283332, 0.03286665, 0.03333000), and R6 saw 0.041532 in a level. The game measures the frame
/// rather than assuming it, so the number the simulation integrates with depends on how long the frame
/// actually took — which a fixed-step port cannot reproduce, and which no amount of care in the harness
/// can fix. It is reported rather than refused: see this task's journal entry, because what to do about
/// it is a decision about the port's tick (S1), not about the acceptance reader.
fn dt_spread(cap: &Capture) -> Result<(f32, f32), String> {
    let at = column(cap, "dt")?;
    let mut least = f32::INFINITY;
    let mut most = f32::NEG_INFINITY;
    for row in 0..cap.rows.len() {
        let text = cell(cap, row, at);
        let dt: f32 = text
            .parse()
            .map_err(|_| format!("row {row}: dt '{text}' is not a number"))?;
        least = least.min(dt);
        most = most.max(dt);
    }
    Ok((least, most))
}

/// The sixteen controls the game's own input manager believes are held, as one bit each: bit N is
/// control N.
///
/// This — rather than libpad's button word — is what the fixture's buttons are built from, because it
/// is the input the *simulation* reads. It needs no guess about which wire bit the game assigns to
/// which control (C5 measured that map; 12 is up and 14 is down), and it is immune to the half-stamp
/// trap below. What it is not is the game's own packed bitmask: the game keeps no such word, so the
/// numbering here is the control index. When I1 lands with the port's own mapping, this is the function
/// that changes shape, and every acceptance fixture has to be rebuilt from its capture.
fn held_mask(cap: &Capture, row: usize, at: &[usize]) -> Result<u16, String> {
    let mut mask = 0u16;
    for (control, &column) in at.iter().enumerate() {
        let text = cell(cap, row, column);
        let held: u32 = text.parse().map_err(|_| {
            format!(
                "frame {}: ctl{control}.held is '{text}', which is not 0 or 1",
                cap.frames[row],
            )
        })?;
        if held != 0 {
            mask |= 1 << control;
        }
    }
    Ok(mask)
}

/// The bytes of one libpad buffer half, from a `bytes:20` column.
fn half(cap: &Capture, row: usize, at: usize) -> Result<Vec<u8>, String> {
    let text = cell(cap, row, at);
    let hex = text.strip_prefix("0x").unwrap_or(text);
    if hex.len() < 16 || hex.len() % 2 != 0 {
        return Err(format!(
            "frame {}: '{text}' is not a libpad buffer half (expected at least 8 bytes of hex)",
            cap.frames[row],
        ));
    }
    (0..hex.len() / 2)
        .map(|i| {
            u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(|_| format!("frame {}: '{text}' is not hex", cap.frames[row]))
        })
        .collect()
}

struct PadColumns {
    pad_a: usize,
    stamp_a: usize,
    pad_b: usize,
    stamp_b: usize,
}

/// One [`Input`] per frame of a capture of `watch/acceptance.toml`.
///
/// **Alignment**, which is the whole of the difficulty: the recorder samples once a frame, after the
/// tick, so row *k*'s control columns are the input the game consumed in the tick that produced row
/// *k*'s state. A2's harness numbers the same way — tick 0 is the state the run starts from and
/// `inputs[k - 1]` produces tick *k* — so row 0's controls belong to the tick *before* the recording
/// and are dropped, and `inputs` is one shorter than the capture. If a real recording ever diverges at
/// tick 1 and stays diverged, this paragraph is the first suspect and
/// [`Watch::shifted`] is the knob; `an_off_by_one_in_alignment_fails_at_the_first_tick` above is what
/// that failure looks like.
fn inputs_from_capture(cap: &Capture) -> Result<Vec<Input>, String> {
    // The game's own tick counter, which `PadTape::from_capture` only falls back to the recorder's
    // `frame` column for: a capture of this spec has one, and its gap message names it.
    ticks_of(cap)?;
    let tape = PadTape::from_capture(cap).map_err(|e| e.to_string())?;
    Ok(tape.inputs())
}

/// A capture of `watch/acceptance.toml` as a complete fixture: the inputs the original was given, and
/// the bike it moved, ready for [`replay::verify`].
///
/// The checks are the spec's own three, and each of them is a way a capture can look like an
/// acceptance recording and not be one: a chain that resolved to the wrong actor (`bike_type` is not
/// 4), a chain that went null (an empty cell), an actor nobody was riding (`bike_player` is 0), a
/// variable `dt` (the original integrated a frame the port's fixed step cannot reproduce), and a gap in
/// the frames. None of them can be left to the eye: they are all invisible in a CSV of 9,000 rows.
fn acceptance_fixture(cap: &Capture) -> Result<Fixture, String> {
    acceptance_fixture_on(cap, None)
}

/// [`acceptance_fixture`] on a bike read off the disc instead of [`game_sim::stock`]'s stand-in: the
/// one seam S28 needed on this side, since the `.phy` decides the suspension's rates, the tire forces
/// and the mass. The wheel mounts come with it but not off it: they are the bike model's tire nodes
/// ([`Ridden`]).
///
/// `None` is the stand-in, which is what every synthetic capture in this file is built and checked
/// against; a real acceptance capture passes the Race 125 the human rode
/// ([`the_bike_the_human_rode`]).
fn acceptance_fixture_on(
    cap: &Capture,
    bike: Option<Ridden>,
) -> Result<Fixture, String> {
    let inputs = inputs_from_capture(cap)?;

    let (kind, player) = (column(cap, "bike_type")?, column(cap, "bike_player")?);
    let source = column(cap, "bike_ctl_src")?;
    for row in 0..cap.rows.len() {
        let frame = cap.frames[row];
        // The one column that tells the human's bike from an AI's. A freeride area holds one vehicle
        // per rider — four of them on the acceptance recording's own area — and an AI's is type 4 with
        // a non-null player record too, so without this a capture of the wrong list entry reads as a
        // perfectly good acceptance recording of somebody else's ride.
        if cell(cap, row, source) != "1" {
            return Err(format!(
                "frame {frame}: bike_ctl_src is '{}', not 1 — this list entry is not the local \
                 human's vehicle (2 is an AI rider); change the number of 0x8 hops in \
                 watch/acceptance.toml until it reads 1",
                cell(cap, row, source),
            ));
        }
        if cell(cap, row, kind) != "4" {
            return Err(format!(
                "frame {frame}: bike_type is '{}', not 4 — the chain in watch/acceptance.toml is \
                 pointing at something that is not a vehicle actor; change its number of 0x8 hops",
                cell(cap, row, kind),
            ));
        }
        if cell(cap, row, player) == "0" || cell(cap, row, player).is_empty() {
            return Err(format!(
                "frame {frame}: bike_player is '{}' — nobody is riding this actor, so its motion is \
                 not the recorded input's doing",
                cell(cap, row, player),
            ));
        }
    }

    // The step the original actually integrated with, carried in the fixture's name because it is the
    // first thing to look at when a replay diverges everywhere at once: a fixed-step port cannot match
    // a run the original measured frame by frame.
    let (least, most) = dt_spread(cap)?;

    // The original's RNG state at the start of the run, so a replay draws the same numbers. The format
    // can only carry 32 bits of it (`Fixture::seed`), which is this task's largest gap — see the
    // journal entry.
    let rng = column(cap, "rng_state")?;
    let state: u64 = cell(cap, 0, rng)
        .parse()
        .map_err(|_| format!("rng_state '{}' is not a number", cell(cap, 0, rng)))?;

    let ticks = ticks_of(cap)?;
    let mut watches = Vec::new();
    for (column, path) in BIKE {
        let mut w = cap.float(column, path).map_err(|e| e.to_string())?;
        if w.samples.len() != cap.rows.len() {
            return Err(format!(
                "column '{column}' has {} samples for {} rows: an empty cell is a pointer chain that \
                 went null, and a fixture cannot be built across one",
                w.samples.len(),
                cap.rows.len(),
            ));
        }
        // Re-ticked off the game's own counter: the samples are in row order (there are no empty
        // cells, checked just above), and the recorder's `frame` column counts video fields, not ticks.
        for (sample, &tick) in w.samples.iter_mut().zip(&ticks) {
            sample.tick = tick;
        }
        watches.push(w);
    }

    Ok(Fixture {
        name: format!(
            "{ACCEPTANCE} ({} ticks of real play, rng 0x{state:016x}, dt {least}..{most})",
            inputs.len(),
        ),
        seed: state as u32,
        // Where the run starts, read off the capture's own first row rather than fallen back to
        // `Sim::new`'s spawn — see [`acceptance_start`], and S8's journal entry for the verdict that
        // made it necessary.
        start: Some(acceptance_start(cap, state as u32, bike)?),
        inputs,
        watches,
        ..Default::default()
    })
}

/// **The state the capture's first row was taken in**, as a [`Sim`] the harness can start from.
///
/// An acceptance capture is frames out of the middle of a session — the one in hand is the game's
/// ticks 2774..4584 of a ride that was already minutes old — so [`Sim::new`]'s start line is not
/// where it begins. Without this, [`Fixture::initial`] falls back to that spawn and the very first
/// comparison is `tick 0: bike.pos[0] 4309.563 vs 0`: the fixture's own start against the port's,
/// before a single force has been computed. That was the verdict P5 left to S8 and it is not a
/// statement about the physics at all.
///
/// What the capture carries of a start state is four quads, and all four are seeded here from the
/// original's own bits:
///
/// - the bike's **whole world transform** — its basis in rows 0..2 and its origin in row 3. The basis
///   goes in as a quaternion through [`math::mat4_to_quat`] rather than through [`Ride::place`]'s
///   `basis(at, forward, UP)`, because that rebuilds an *up-righted* frame from a forward vector and a
///   bike mid-ride is rolled and pitched. An up-righted copy of a leaning bike is a different state.
/// - the body's **velocity** quad, `w` lane included: the original's reads 1.0 on the capture in hand
///   and a port that leaves it 0 has a divergence in `bike.vel[3]` on every tick of the run.
///
/// Everything else about that instant is **not in the capture** and stays at what [`Sim::ride`]
/// builds: the engine's revs and gear, the suspension's compression and the wheels' spin, the air and
/// trick state, the balance accumulators, the input layer's press timers, and the area's respawn
/// point (set here to where the capture starts, which is at least on the ground the rider was on).
/// Each of those is a real difference from the original's state at tick 0 and each of them is a
/// reason the replay diverges a few ticks in rather than at tick 0; the journal entry lists them
/// against what would close them.
fn acceptance_start(
    cap: &Capture,
    seed: u32,
    bike: Option<Ridden>,
) -> Result<Sim, String> {
    // Read through `Capture::float`, which is the same parse the watches get, so the start state's
    // position and velocity are bit-identical to the tick-0 samples they are compared against.
    let lane = |column: &str| -> Result<f32, String> {
        let w = cap.float(column, column).map_err(|e| e.to_string())?;
        match w.samples.first().map(|s| s.value) {
            Some(Value::Float(bits)) => Ok(f32::from_bits(bits)),
            _ => Err(format!("column '{column}' has no first row to start from")),
        }
    };
    let row = |n: usize| -> Result<Vf, String> {
        Ok(Vf::new(
            lane(&format!("bike_xform.m{n}0"))?,
            lane(&format!("bike_xform.m{n}1"))?,
            lane(&format!("bike_xform.m{n}2"))?,
            lane(&format!("bike_xform.m{n}3"))?,
        ))
    };
    let m = Mat4([row(0)?, row(1)?, row(2)?, row(3)?]);
    let vel =
        Vf::new(lane("bike_vel.x")?, lane("bike_vel.y")?, lane("bike_vel.z")?, lane("bike_vel.w")?);

    let origin = m.0[3];
    // Row 2 is the forward axis, which is where the original's own reader takes it off this transform
    // (`Ride::axes`). `place` is called for its clean slate — the derived fields, the wheels, the air
    // and the ground step's accumulators all zeroed together — and then the parts of the pose the
    // capture actually measured are put in over the top of it.
    let ridden = bike.is_some();
    let mut sim = match bike {
        // The bike off the disc, placed where the capture starts. `Sim::ride` is the shell's own
        // constructor and the rest of this function is the same whichever bike arrived; the wreck
        // test is left where `Sim::new` leaves it, off, which is a difference from the original and
        // is written down in S28's journal entry rather than guessed at here.
        //
        // Built the way the shell builds it rather than through `Sim::ride`, because the wheel mounts
        // are the bike *model's* tire nodes ([`the_wheels_the_human_rode`]) and `Sim::ride` takes
        // the `.phy`'s, which hang the wheels 1.48 units lower than the original's do.
        Some(Ridden { tables, phy, mounts }) => {
            let mut rider = Rider::new(&phy, mounts, origin, m.0[2], Set::default());
            rider.sense.wrecks_enabled = false;
            Sim::with_rider(seed, tables, phy, rider)
        }
        None => Sim::new(seed),
    };
    let ride = &mut sim.rider.ride;
    ride.place(origin, m.0[2]);
    ride.spawn = ride.origin;
    ride.spawn_facing = m.0[2];
    ride.chassis.quat = math::mat4_to_quat(&m);
    ride.node = [m.0[0], m.0[1], m.0[2]];
    ride.chassis.cofg_world = math::transform_point(&m, ride.chassis.cofg);
    ride.chassis.vel = vel;
    // A pure function of the velocity, and the integrator's own four instructions for it (`mula`,
    // `madda`, `madd`, `sqrt`): seeded, because the ground step reads a speed on the frame before the
    // integrator first writes one and a standing bike's zero is not what the original read there.
    ride.chassis.speed =
        Acc::mula(vel.x(), vel.x()).madda(vel.y(), vel.y()).madd(vel.z(), vel.z()).sqrt();
    // The drivetrain the capture cannot carry, from the save state the capture was recorded out of.
    // Left at `place`'s zeros, the clutch's launch ramp reads a timer of 0 and fires on tick 1 —
    // three times the torque and the centre of gravity pulled 1.47 units back — on a bike the original
    // had finished launching a second and a half earlier.
    if ridden {
        if let Some(d) = SaveState::open().filter(|s| s.leads(cap)).and_then(|s| s.drivetrain()) {
            d.seed(ride);
        }
    }
    Ok(sim)
}

/// The eight lanes a capture of the original carries, as the port's state names them — the right column
/// of [`BIKE`], written out so a synthetic capture can be built out of the port's own numbers in them.
/// That the two lists are the same list is what P5 did and is asserted where the reader is proved end
/// to end (`a_capture_of_real_play_becomes_a_fixture`).
const AS_THE_PORT_HAS_THEM: [&str; 8] = [
    "bike.pos[0]",
    "bike.pos[1]",
    "bike.pos[2]",
    "bike.pos[3]",
    "bike.vel[0]",
    "bike.vel[1]",
    "bike.vel[2]",
    "bike.vel[3]",
];

/// The first vsync counter of a synthetic capture — the recorder's own `frame` column — and the first
/// value of the game's tick counter. Two different numbers, neither of them zero, so that anything
/// confusing one for the other (or either for a tick index) is caught. The vsync counter rises by two
/// a row, as it does in a real capture of this game: the simulation ticks at half the video rate.
const FIRST_FRAME: u64 = 101_234;
const FIRST_COUNTER: u64 = 2_268;

/// The three values `dt` took over 300 frames of the real game, in the order they appeared. A synthetic
/// capture cycles them, because a capture where `dt` is constant is not one this game ever produced.
const MEASURED_DT: [&str; 3] = ["0.03286665305495262", "0.03283332288265228", "0.033330000936985016"];

/// The whole 64-bit LCG state of the synthetic recording; its low half is the seed a fixture can carry.
const RNG_STATE: u64 = 0x0123_4567_89ab_cdef;

/// A capture in the exact shape `watch/acceptance.toml` writes: one row per frame, sixteen control
/// columns, both libpad halves with their stamps, and the bike.
///
/// `input(frame)` says what was held and where the stick was on each frame, and the bike columns are
/// the port's own run of those very frames, so a fixture built from this capture replays exactly. Half
/// **b** carries the frame's own input and half **a** the next frame's, with `stamp_a = stamp_b + 1`,
/// which is what the recorder sees: the game read the older half and the IOP has already refreshed the
/// newer one.
///
/// Two passes, because since P5 the bike is driven by the pad: the port's numbers are its run on *this
/// capture's own* inputs and seed, and those are what [`acceptance_fixture`] derives from the rows, so
/// the rows have to exist before the numbers that belong in them can be simulated. The first pass
/// writes the capture with its bike columns zeroed and reads it back exactly as a test would; the
/// second writes the same capture with the port's own numbers in those columns.
///
/// Since S8 the start state is one of the things the reader derives from the rows, so the recording
/// pass starts from the first pass's own [`Fixture::initial`] rather than from a bare seed, and the
/// two passes are a fixed point: the blank capture's start is the origin with an identity basis and
/// no velocity, a run from there puts exactly that in row 0, and reading row 0 back gives the same
/// start again. A `record` from `Sim::new`'s spawn instead would make a capture whose first row is a
/// state its own reader does not rebuild, and every synthetic fixture would diverge at tick 0.
fn acceptance_csv(frames: usize, input: impl Fn(usize) -> (u16, (u8, u8))) -> String {
    let blank = acceptance_rows(frames, &input, &|_, _| "0".to_string());
    let read = acceptance_fixture(&Capture::parse(&blank).expect("the recorder's format"))
        .expect("a well-formed capture, whatever its bike columns hold");
    let port = replay::record_from("port", read.initial(), &read.inputs, &AS_THE_PORT_HAS_THEM)
        .expect("the port's own lanes");
    acceptance_rows(frames, &input, &|n: usize, tick: usize| {
        match port.watches[n].samples[tick].value {
            Value::Float(bits) => format!("{}", f32::from_bits(bits)),
            v => panic!("{v} is not a float"),
        }
    })
}

/// The rows themselves: `lane(n, tick)` is the text of lane `n` of [`AS_THE_PORT_HAS_THEM`] on `tick`.
fn acceptance_rows(
    frames: usize,
    input: &dyn Fn(usize) -> (u16, (u8, u8)),
    lane: &dyn Fn(usize, usize) -> String,
) -> String {
    let buffer = |frame: usize| {
        let (buttons, (lx, ly)) = input(frame);
        let mut bytes = [0u8; 20];
        bytes[1] = 0x79; // DualShock 2 with pressures
        // The wire word is active low, and the controls map onto it through the game's own map.
        let wire: u16 = (0..CONTROLS)
            .filter(|n| buttons & (1 << n) != 0)
            .fold(0, |w, n| w | CONTROL_MASK[n]);
        bytes[2] = !(wire >> 8) as u8;
        bytes[3] = !wire as u8;
        // And the pressures, because the game reads twelve of its sixteen controls from those bytes
        // and not from the word: a capture whose pressures are all zero is one the game itself would
        // have read as nothing held, however much its control columns say. A digital source reports
        // full pressure, which is what the kit's virtual pad sends and what the real captures show.
        for n in 0..CONTROLS {
            if let (true, Some(at)) = (buttons & (1 << n) != 0, CONTROL_PRESSURE[n]) {
                bytes[at] = 0xff;
            }
        }
        bytes[4] = 0x7f; // RX
        bytes[5] = 0x7f; // RY
        bytes[6] = lx;
        bytes[7] = ly;
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };

    let mut csv = String::from("frame,frame_counter,dt,rng_state");
    for n in 0..CONTROLS {
        csv += &format!(",ctl{n}.held");
    }
    csv += ",pad_a,stamp_a,pad_b,stamp_b,bike_type,bike_ctl_src,bike_player";
    for lane in ["x", "y", "z", "w"] {
        csv += &format!(",bike_vel.{lane}");
    }
    for row in 0..4 {
        for col in 0..4 {
            csv += &format!(",bike_xform.m{row}{col}");
        }
    }
    csv += "\n";

    for tick in 0..=frames {
        csv += &format!(
            "{},{},{},{RNG_STATE}",
            FIRST_FRAME + 2 * tick as u64,
            FIRST_COUNTER + tick as u64,
            MEASURED_DT[tick % MEASURED_DT.len()],
        );
        let (buttons, _) = input(tick);
        for n in 0..CONTROLS {
            csv += if buttons & (1 << n) != 0 { ",1" } else { ",0" };
        }
        csv += &format!(
            ",{},{},{},{},4,1,19873344",
            buffer(tick + 1),
            2000 + tick + 1,
            buffer(tick),
            2000 + tick,
        );
        for n in 4..8 {
            csv += &format!(",{}", lane(n, tick));
        }
        // The transform, with the position in row 3 and plausible rubbish in the rest of it.
        for row in 0..3 {
            for col in 0..4 {
                csv += if row == col { ",1" } else { ",0" };
            }
        }
        for n in 0..4 {
            csv += &format!(",{}", lane(n, tick));
        }
        csv += "\n";
    }
    csv
}

/// What a synthetic capture's inputs look like: something held on most frames, the stick moving, and a
/// few frames with nothing at all.
fn played(frame: usize) -> (u16, (u8, u8)) {
    let buttons = match frame % 7 {
        0 => 0,                         // nothing held
        1 => 1 << 14,                   // down
        2 => (1 << 14) | (1 << 6),      // down + L2: a brake and a turn, which is most of riding
        3 => 1 << 12,                   // up
        4 => (1 << 12) | (1 << 15) | 1, // up + left + triangle
        5 => 1 << 4,                    // L1
        _ => 0xffff,                    // everything at once, which a real pad can do
    };
    let stick = ((0x7f + (frame % 13) as u8).wrapping_sub(6), (0x7f + (frame % 5) as u8));
    (buttons, stick)
}

/// A capture of real play becomes a fixture with one input per frame and one sample per watched lane
/// per frame — and replays exactly, against the lanes the capture's own columns name.
///
/// This is the end-to-end proof of the reader: the columns of a capture in the recorder's own shape
/// become inputs, a seed and watches, and the watches line up tick for tick with a run of the
/// simulation that those inputs drive. What it cannot prove is that the *original's* bike agrees with
/// the port's — the numbers in these bike columns are the port's own, because the capture is
/// synthetic. That comparison is a capture of the disc, which no test can carry; see this task's
/// journal entry.
#[test]
fn a_capture_of_real_play_becomes_a_fixture() {
    let cap = Capture::parse(&acceptance_csv(240, played)).expect("the recorder's format");
    assert_eq!(cap.rows.len(), 241, "one row per frame, plus the state tick 0 starts from");

    let f = acceptance_fixture(&cap).expect("a well-formed acceptance capture");
    assert_eq!(f.inputs.len(), 240, "240 frames of input from 241 rows");
    assert_eq!(f.seed, RNG_STATE as u32, "the original's RNG state, as far as the format fits");
    assert_eq!(f.watches.len(), BIKE.len());
    assert_eq!(f.checks(), BIKE.len() * 241);

    // Row 1's controls are tick 1's input, which is `played(1)`: down, and the stick where it was.
    // The buttons are the game's own bitmask, not the control indices the capture's columns are in.
    assert_eq!(f.inputs[0], Input { buttons: wire_of(1 << 14), stick: (-5, 1) });
    assert_eq!(f.inputs[0].buttons, Control::Down.mask(), "control 14 is down, bit 0x4000");
    assert_eq!(f.inputs[1], Input { buttons: wire_of((1 << 14) | (1 << 6)), stick: (-4, 2) });
    assert_eq!(
        f.inputs[1].buttons,
        Control::Down.mask() | Control::L2.mask(),
        "down and L2 — and L2 is bit 0x0001, which is the whole reason for the conversion",
    );
    assert_eq!(f.inputs[4], Input { buttons: wire_of(1 << 4), stick: (-1, 0) });
    assert_eq!(f.inputs[5], Input { buttons: 0xffff, stick: (0, 1) }, "every button at once");
    assert_eq!(f.inputs[6], Input { buttons: 0, stick: (1, 2) }, "and a frame with nothing held");

    // And it replays tick-exact without a path being touched, because the fields a capture of the
    // original names are fields the port has (P5): the watches are the port's own numbers on the
    // inputs this very capture carries, so a divergence here is the harness or the reader, never the
    // game.
    assert_eq!(
        f.watches.iter().map(|w| w.path.as_str()).collect::<Vec<_>>(),
        AS_THE_PORT_HAS_THEM,
        "the capture's columns are aimed at the port's own paths",
    );
    replay::verify(&f).expect("the reader's ticks line up with the simulation's");
}

/// An acceptance capture is a *real* comparison: every lane it carries exists in the state the harness
/// drives, so the verdict is a tick and a field and never `NoSuchField`.
///
/// This is what P5 bought. Before it, the harness drove A1's example ball while the ridden bike lived
/// behind the shell's own world, so the verdict of a capture of the original was `NoSuchField
/// 'bike.pos[0]'` at tick 0 — the fixture's own fault in A2's taxonomy, and a run that compared
/// nothing. Checked both ways round: the capture passes as it stands, and one bit moved in one lane of
/// one row fails at that row's tick, naming that lane.
#[test]
fn the_acceptance_run_compares_the_bike_the_port_rides() {
    let cap = Capture::parse(&acceptance_csv(60, played)).unwrap();
    let f = acceptance_fixture(&cap).unwrap();
    assert_eq!(f.watches.len(), BIKE.len(), "all eight lanes");
    replay::verify(&f).expect("every lane of the capture is a field of the state");

    // A lane the original moved and the port did not: the last bit of the bike's height on tick 37.
    let mut moved = f.clone();
    let w = moved.watches.iter_mut().find(|w| w.path == "bike.pos[1]").expect("the height");
    let Value::Float(bits) = w.samples[37].value else { panic!("a vector lane is a float") };
    assert_eq!(w.samples[37].tick, 37, "sample 37 is tick 37");
    w.samples[37].value = Value::Float(bits ^ 1);

    let d = replay::verify(&moved).expect_err("a divergence in a watched lane fails the run");
    assert_eq!(d.tick, 37);
    assert!(matches!(&d.reason, Reason::Field { path, .. } if path == "bike.pos[1]"), "{d}");
}

/// The same capture with a chosen basis in its transform's first three rows, leaving the position in
/// row 3 and the velocity alone.
///
/// [`acceptance_rows`] writes an identity basis, which is the one orientation an up-righted rebuild of
/// a forward vector happens to agree with — so a leaning bike has to be written in by hand for the
/// start-state test below to be able to tell the two apart.
fn with_basis(csv: &str, basis: [Vf; 3]) -> String {
    let mut out = String::new();
    for (n, line) in csv.lines().enumerate() {
        let mut cells: Vec<String> = line.split(',').map(str::to_string).collect();
        // The sixteen transform cells are the last sixteen columns, row-major, so the basis is the
        // twelve before the position's four.
        let first = cells.len() - 16;
        if n > 0 {
            for row in 0..3 {
                for col in 0..4 {
                    cells[first + 4 * row + col] = format!("{}", basis[row].0[col]);
                }
            }
        }
        out += &cells.join(",");
        out.push('\n');
    }
    out
}

/// **A fixture starts where the capture starts**, and not at [`Sim::new`]'s start line.
///
/// This is S8's own fix. An acceptance capture is frames out of the middle of a session, so a fixture
/// that falls back to a bare seed is a run that begins with a teleport: the real capture's verdict
/// before this was `tick 0: bike.pos[0] 4309.563 vs 0`, which compares two spawns and says nothing
/// about the simulation. Checked three ways, because each one is a different way to get it wrong:
///
///  1. all eight lanes of the start state are the capture's own first row, bit for bit — which is also
///     what makes tick 0 of the real run agree;
///  2. the *whole basis* survives, not an up-righted copy of it. [`Ride::place`] rebuilds a frame from
///     a forward vector and world up, which is right for a bike put on a start line and wrong for one
///     mid-corner; a capture of a leaning bike proves the difference is not being thrown away.
///  3. it is not the spawn, so a fixture that quietly lost its start state cannot pass this.
#[test]
fn a_fixture_starts_where_the_capture_starts() {
    // A bike leaning left: the basis rolled about its own forward axis (+z) by about 17 degrees,
    // orthonormal to the last bit the literals can carry.
    let basis = [
        Vf::new(0.956_304_8, 0.292_371_7, 0.0, 0.0),
        Vf::new(-0.292_371_7, 0.956_304_8, 0.0, 0.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
    ];
    let cap = Capture::parse(&with_basis(&acceptance_csv(8, played), basis)).unwrap();
    let f = acceptance_fixture(&cap).expect("a well-formed capture");
    let start = f.initial();

    // (1) The eight lanes the capture carries, as the start state holds them.
    let lanes = [
        start.bike_pos().x(),
        start.bike_pos().y(),
        start.bike_pos().z(),
        start.bike_pos().w(),
        start.bike_vel().x(),
        start.bike_vel().y(),
        start.bike_vel().z(),
        start.bike_vel().w(),
    ];
    for (n, path) in AS_THE_PORT_HAS_THEM.iter().enumerate() {
        assert_eq!(
            f.watches[n].samples[0].value,
            Value::Float(lanes[n].to_bits()),
            "{path}: the start state is not the capture's first row",
        );
        assert_eq!(f.watches[n].samples[0].tick, 0, "{path}: the first sample is tick 0");
    }

    // (2) The lean. The state's own orientation is the quaternion, so the matrix is read back out of
    // it: a round trip through `mat4_to_quat`, which is exact to about a part in a million and is not
    // what this is measuring. What it is measuring is that the up axis is the capture's leaning one
    // and not the world's, and those are 0.29 apart — five orders of magnitude more than the trip.
    let up = start.rider.ride.transform().0[1];
    for lane in 0..4 {
        let (got, want) = (up.0[lane], basis[1].0[lane]);
        assert!((got - want).abs() < 1e-5, "up lane {lane}: {got} is not {want}");
    }
    let upright = Sim::new(f.seed);
    let world_up = upright.rider.ride.transform().0[1];
    assert!(
        (world_up.0[0] - up.0[0]).abs() > 0.1,
        "an up-righted basis would be indistinguishable from the capture's, so (2) proves nothing",
    );

    // (3) And it is not the spawn.
    assert_ne!(
        start.bike_pos().y(),
        upright.bike_pos().y(),
        "the fixture started at `Sim::new`'s spawn, which is the bug this test is about",
    );
}

/// **The acceptance gate bites.** A fixture that has lost its start state diverges on tick 0, which is
/// at or before [`ACCEPTANCE_AGREES_THROUGH`], and [`acceptance_verdict`] fails rather than reporting.
///
/// Without this the whole ratchet could be an assert that cannot fire: the verdict reports every
/// divergence the port has today on purpose, so the one thing worth proving is that it still has a
/// floor under it.
#[test]
#[should_panic(expected = "gone backwards")]
fn the_acceptance_gate_fails_when_a_run_stops_starting_where_its_capture_does() {
    let cap = Capture::parse(&acceptance_csv(60, played)).unwrap();
    let mut f = acceptance_fixture(&cap).unwrap();
    f.start = None; // what every acceptance fixture was before S8
    // A synthetic capture is the port's own run over the flat floor, so the flat floor is the world
    // this verdict is taken over — there is no disc in it and nothing to identify.
    acceptance_verdict(&f, &mut game_sim::world::Flat::default());
}

/// The acceptance capture is looked for in its own folder first and in the recorder's default output
/// directory second — the mismatch S8 arrived to find, where the capture was in one and the reader
/// looked only in the other, so the run skipped and said nothing.
#[test]
fn an_acceptance_capture_is_looked_for_where_it_is_kept_and_where_the_recorder_writes_it() {
    let dirs = acceptance_dirs();
    assert_eq!(dirs[0], private_dir("capture").join("acceptance"), "its own folder, first");
    assert_eq!(dirs[1], private_dir("capture"), "and `./kit record -o`'s default, second");
    // Whichever of them holds it, the run finds it: this is the test that would have caught the skip.
    if let Some((path, _)) = acceptance_capture_text() {
        assert!(dirs.iter().any(|d| path.parent() == Some(d)), "{} is in neither", path.display());
    }
}

/// A capture with a gap in its frames is not an acceptance recording, and says so rather than
/// desyncing: the port has nothing to simulate a missing frame's input with.
#[test]
fn a_capture_with_a_gap_is_not_an_acceptance_recording() {
    let csv = acceptance_csv(30, played);
    let mut lines: Vec<&str> = csv.lines().collect();
    lines.remove(12); // frame 101245, somewhere in the middle
    let cap = Capture::parse(&lines.join("\n")).unwrap();
    let e = acceptance_fixture(&cap).expect_err("a frame of input is missing");
    assert!(e.contains("frame_counter 2280 follows 2278"), "{e}");
    assert!(e.contains("every = 1"), "{e}");
}

/// The stick comes from the half of libpad's buffer the game read — the one with the *smaller* stamp —
/// whichever half that is. Taking the newer one gives the next frame's stick, which is wrong on exactly
/// the frames where the input changed, so it is wrong invisibly.
#[test]
fn the_input_comes_from_the_half_the_game_read() {
    let cap = Capture::parse(&acceptance_csv(8, played)).unwrap();
    let f = acceptance_fixture(&cap).unwrap();
    assert_eq!(f.inputs[0].stick, (-5, 1), "frame 1's stick, from half b");

    // The same capture with the stamps the other way round: the game read half a, which this synthetic
    // capture filled with the *next* frame's input, so every stick moves on by one.
    let swapped = cap
        .rows
        .iter()
        .map(|cells| {
            let mut cells = cells.clone();
            let (a, b) = (column(&cap, "stamp_a").unwrap(), column(&cap, "stamp_b").unwrap());
            cells.swap(a, b);
            cells.join(",")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let other = Capture::parse(&format!("{}\n{swapped}", cap.columns.join(","))).unwrap();
    let g = acceptance_fixture(&other).unwrap();
    assert_eq!(g.inputs[0].stick, (-4, 2), "frame 2's stick: the wrong half, by one frame");
    assert_eq!(
        g.inputs.iter().map(|i| i.stick).collect::<Vec<_>>(),
        f.inputs.iter().map(|i| i.stick).skip(1).chain([stick_of(9)]).collect::<Vec<_>>(),
        "a one-frame shift in the input, which no later check would attribute to the pad",
    );
}

/// `played`'s stick for a frame, as the reader decodes it: the centre is 0x7f.
fn stick_of(frame: usize) -> (i8, i8) {
    let (_, (lx, ly)) = played(frame);
    ((i16::from(lx) - 0x7f) as i8, (i16::from(ly) - 0x7f) as i8)
}

/// Five ways a capture can look like an acceptance recording and not be one, each reported as the
/// capture's own problem with the frame it happened on. All five are invisible in a CSV of 9,000 rows,
/// which is why they are checked rather than eyeballed.
#[test]
fn a_capture_that_is_not_of_the_bike_is_refused() {
    let csv = acceptance_csv(30, played);
    let fixture_of = |text: &str| acceptance_fixture(&Capture::parse(text).unwrap());

    // (1) the chain resolved to something that is not a vehicle actor
    let e = fixture_of(&csv.replacen(",4,1,19873344", ",2,1,19873344", 1)).expect_err("type 2");
    assert!(e.contains("bike_type is '2'") && e.contains("0x8 hops"), "{e}");

    // (2) nobody is riding it — a world object, or an actor whose rider has been freed
    let e = fixture_of(&csv.replace(",4,1,19873344", ",4,1,0")).expect_err("no player");
    assert!(e.contains("bike_player is '0'"), "{e}");

    // (2b) *somebody* is riding it, but an AI is: the list entry next door on the acceptance
    // recording's own area, which is a type-4 vehicle with a player record of its own
    let e = fixture_of(&csv.replacen(",4,1,19873344", ",4,2,19873344", 1)).expect_err("an AI's bike");
    assert!(e.contains("bike_ctl_src is '2'") && e.contains("0x8 hops"), "{e}");

    // (3) a pointer chain that went null: the cell is empty, and a fixture cannot span it
    let mut lines: Vec<String> = csv.lines().map(str::to_string).collect();
    let at = column(&Capture::parse(&csv).unwrap(), "bike_vel.y").unwrap();
    let mut cells: Vec<String> = lines[5].split(',').map(str::to_string).collect();
    cells[at] = String::new();
    lines[5] = cells.join(",");
    let e = fixture_of(&lines.join("\n")).expect_err("one empty cell");
    assert!(e.contains("bike_vel.y") && e.contains("30 samples for 31 rows"), "{e}");

    // (4) a dt that is not a number at all: the row is not a frame of play
    let e = fixture_of(&csv.replacen(MEASURED_DT[0], "---", 1)).expect_err("an unreadable dt");
    assert!(e.contains("dt '---' is not a number"), "{e}");
}

/// Ticks come from the game's counter, not the recorder's `frame` column — the trap that would have
/// made every acceptance fixture of this game wrong by a factor of two.
///
/// The simulation ticks once per two video fields, so a capture's `frame` column rises by 2 a row while
/// the game's own `frame_counter` rises by 1. A watch built by `Capture::float` alone is therefore
/// sampled at ticks 0, 2, 4 …, which compares the original's tick 1 against the port's tick 2 — and
/// nothing about that failure says "the tick mapping is doubled": it looks like the simulation is
/// wrong from the first frame.
#[test]
fn the_ticks_come_from_the_games_counter_not_the_recorders_frame() {
    let cap = Capture::parse(&acceptance_csv(30, played)).unwrap();
    assert_eq!(cap.span(), 60, "the recorder's frame column counts video fields");
    assert_eq!(cap.tick_of(30), 60, "which is twice the tick, for this game");
    assert_eq!(ticks_of(&cap).unwrap(), (0..=30).collect::<Vec<u64>>(), "one tick a row");

    let f = acceptance_fixture(&cap).unwrap();
    for w in &f.watches {
        assert_eq!(
            w.samples.iter().map(|s| s.tick).collect::<Vec<_>>(),
            (0..=30).collect::<Vec<u64>>(),
            "the watch on {} is sampled once a tick, not once every other tick",
            w.path,
        );
    }
    // The same watch straight off `Capture`, which knows only about the video counter.
    let raw = cap.float("bike_vel.y", "bike.vel[1]").unwrap();
    assert_eq!(raw.samples[1].tick, 2, "the trap, so that it is recognised rather than investigated");
}

/// The step the original integrated with is recorded and carried, not assumed.
///
/// `dt` is a *measured* number in this game — three distinct values in 300 frames of the front end,
/// and R6 saw 0.041532 in a level — so a recording of real play is a recording of a run whose step
/// varied. The reader does not refuse it (a fixture nobody can build proves nothing) but it does put
/// the spread in the fixture's name, because a port with a fixed step cannot match such a run and that
/// is the first thing to know when the first acceptance run diverges everywhere at once. What to do
/// about it belongs to the port's tick, not to this reader: see S8's journal entry.
#[test]
fn the_step_the_original_measured_is_carried_in_the_fixture() {
    let cap = Capture::parse(&acceptance_csv(30, played)).unwrap();
    let (least, most) = dt_spread(&cap).unwrap();
    assert!(least < most, "a capture of this game does not have one dt: {least} {most}");

    let f = acceptance_fixture(&cap).unwrap();
    assert!(f.name.contains(&format!("dt {least}..{most}")), "{}", f.name);
    assert!(f.name.contains("rng 0x0123456789abcdef"), "{}", f.name);
    assert!(f.name.contains("30 ticks of real play"), "{}", f.name);
}

/// Every capture in `context/capture/` that carries the acceptance spec's input columns is read as
/// inputs — and the two independent sources of the same frame are checked against each other.
///
/// The game's input manager holds sixteen controls; libpad's buffer holds a sixteen-bit button word,
/// active low, in two halves. They are written by different code at different times, so they are a real
/// cross-check on the decode rather than a restatement of it, and between them they settle the two
/// things a reader cannot get from a format document: **the controls are not the wire bits** (they go
/// through [`CONTROL_MASK`]), and **the half the game read is the one with the smaller stamp**.
///
/// Measured here on two 300-frame captures of the front end — one pressing up and down seven times
/// each, one pressing every button `./kit pad` can send that a menu ignores. The word implied by the
/// controls equalled the older half's in 300 of 300 rows in *both*, and the newer half's in 293 and 289
/// — the rows where a press began or ended between the halves being refreshed. Taking the newer half
/// would therefore be right about 97% of the time, which is exactly the kind of wrong that survives
/// every short test and desyncs a five-minute replay.
///
/// Captures are game data and live in the git-ignored `context/`, so this test says what it found and
/// passes when there is nothing there.
#[test]
fn every_input_capture_in_context_agrees_with_the_wire() {
    for path in files_in(&private_dir("capture"), "csv") {
        let text = std::fs::read_to_string(&path).expect("a readable capture");
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        // Only the captures of this spec: anything else in the directory is another task's.
        if column(&cap, "ctl0.held").is_err() || column(&cap, "pad_a").is_err() {
            println!("{}: not an input capture, skipped", path.display());
            continue;
        }
        let held: Vec<usize> =
            (0..CONTROLS).map(|n| column(&cap, &format!("ctl{n}.held")).unwrap()).collect();
        let pad = PadColumns {
            pad_a: column(&cap, "pad_a").unwrap(),
            stamp_a: column(&cap, "stamp_a").unwrap(),
            pad_b: column(&cap, "pad_b").unwrap(),
            stamp_b: column(&cap, "stamp_b").unwrap(),
        };

        let (mut older, mut newer, mut seen) = (0, 0, 0u16);
        for row in 0..cap.rows.len() {
            let mask = held_mask(&cap, row, &held).unwrap_or_else(|e| panic!("{e}"));
            let stamps = |c| cell(&cap, row, c).parse::<u64>().expect("a stamp");
            let (a, b) = (half(&cap, row, pad.pad_a).unwrap(), half(&cap, row, pad.pad_b).unwrap());
            let wire: u16 = (0..CONTROLS)
                .filter(|n| mask & (1 << n) != 0)
                .fold(0, |w, n| w | CONTROL_MASK[n]);
            let word = |h: &[u8]| !(u16::from(h[2]) << 8 | u16::from(h[3]));
            let (o, n) = if stamps(pad.stamp_a) <= stamps(pad.stamp_b) {
                (word(&a), word(&b))
            } else {
                (word(&b), word(&a))
            };
            older += usize::from(o == wire);
            newer += usize::from(n == wire);
            seen |= mask;
        }
        let rows = cap.rows.len();
        println!(
            "{}: {rows} rows, the control mask equals the older half's word in {older} and the \
             newer's in {newer}; controls seen held: 0x{seen:04x}",
            path.display(),
        );
        assert_eq!(
            older, rows,
            "{}: the game's own controls disagree with the libpad half it read — either control N is \
             not wire bit N, or the older half is not the one the game read",
            path.display(),
        );
        // And the capture reads as inputs at all, which is what a fixture needs.
        let inputs = inputs_from_capture(&cap).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(inputs.len(), rows - 1);
        let (least, most) = dt_spread(&cap).unwrap();
        let mut sticks: Vec<(i8, i8)> = inputs.iter().map(|i| i.stick).collect();
        sticks.sort_unstable();
        sticks.dedup();
        println!(
            "{}: {} inputs, dt {least}..{most}, sticks {sticks:?}",
            path.display(),
            inputs.len(),
        );
    }
}

/// The reader and `watch/acceptance.toml` describe the same row. A capture is made by the spec and read
/// by the code above, and nothing but this test connects them: rename a field in the spec and every
/// acceptance capture made afterwards is unreadable, with the failure arriving minutes into a recording
/// session rather than here.
#[test]
fn the_watch_spec_and_this_reader_agree() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../watch/acceptance.toml");
    let spec = std::fs::read_to_string(&path).expect("watch/acceptance.toml is in the repository");
    let flat: String = spec.chars().filter(|c| !c.is_whitespace()).collect();

    for field in ["frame_counter", "dt", "rng_state", "pad_a", "stamp_a", "pad_b", "stamp_b"]
        .into_iter()
        .chain(["bike_type", "bike_ctl_src", "bike_player", "bike_vel", "bike_xform"])
    {
        assert!(flat.contains(&format!("name=\"{field}\"")), "the spec has no field '{field}'");
    }
    // The sixteen control columns come from one [[array]] named `ctl` with a `held` field…
    assert!(flat.contains("name=\"ctl\"") && flat.contains("count=16"));
    assert!(flat.contains("name=\"held\""));
    // …and the row has to be every frame, or the inputs have holes in them.
    assert!(flat.contains("every=1"), "an acceptance recording is one row per frame");
}

// ---------------------------------------------------------------------------------------------------
// The scale the word "acceptance" implies: several minutes, not several hundred frames
// ---------------------------------------------------------------------------------------------------

/// Five minutes at the 30 Hz the running game ticks at (R6 measured it; `dt` is 1/30 in play).
const SEVERAL_MINUTES: usize = 9_000;

/// A fixture of several minutes replays exactly, and is still checking something at the end of it.
///
/// Everything above this works over a few hundred frames. The acceptance run is twenty times that, and
/// the two failures that only appear at length are both silent: a harness that stops comparing (a
/// sample list that ran out, a tick counter that wrapped into nonsense) and a simulation that has
/// settled into a constant, where every later tick agrees because nothing is happening. So this asserts
/// the check count *and* that the state is still moving on the last tick.
#[test]
fn several_minutes_replay_exactly() {
    let f = recorded(SEVERAL_MINUTES);
    assert_eq!(f.inputs.len(), SEVERAL_MINUTES);
    assert_eq!(f.hashes.len(), SEVERAL_MINUTES);
    // One hash a frame, plus one sample a tick for each watched field, ticks 0..=9000 included.
    assert_eq!(f.checks(), SEVERAL_MINUTES + WATCHED.len() * (SEVERAL_MINUTES + 1));

    let end = replay::verify(&f).expect("nine thousand ticks of the port against itself");
    assert_eq!(end.ticks, SEVERAL_MINUTES as u64);

    let penultimate = replay::state_at(&f, SEVERAL_MINUTES as u64 - 1);
    assert_ne!(
        penultimate.hash(),
        end.hash(),
        "the last two ticks of the run are identical: a fixture this long has stopped proving \
         anything and needs a run with something happening in it",
    );
}

/// A divergence nine thousand ticks in is reported at *its* tick, with the field's name — not at tick
/// 1, not at the end, and not as a bare hash.
///
/// The arithmetic of a tick counter is the usual cause of the first two, and it only shows up past the
/// couple of hundred frames the short fixtures cover.
#[test]
fn a_divergence_deep_in_a_long_run_names_its_tick_and_field() {
    let deep = 8_765;
    let mut f = recorded(SEVERAL_MINUTES);
    let Value::Float(bits) = f.watches[0].samples[deep].value else { panic!("a float lane") };
    assert_eq!(f.watches[0].samples[deep].tick, deep as u64);
    assert_eq!(f.watches[0].path, "bike.pos[1]");
    f.watches[0].samples[deep].value = Value::Float(bits ^ 1); // one bit, the last one

    let d = replay::verify(&f).expect_err("one bit, eight thousand ticks in");
    assert_eq!(d.tick, deep as u64);
    assert!(d.to_string().starts_with("tick 8765: bike.pos[1] "), "{d}");

    // And a hash-only divergence just as deep: the same tick, said less usefully, which is what a
    // capture with no watch on the field that moved can tell you.
    let mut g = recorded(SEVERAL_MINUTES);
    g.watches.clear();
    g.hashes[6_999] ^= 1;
    let d = replay::verify(&g).expect_err("a changed hash at tick 7000");
    assert_eq!(d.tick, 7_000);
    assert!(matches!(d.reason, Reason::Hash { .. }), "{d}");
}

/// A fixture of several minutes round-trips through its file form byte for byte, and still replays.
///
/// The short round trip above proves the grammar; this one proves the format at the size it will
/// actually be used at — a few hundred kilobytes of text, nine thousand `frame` lines and thirty-six
/// thousand float patterns, any one of which losing a bit would make the recording subtly wrong rather
/// than unreadable.
#[test]
fn a_long_fixture_round_trips_as_a_file() {
    let f = recorded(SEVERAL_MINUTES);
    let text = f.to_text();
    let again = Fixture::parse(&text).expect("what it just wrote");
    assert_eq!(again.inputs, f.inputs, "nine thousand inputs, buttons and both axes");
    assert_eq!(again.hashes, f.hashes, "nine thousand hashes");
    assert_eq!(again.watches, f.watches, "thirty-six thousand samples, float patterns included");
    assert_eq!(again.to_text(), text, "and writing it again gives the same bytes");
    replay::verify(&again).expect("and it still replays");
}

// ---------------------------------------------------------------------------------------------------
// The acceptance run itself
// ---------------------------------------------------------------------------------------------------

/// **The acceptance run.** Several minutes of real play, replayed tick-exact against the port.
///
/// Two forms, either of which this finds on its own:
///
/// - `context/capture/acceptance/acceptance-freeride.csv`, or the same name loose in
///   `context/capture/` — a capture of `watch/acceptance.toml`, straight off `./kit record`
///   ([`acceptance_dirs`] says why there are two places and which to prefer). Read by
///   [`acceptance_fixture`] above, which also checks it really is a recording of a ridden bike at a
///   fixed step with no frame missing, and which seeds the run's start state from its first row.
/// - `context/fixtures/slow/acceptance-freeride.fixture` — the same thing in the harness's own format,
///   for when the capture has been turned into a fixture file once and for all. **Not equivalent to
///   the capture since S8**: the text format cannot carry a start state yet
///   ([`replay::Fixture::start`]), so a file written out of this capture replays from `Sim::new`'s
///   spawn and diverges on tick 0 — the gate below says so in as many words. The capture is the
///   canonical form of an acceptance run until the format grows a state reader.
///
/// The data is game data — somebody's play of a disc — so it lives in the git-ignored `context/` and is
/// absent on every machine but the one that recorded it, and in every worktree but the one it was
/// recorded in. When neither file is there this says what to record and passes, which is what the
/// capture and recording tests beside it do about an empty directory and for the same reason: a test
/// cannot fail over a file it is not allowed to ship. **The absence is the status of the port, and it
/// is written down in `plan/HUMAN.md` and in this task's journal entry, not here.**
///
/// Not `#[ignore]`d since S8: the capture exists now, so `./kit check` runs the real comparison
/// wherever the data is, and prints the one line of verdict ([`acceptance_verdict`]) wherever it is
/// not. See `every_acceptance_capture_is_the_original_at_a_step_the_port_can_match` at the end of this
/// file for what the captures in hand are, and what they do not yet contain.
///
/// ```sh
/// cargo test -p game-sim --test replay -- --nocapture the_acceptance_run
/// ```
#[test]
fn the_acceptance_run() {
    let fixture = private_dir("fixtures").join("slow").join(format!("{ACCEPTANCE}.fixture"));
    let announce = |f: &Fixture| {
        println!(
            "{}: {} frames, {} checks, seed 0x{:08x}",
            f.name,
            f.inputs.len(),
            f.checks(),
            f.seed,
        );
        assert!(
            f.inputs.len() >= 60 * 29,
            "{} frames is not an acceptance run: the point is minutes of play, where the hundredth \
             awkward landing is",
            f.inputs.len(),
        );
    };

    // A fixture *file*, which carries neither a start state nor a world: the gate below says so in as
    // many words, and the flat floor is the only world it can be replayed over.
    if fixture.exists() {
        let text = std::fs::read_to_string(&fixture).expect("a readable fixture");
        let f = Fixture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", fixture.display()));
        announce(&f);
        acceptance_verdict(&f, &mut game_sim::world::Flat::default());
        return;
    }

    let Some((capture, cap)) = acceptance_capture() else { return };
    println!(
        "{}: {} rows x {} columns, frames {}..{}",
        capture.display(),
        cap.frames.len(),
        cap.columns.len(),
        cap.frames[0],
        cap.frames[cap.frames.len() - 1],
    );
    // The world the capture was recorded on and the bike it was recorded with, both off the disc
    // (S28). Without them this is a minute of Free09 replayed over a floor at y = 0 on a stand-in
    // 250, which is the run S8 left diverging at tick 1.
    let taken = in_the_world_of(&cap, |world, named, bike| {
        let f = acceptance_fixture_on(&cap, bike)
            .unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
        let f = Fixture { world: Some(named.clone()), ..f };
        announce(&f);
        acceptance_verdict(&f, world);
    });
    if taken.is_none() {
        println!(
            "— so there is no acceptance verdict in this worktree: the capture is here and the \
             world it was ridden on is not. The ratchet is unchecked, not met.",
        );
    }
}

// ===================================================================================================
// I3 — the pad tape: a played sequence recorded, stored, and played back through the port
// ===================================================================================================
//
// A fixture carries one `Input` per frame, which is what the simulation reads. A **tape** carries the
// frame the *pad* delivered: the 32 bytes of each pad's report and the `dt` the engine measured. The
// difference is the whole of this section:
//
//   * a report holds the twelve pressures, and the game reads twelve of its sixteen controls from
//     those bytes rather than from the button word — so a recording of a mask cannot reproduce a
//     control's strength, and a *capture* of a mask cannot be turned back into one;
//   * the input layer is stateful (the press and release timers, the one-frame event queue), so the
//     frames have to be played *through* it, in the original's order, and not merely listed;
//   * `dt` is measured by this engine rather than assumed, and the timers accumulate `dt * 1000`.
//
// So the tape is what a human-played sequence from the real game is recorded as, and `Playback` is the
// port being driven by one. The proofs below are: the frame decodes the way the game's own poll does,
// the file round-trips bit for bit, a capture becomes the frames the game actually read (the right
// half of the pad library's buffer, every tick, aligned to the fixture's tick numbering), a playback
// reads the pad before it simulates, and — the one that is evidence rather than plumbing — every real
// capture in `context/capture/` replays through the port with the game's own sixteen controls agreeing
// row for row.

/// The two halves of the control map restated in this file are the map the port measured.
///
/// `CONTROL_MASK` and `CONTROL_PRESSURE` are written out here so that the synthetic captures above are
/// not built from the same table they are checked against. That only works while the two agree, and
/// nothing but this test says they do.
#[test]
fn the_tables_in_this_file_are_the_map_the_port_measured() {
    assert_eq!(CONTROLS, PORT_CONTROLS);
    for n in 0..CONTROLS {
        assert_eq!(CONTROL_MASK[n], CONTROL_MAP[n].mask, "control {n}'s bit");
        let pressure = CONTROL_MAP[n].is_analogue().then_some(CONTROL_MAP[n].pressure as usize);
        assert_eq!(CONTROL_PRESSURE[n], pressure, "control {n}'s pressure byte");
    }
    // And the sixteen bits are a permutation of the sixteen controls, which is why "everything at
    // once" is 0xffff in either space and why only the single-control cases above catch a mix-up.
    assert_eq!(wire_of(0xffff), 0xffff);
    assert_eq!(wire_of(1 << 6), Control::L2.mask());
}

/// A frame's controls are whatever the game's own poll makes of its report — pressures included.
///
/// The three cases that a recording of a button mask would get wrong: an analogue control is read from
/// its pressure byte and **not** from its bit, a digital one from its bit, and a pad whose status byte
/// says it is not there holds nothing at all.
#[test]
fn a_frames_controls_are_what_the_games_own_poll_makes_of_it() {
    let dt = replay::CAPPED_DT;

    // Cross's bit set, its pressure byte zero: the wire says pressed and the game does not.
    let mut bytes = Report::idle().0;
    let low = !Control::Cross.mask();
    bytes[2] = (low >> 8) as u8;
    bytes[3] = low as u8;
    let bit_only = PadFrame::new(dt, [Report::from_bytes(bytes), Report::missing()]);
    assert_eq!(bit_only.reports[0].buttons(), Control::Cross.mask(), "the word says cross");
    assert_eq!(bit_only.held_mask(0), 0, "and the game reads cross from its pressure byte");

    // The same press with the pressure a real pad reports.
    bytes[CONTROL_MAP[Control::Cross.index()].pressure as usize] = 0xff;
    let real = PadFrame::new(dt, [Report::from_bytes(bytes), Report::missing()]);
    assert_eq!(real.held_mask(0), Control::Cross.mask());
    assert_eq!(real.input().buttons, Control::Cross.mask());

    // A digital control has no pressure byte, so its bit is all there is.
    let start = PadFrame::of_buttons(dt, Control::Start.mask());
    assert_eq!(start.held_mask(0), Control::Start.mask());
    // And `of_buttons` is the digital source: full pressure for the analogue ones, so they read back.
    let both = PadFrame::of_buttons(dt, Control::Start.mask() | Control::Cross.mask());
    assert_eq!(both.held_mask(0), Control::Start.mask() | Control::Cross.mask());

    // No pad: nothing held, whatever the rest of the bytes say.
    assert_eq!(PadFrame::default().held_mask(0), 0);
    assert_eq!(PadFrame::default().stick(0), (0, 0), "a missing pad's sticks are centred");

    // The stick as it arrived: the centre byte off, and the one value that cannot be represented
    // wrapped (0xff - 0x7f is 128, which is not an i8): -128, which [`Sim::reports`] puts back as 0xff
    // (S94; clamped to 127 it came back 0xfe, and a full stick replayed short of full).
    let stick = |x: u8, y: u8| {
        let mut bytes = Report::idle().0;
        bytes[6] = x;
        bytes[7] = y;
        PadFrame::new(dt, [Report::from_bytes(bytes), Report::missing()]).stick(0)
    };
    assert_eq!(stick(0x7f, 0x7f), (0, 0));
    assert_eq!(stick(0x89, 0x75), (10, -10));
    assert_eq!(stick(0x00, 0xff), (-127, -128), "hard left, hard down, with 0xff wrapped");
    for b in 0..=255u8 {
        let mut bytes = Report::idle().0;
        bytes[6] = b;
        bytes[7] = b;
        let frame = PadFrame::new(dt, [Report::from_bytes(bytes), Report::missing()]);
        let back = Sim::reports(Input { buttons: 0, stick: frame.stick(0) })[0];
        assert_eq!((back.axis(2), back.axis(3)), (b, b), "stick byte {b:#04x} does not survive an Input");
    }
}

/// A tape round-trips through its file form byte for byte, with every byte of every report and every
/// bit of every `dt`.
#[test]
fn a_tape_round_trips_as_a_file() {
    let mut tape = PadTape::new("hand-made");
    tape.push(PadFrame::of_buttons(replay::CAPPED_DT, Control::Cross.mask()));
    tape.push(PadFrame::new(F(0.03283332), [Report::idle(), Report::idle()]));
    tape.push(PadFrame::default());
    let mut odd = Report::idle().0;
    odd[0] = 0; // a good read…
    odd[19] = 0x1a; // …with R2 just over the threshold, which is a value a mask cannot hold
    tape.push(PadFrame::new(F(0.041532), [Report::from_bytes(odd), Report::missing()]));

    let text = tape.to_text();
    let back = PadTape::parse(&text).expect("what it just wrote");
    assert_eq!(back, tape, "every report byte, every dt bit, and the name");
    assert_eq!(back.to_text(), text, "and writing it again gives the same bytes");
    assert_eq!(back.frames[3].held_mask(0), Control::R2.mask(), "0x1a is above the threshold");
    assert_eq!(
        back.dt_range().map(|(a, b)| (a.to_bits(), b.to_bits())),
        Some((F(0.03283332).to_bits(), F(0.041532).to_bits())),
        "the spread of a run the original measured frame by frame",
    );
}

/// A tape as a human would type one: the mask form, the bytes form, `-` for an absent pad, and a `dt`
/// written as a decimal.
#[test]
fn a_hand_written_tape_reads_as_it_looks() {
    let tape = PadTape::parse(
        "# two frames\n\
         format 1\n\
         tape by-hand\n\
         frame 0.03333 *0x0040            # cross, and no second pad\n\
         frame 0x3d0699a3 0x0079fffb7f7f7f7f0000000000000000ff000000 -\n",
    )
    .expect("the documented format");

    assert_eq!(tape.name, "by-hand");
    assert_eq!(tape.len(), 2);
    assert_eq!(tape.frames[0].dt().to_bits(), F(0.03333).to_bits());
    assert_eq!(tape.frames[0].held_mask(0), Control::Cross.mask());
    assert_eq!(tape.frames[0].reports[1], Report::missing(), "an omitted pad is an absent pad");
    // The second frame is a row of a real capture: L1, which the game reads from pressure byte 16.
    assert_eq!(tape.frames[1].reports[0].mode(), 0x79);
    assert_eq!(tape.frames[1].held_mask(0), Control::L1.mask());
    assert_eq!(tape.frames[1].stick(0), (0, 0));
    assert_eq!(tape.inputs(), vec![
        Input { buttons: Control::Cross.mask(), stick: (0, 0) },
        Input { buttons: Control::L1.mask(), stick: (0, 0) },
    ]);
}

/// A typo in a tape says which line, like a fixture's: a recording that silently loses a frame is a
/// recording that desyncs.
#[test]
fn a_broken_tape_file_says_which_line() {
    let cases = [
        ("format 1\nframes 3\n", 2, "unknown keyword"),
        ("format 2\n", 1, "this build reads"),
        ("format x\n", 1, "wants a version number"),
        ("frame\n", 1, "frame wants"),
        ("frame 0.03 - - -\n", 1, "frame wants"),
        ("frame later -\n", 1, "dt 'later' is not a number"),
        ("frame 0.03 *0x1ffff\n", 1, "not a 16-bit bitmask"),
        ("frame 0.03 0x0079ff\n", 1, "is not a pad report"),
        ("frame 0.03 0x0079fffb7f7f7f7g\n", 1, "is not hex"),
    ];
    for (text, line, needle) in cases {
        let e = PadTape::parse(text).unwrap_err();
        assert_eq!(e.line, line, "{text:?} -> {e}");
        assert!(e.to_string().contains(needle), "{text:?} -> {e}");
    }
}

/// A capture of the original becomes the pad frames the game actually read.
///
/// Four properties, each of them a silent desync when it is wrong: the frame count and its alignment
/// to the harness's ticks, the half of the pad library's buffer the frame comes from, the `dt` of each
/// frame, and the controls — which must come out as the sixteen the *game* believed were held, since
/// that is what the simulation reads.
#[test]
fn a_capture_becomes_the_pad_frames_the_game_read() {
    let cap = Capture::parse(&acceptance_csv(240, played)).expect("the recorder's format");
    let tape = PadTape::from_capture(&cap).expect("a well-formed input capture");

    assert_eq!(cap.rows.len(), 241);
    assert_eq!(tape.len(), 240, "row 0's input belongs to the tick before the recording");

    // The controls the game itself recorded, row for row: the capture's own `ctlN.held` columns, which
    // are written by different code than the pad buffer the tape is decoded from.
    let held: Vec<usize> =
        (0..CONTROLS).map(|n| column(&cap, &format!("ctl{n}.held")).unwrap()).collect();
    for (k, frame) in tape.frames.iter().enumerate() {
        let row = k + 1;
        let want = wire_of(held_mask(&cap, row, &held).unwrap());
        assert_eq!(frame.held_mask(0), want, "frame {k} (row {row}) of the recording");
        assert_eq!(frame.input().buttons, want);
        // And the frame's own dt, not the cap and not the next row's.
        let dt = cell(&cap, row, column(&cap, "dt").unwrap());
        assert_eq!(frame.dt().to_bits(), dt.parse::<f32>().unwrap().to_bits(), "frame {k}'s dt");
    }
    assert_eq!(tape.inputs(), inputs_from_capture(&cap).unwrap(), "the reader above is this one");

    // The half with the smaller stamp is the half the game read. This synthetic capture puts the
    // *next* frame's input in the newer half, so reading the wrong one shifts everything by a frame —
    // which is right 97% of the time on a real recording, and wrong on exactly the frames that matter.
    let swapped: String = cap
        .rows
        .iter()
        .map(|cells| {
            let mut cells = cells.clone();
            cells.swap(column(&cap, "stamp_a").unwrap(), column(&cap, "stamp_b").unwrap());
            cells.join(",")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let other = Capture::parse(&format!("{}\n{swapped}", cap.columns.join(","))).unwrap();
    let wrong = PadTape::from_capture(&other).unwrap();
    assert_eq!(
        wrong.inputs().iter().map(|i| i.buttons).collect::<Vec<_>>(),
        tape.inputs().iter().map(|i| i.buttons).skip(1).chain([wire_of(played(241).0)]).collect::<Vec<_>>(),
        "the other half is the next frame's input",
    );
}

/// A capture with a tick missing is not an input recording, and says so with the row.
///
/// Two counters, because a capture has two: the game's own `frame_counter`, which rises by 1 a tick,
/// and the recorder's `frame`, which counts video fields and rises by 2 in this game. A capture with
/// neither column is not a capture at all — `frame` is the one column the format guarantees.
#[test]
fn a_capture_with_a_tick_missing_is_refused() {
    // With the game's counter: the step is 1, and a dropped row makes it 2.
    let csv = acceptance_csv(30, played);
    let mut lines: Vec<&str> = csv.lines().collect();
    lines.remove(12);
    let cap = Capture::parse(&lines.join("\n")).unwrap();
    let e = PadTape::from_capture(&cap).expect_err("a frame of input is missing");
    assert!(e.to_string().contains("frame_counter 2280 follows 2278"), "{e}");
    assert!(e.to_string().contains("every = 1"), "{e}");

    // Without it: the stride of the recorder's `frame` column, whatever it is, has to be constant.
    let vsync = |frames: &[u64]| {
        let mut csv = String::from("frame,dt,pad_a,stamp_a,pad_b,stamp_b\n");
        for (k, frame) in frames.iter().enumerate() {
            let pad = format!("0x{}", hex20(Control::Cross.mask()));
            csv += &format!("{frame},0.03333,{pad},{},{pad},{}\n", 2 * k + 1, 2 * k);
        }
        Capture::parse(&csv).unwrap()
    };
    let even = vsync(&[900, 902, 904, 906]);
    assert_eq!(PadTape::from_capture(&even).unwrap().len(), 3, "a video-field stride of 2 is fine");
    let e = PadTape::from_capture(&vsync(&[900, 902, 906, 908])).expect_err("a dropped frame");
    assert!(e.to_string().contains("frame 906 follows 902"), "{e}");
    assert!(e.to_string().contains("a step of 4"), "{e}");
    assert_eq!(e.line, 4, "the row it happened on");

    // And the columns a tape needs are named when they are absent, rather than guessed at.
    let bare = Capture::parse("frame,bike_pos.y\n900,1.0\n901,2.0\n").unwrap();
    let e = PadTape::from_capture(&bare).expect_err("not an input capture");
    assert!(e.to_string().contains("no column 'dt'"), "{e}");
}

/// The first 20 bytes of a pad report holding `buttons`, as a capture's `bytes:20` column writes them:
/// every byte the game's poll reads.
fn hex20(buttons: u16) -> String {
    Report::from_buttons(buttons).0[..20].iter().map(|b| format!("{b:02x}")).collect()
}

/// A playback reads the pad *before* it simulates, and the input layer is carried from frame to frame.
///
/// This is why a tape is played rather than listed: the timers and the event queue are state, and a
/// harness that fed the simulation a list of masks would have neither.
#[test]
fn a_playback_reads_the_pad_before_it_simulates() {
    let dt = replay::CAPPED_DT;
    let mut tape = PadTape::new("a press, held, then let go");
    for _ in 0..3 {
        tape.push(PadFrame::of_buttons(dt, Control::Cross.mask()));
    }
    for _ in 0..2 {
        tape.push(PadFrame::of_buttons(dt, 0));
    }

    let mut run = Playback::new(7);
    run.advance(&tape.frames[0]);
    assert_eq!(run.tick(), 1, "one frame of input is one tick of the simulation");
    assert!(run.input().pad(0).held(Control::Cross), "the press is latched before the tick that acts");
    assert!(run.input().pad(0).pressed(Control::Cross), "and this is its press frame");
    assert_eq!(run.input().events.len(), 1, "one transition, on the frame it happened");
    assert_eq!(run.input().pad(0).control(Control::Cross).held_ms, 0);

    run.advance(&tape.frames[1]);
    assert!(!run.input().pad(0).pressed(Control::Cross), "the edge does not survive the frame");
    assert_eq!(run.input().pad(0).control(Control::Cross).held_ms, 33, "a flat +33 at the cap");
    assert!(run.input().events.is_empty(), "the queue is cleared at the top of every update");

    run.advance(&tape.frames[3]);
    assert!(run.input().pad(0).released(Control::Cross));
    assert_eq!(run.input().events.len(), 1);
    assert_eq!(run.input().events.events()[0].pressed, 0, "a release");

    // The whole tape in one call, and the same place.
    let whole = replay::play(&tape, 7);
    assert_eq!(whole.tick(), 5);
    assert_eq!(whole.hash(), replay::play(&tape, 7).hash(), "a replay is a function of its tape");

    // The hash covers the input layer as well as the simulation, because since P5 the pad *is* part of
    // the state: [`Sim`] owns the `Manager` the ride reads. A tape that holds the throttle and a tape
    // that holds nothing end in different states for the obvious reason — and so do two tapes the
    // physics cannot tell apart, which is the property a playback hash is for. `Select` is a button no
    // `ride::Controls` lane reads, so it moves the bike nowhere and the hash still notices it.
    let idle: PadTape = PadTape { frames: vec![PadFrame::of_buttons(dt, 0); 5], ..tape.clone() };
    let held = PadFrame::of_buttons(dt, Control::Select.mask());
    let unread: PadTape = PadTape { frames: vec![held; 5], ..tape.clone() };
    assert_ne!(replay::play(&idle, 7).hash(), whole.hash(), "cross is the throttle");
    assert_eq!(
        replay::play(&idle, 7).sim.bike_pos(),
        replay::play(&unread, 7).sim.bike_pos(),
        "a button the ride does not read moves the bike nowhere",
    );
    assert_ne!(
        replay::play(&unread, 7).hash(),
        replay::play(&idle, 7).hash(),
        "and the pad remembers it anyway",
    );

    // A tape played from a mid-session state starts there: a recording that begins at a save state.
    let mut middle = Sim::new(7);
    middle.tick(Input::default());
    let from = replay::play_from(&tape, middle);
    assert_eq!(from.tick(), 6, "the state's own tick count carries on");
}

/// A fixture of pad frames: the tape's own inputs, replayed against the port's recording of the same
/// run — the shape an acceptance fixture has once the input side comes from a capture.
#[test]
fn a_fixture_of_pad_frames_replays() {
    let cap = Capture::parse(&acceptance_csv(240, played)).unwrap();
    let tape = PadTape::from_capture(&cap).unwrap();

    let bare = Fixture::from_tape(&tape, 0x1234_5678);
    assert_eq!(bare.inputs, tape.inputs(), "one input per recorded frame");
    assert_eq!(bare.last_tick(), 240);
    assert_eq!(bare.checks(), 0, "a tape expects nothing on its own: the watches are the capture's");

    // The port's own run over the recorded input, which is a regression fixture of a real session.
    let f = replay::record("played", bare.seed, &bare.inputs, &WATCHED).unwrap();
    assert_eq!(f.inputs, bare.inputs);
    assert_eq!(f.hashes.len(), 240);
    replay::verify(&f).expect("the port agrees with its own run of the recorded input");

    // And it survives the file: a session recorded once is replayable a year later.
    let text = f.to_text();
    let back = Fixture::parse(&text).expect("the fixture format");
    assert_eq!(back.inputs, f.inputs);
    replay::verify(&back).expect("still replaying");
}

/// The tape at the scale of the acceptance run: nine thousand frames through the file and through the
/// port, since a format that loses a byte in 700 kB and a playback that drifts after a few thousand
/// ticks are both invisible in a test of five frames.
#[test]
fn a_long_tape_round_trips_and_plays() {
    let dt = replay::CAPPED_DT;
    let mut tape = PadTape::new("nine thousand frames");
    for k in 0..SEVERAL_MINUTES {
        // Something held on most frames, nothing on some, and the stick wandering.
        let buttons = wire_of(match k % 7 {
            0 => 0,
            6 => 0xffff,
            n => (1 << n) | (1 << (n + 8)),
        });
        let mut report = Report::from_buttons(buttons).0;
        report[6] = (0x7f + (k % 61) as u8).wrapping_sub(30);
        report[7] = (0x7f + (k % 37) as u8).wrapping_sub(18);
        tape.push(PadFrame::new(dt, [Report::from_bytes(report), Report::missing()]));
    }

    let text = tape.to_text();
    assert_eq!(PadTape::parse(&text).expect("nine thousand frames"), tape);

    let run = replay::play(&tape, 0x1234_5678);
    assert_eq!(run.tick(), SEVERAL_MINUTES as u64);
    // The timers are still counting at the end of it: a nine-thousand-frame playback where the input
    // layer had stopped moving would agree with anything.
    let longest = Control::ALL
        .iter()
        .map(|c| run.input().pad(0).control(*c).held_ms)
        .max()
        .unwrap();
    assert!(longest > 0, "nothing was held at the end of the run");
    assert_eq!(Fixture::from_tape(&tape, 1).inputs.len(), SEVERAL_MINUTES);
}

/// **A recording of the real game, played back through the port.**
///
/// Every capture in `context/capture/` that carries the pad columns becomes a tape and is played
/// through [`Playback`]; the sixteen controls the *game* recorded for that frame are compared against
/// the sixteen the port's input layer makes of the same pad bytes, row for row. The capture's control
/// columns and its pad buffer are written by different code at different times, so this is a real
/// comparison and not a restatement of the decode.
///
/// What it proves, and does not: the input side of a replay — that a human-played sequence from the
/// real game, recorded by the kit, drives the port's input layer to the state the original was in. The
/// simulation those inputs now drive is the ride itself (P5), but nothing here compares its numbers
/// against the original's; the *simulation* side of the acceptance run is `the_acceptance_run` above
/// and waits for its recording.
///
/// Captures are game data and live in the git-ignored `context/`, so this says what it found and
/// passes when there is nothing there — and counts a capture of a pad *at rest* apart from a capture
/// of play, because the first one proves the idle decode and claims nothing about the rest.
#[test]
fn a_recording_of_the_real_game_plays_back_through_the_port() {
    let dir = private_dir("capture");
    let (mut proved, mut idle) = (0, 0);
    for path in files_in(&dir, "csv") {
        let text = std::fs::read_to_string(&path).expect("a readable capture");
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if column(&cap, "ctl0.held").is_err() || column(&cap, "pad_a").is_err() {
            println!("{}: not an input capture, skipped", path.display());
            continue;
        }
        let tape = PadTape::from_capture(&cap)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(tape.len(), cap.rows.len() - 1);

        let held: Vec<usize> =
            (0..CONTROLS).map(|n| column(&cap, &format!("ctl{n}.held")).unwrap()).collect();
        let events = column(&cap, "events").ok();
        let mut run = Playback::new(0);
        let (mut transitions, mut seen) = (0usize, 0u16);
        for (k, frame) in tape.frames.iter().enumerate() {
            let row = k + 1;
            let before = run.input().pad(0).held_mask();
            run.advance(frame);
            let got = run.input().pad(0).held_mask();
            let want = wire_of(held_mask(&cap, row, &held).unwrap_or_else(|e| panic!("{e}")));
            assert_eq!(
                got,
                want,
                "{}: frame {} (row {row}): the port holds 0x{got:04x} where the game held \
                 0x{want:04x}",
                path.display(),
                cap.frames[row],
            );
            if let Some(at) = events {
                let want: usize = cell(&cap, row, at).parse().expect("an event count");
                assert_eq!(
                    run.input().events.len(),
                    want,
                    "{}: row {row}: the transitions the game raised",
                    path.display(),
                );
            }
            transitions += (before != got) as usize;
            seen |= got;
        }
        let (least, most) = tape.dt_range().expect("a non-empty tape");
        println!(
            "{}: {} frames played back, {transitions} transitions, controls 0x{seen:04x}, \
             dt {least:?}..{most:?}",
            path.display(),
            tape.len(),
        );
        // The tape of a real recording survives its file form too, bytes and dt alike.
        let again = PadTape::parse(&tape.to_text()).expect("a tape of a real capture");
        assert_eq!(again, tape);
        assert_eq!(replay::play(&again, 0).hash(), run.hash(), "and plays back to the same state");
        // A recording of a pad nobody touched passes every assertion above — the port's idle decode
        // against the game's idle controls, which is worth having and is not a recording of *play*.
        // So the two are counted apart rather than one of them failing: which captures are on this
        // disk is not something a test gets to assert (S8's acceptance capture is one of these, taken
        // under no input at all because PCSX2 would not play its recording back).
        if transitions == 0 {
            idle += 1;
        } else {
            proved += 1;
        }
    }
    println!("{proved} captures of a pad being used, {idle} of one at rest");
    if proved == 0 {
        println!(
            "no capture in {} holds a control changing, so the input path is unproved by this \
             directory — record one with `./kit record watch/input.toml --frames 220 \
             --out context/capture/i3-input.csv` while a pad script drives the pad",
            dir.display(),
        );
    }
}

// ---------------------------------------------------------------------------------------------------
// A human's PCSX2 input recording (.p2m2)
// ---------------------------------------------------------------------------------------------------

/// One pad entry as PCSX2 records it, from the game's own bitmask: the button word active low, the
/// right stick centred and the left where it was asked for, and a full pressure byte for every
/// analogue control that is down — which is what a digital host pad gives it, and what the one
/// recording in the project holds (0 or 255 and nothing between).
///
/// Written here from `CONTROL_MAP`'s *report* indices shifted by the two header bytes a file entry does
/// not have, so a test that passes is a test of the reader's own shift and not of a copy of it.
fn p2m2_entry(buttons: u16, lx: u8, ly: u8) -> [u8; replay::P2M2_PAD_BYTES] {
    let mut entry = [0u8; replay::P2M2_PAD_BYTES];
    let low = !buttons;
    entry[0] = (low >> 8) as u8;
    entry[1] = low as u8;
    entry[2] = game_sim::input::AXIS_CENTRE;
    entry[3] = game_sim::input::AXIS_CENTRE;
    entry[4] = lx;
    entry[5] = ly;
    for control in Control::ALL {
        let binding = control.binding();
        if binding.is_analogue() && buttons & binding.mask != 0 {
            entry[binding.pressure as usize - 2] = 0xff;
        }
    }
    entry
}

/// A `.p2m2` as PCSX2 writes one: the 570-byte header, the frame slot it never writes, then one frame
/// per video field with the second port left unplugged.
fn p2m2_file(author: &str, fields: &[[u8; replay::P2M2_PAD_BYTES]]) -> Vec<u8> {
    let mut out = vec![0u8; replay::P2M2_HEADER_BYTES];
    out[0] = replay::P2M2_FILE_VERSION;
    let put = |out: &mut Vec<u8>, at: usize, text: &str| {
        out[at..at + text.len()].copy_from_slice(text.as_bytes());
    };
    put(&mut out, 1, "PCSX2-v2.9.127");
    put(&mut out, 51, author);
    put(&mut out, 306, "MX Unleashed");
    out[561..565].copy_from_slice(&(fields.len() as u32).to_le_bytes());
    out[565..569].copy_from_slice(&3u32.to_le_bytes());
    out[569] = 1;
    out.extend_from_slice(&[0u8; replay::P2M2_FRAME_BYTES]);
    for entry in fields {
        out.extend_from_slice(entry);
        out.extend_from_slice(&[0u8; replay::P2M2_PAD_BYTES]); // port 1: never written
    }
    out
}

/// A recording's header is read where PCSX2 put it, and its frames are the slots after the one it
/// never writes.
#[test]
fn a_pcsx2_recording_reads_as_its_header_says() {
    let fields = vec![p2m2_entry(Control::Cross.mask(), 0x7f, 0x7f); 9];
    let movie = replay::P2m2::parse(&p2m2_file("rycs", &fields)).expect("PCSX2's own format");

    assert_eq!(movie.file_version, replay::P2M2_FILE_VERSION);
    assert_eq!(movie.emulator, "PCSX2-v2.9.127");
    assert_eq!(movie.author, "rycs");
    assert_eq!(movie.game, "MX Unleashed");
    assert_eq!(movie.total_frames, 9);
    assert_eq!(movie.undo_count, 3);
    assert!(movie.from_savestate, "and so only replayable against the state beside it");
    assert_eq!(movie.fields(), 9, "the written fields, frame 0 dropped");
    // Nine fields is five ticks of the first field of each pair and four of the second.
    assert_eq!(movie.ticks(replay::TickPhase::First), 5);
    assert_eq!(movie.ticks(replay::TickPhase::Second), 4);
}

/// **The byte mapping**: eighteen recorded bytes become the twenty the game's poll reads, and the
/// controls the port derives from them are the ones the human held.
#[test]
fn a_recorded_pad_entry_becomes_the_bytes_the_games_poll_reads() {
    let held = Control::Cross.mask() | Control::L1.mask() | Control::Start.mask();
    let entry = p2m2_entry(held, 0x20, 0xe0);
    let report = replay::report_of_p2m2(&entry);

    assert_eq!(report.status(), 0, "a recorded frame is a read that happened");
    assert_eq!(report.mode(), game_sim::input::MODE_PRESSURES, "the mode the pressures imply");
    assert_eq!(report.buttons(), held, "the word, big-endian and back to active high");
    assert_eq!((report.axis(0), report.axis(1)), (0x7f, 0x7f), "the right stick, centred");
    assert_eq!((report.axis(2), report.axis(3)), (0x20, 0xe0), "the left stick, where it was");
    assert_eq!(report.pressure(Control::Cross.binding().pressure), 0xff);
    assert_eq!(report.pressure(Control::L1.binding().pressure), 0xff);
    assert_eq!(report.pressure(Control::Circle.binding().pressure), 0x00);
    assert_eq!(report.0[20..], [0u8; game_sim::input::REPORT_BYTES - 20], "nothing past the pressures");

    // And through the game's own poll: the pressures are what twelve of the sixteen are read from, so
    // a mapping that put them anywhere else would leave cross and L1 released however good the word is.
    let frame = PadFrame::new(replay::CAPPED_DT, [report, Report::missing()]);
    assert_eq!(frame.held_mask(0), held);
    assert_eq!(frame.stick(0), (-95, 97), "the centre byte taken off");
}

/// An entry PCSX2 never wrote is no pad, not a pad holding everything.
///
/// Eighteen zero bytes are an active-low word with every bit down and both sticks hard into a corner.
/// That is what port 1 of a one-pad recording looks like, on every frame, and reading it literally
/// would replay a session with a second rider leaning into the scenery.
#[test]
fn an_unwritten_pad_entry_is_no_pad_not_every_button() {
    assert_eq!(replay::report_of_p2m2(&[0u8; replay::P2M2_PAD_BYTES]), Report::missing());

    let fields = vec![p2m2_entry(0, 0x7f, 0x7f); 4];
    let movie = replay::P2m2::parse(&p2m2_file("rycs", &fields)).unwrap();
    assert_eq!(movie.field(0, 1), Report::missing(), "port 1 was never plugged in");
    assert_eq!(movie.field(0, 0).status(), 0, "port 0 was");
    assert_eq!(movie.field(99, 0), Report::missing(), "and past the end there is nothing");

    let tape = movie.tape(replay::TickPhase::First);
    assert!(tape.frames.iter().all(|f| f.reports[1] == Report::missing()));
    assert_eq!(replay::play(&tape, 0).input().pad(1).held_mask(), 0, "so pad 1 holds nothing");
}

/// **The file's own proof of the byte mapping.** Twelve of the sixteen controls are recorded twice —
/// as a bit of the word and as a pressure byte — and the two must agree on every frame. A pairing
/// that is off by one disagrees immediately, which is what makes this a measurement rather than a
/// convention.
#[test]
fn the_pressure_bytes_of_a_recording_prove_the_byte_mapping() {
    let fields: Vec<_> = (0..20u16)
        .map(|k| p2m2_entry(wire_of((1 << (k % CONTROLS as u16)) as u16), 0x7f, 0x7f))
        .collect();
    let movie = replay::P2m2::parse(&p2m2_file("rycs", &fields)).unwrap();
    // Twelve analogue controls per written pad, and port 1 is not a pad at all.
    assert_eq!(movie.check_pressures().expect("the file agrees with itself"), 20 * 12);

    // The same recording with the pressures rotated by one control: the file still parses, every
    // report still looks like a pad, and the first frame that holds anything says what is wrong.
    let rotated: Vec<_> = fields
        .iter()
        .map(|entry| {
            let mut e = *entry;
            e[6..18].rotate_right(1);
            e
        })
        .collect();
    let movie = replay::P2m2::parse(&p2m2_file("rycs", &rotated)).unwrap();
    let e = movie.check_pressures().expect_err("a rotated pairing cannot agree");
    assert!(
        e.message.contains("by its bit") && e.message.contains("by its pressure byte"),
        "it says which half of the entry disagreed: {e}",
    );

    // A recording of a host pad that reported no pressures at all is refused the same way, because the
    // game reads twelve of its sixteen controls from those bytes and would see nothing held.
    let mut flat = p2m2_entry(Control::Cross.mask(), 0x7f, 0x7f);
    flat[6..18].fill(0);
    let movie = replay::P2m2::parse(&p2m2_file("rycs", &[flat; 2])).unwrap();
    let e = movie.check_pressures().expect_err("no pressures is no input");
    assert!(e.message.contains("cross"), "named by control: {e}");
    assert!(PadTape::from_p2m2(&p2m2_file("rycs", &[flat; 2]), replay::TickPhase::First).is_err());
}

/// Five ways a file can look like a recording and not be one, each refused by the byte it is at.
///
/// Four of them move every frame of the file by a whole slot or half a tick, which is the kind of
/// wrong that replays as a plausible session that diverges everywhere.
#[test]
fn a_file_that_is_not_a_recording_is_refused_by_the_byte_it_is_at() {
    let good = p2m2_file("rycs", &[p2m2_entry(0, 0x7f, 0x7f); 6]);
    replay::P2m2::parse(&good).expect("the control");

    let short = replay::P2m2::parse(&good[..200]).expect_err("a header and a half");
    assert!(short.message.contains("header alone is 570"), "{short}");

    let ragged = replay::P2m2::parse(&good[..good.len() - 7]).expect_err("a truncated frame");
    assert!(ragged.message.contains("short of a whole frame"), "{ragged}");

    let mut miscounted = good.clone();
    miscounted[561..565].copy_from_slice(&9u32.to_le_bytes());
    let e = replay::P2m2::parse(&miscounted).expect_err("a count that is not the file's");
    assert!(e.message.contains("7 frame slots for a totalFrames of 9"), "{e}");

    let mut written = good.clone();
    written[replay::P2M2_HEADER_BYTES] = 0xff;
    let e = replay::P2m2::parse(&written).expect_err("a written frame 0");
    assert!(e.message.contains("slot 0 is written"), "{e}");
    assert_eq!(e.at, replay::P2M2_HEADER_BYTES, "and where to look");

    let mut versioned = good.clone();
    versioned[0] = 2;
    let e = replay::P2m2::parse(&versioned).expect_err("a format this reader has not seen");
    assert!(e.message.contains("file version 2"), "{e}");
}

/// **One tick is two of PCSX2's frames**, and which of the two the game read is the one thing the file
/// cannot say — so it is a named choice with both answers available, and a measurement of what it
/// costs to pick the wrong one.
#[test]
fn a_tape_takes_one_field_of_each_tick_and_says_which_one() {
    // Ten fields, each tagged by its own left-stick X, with a button crossing an edge between the two
    // fields of the third tick.
    let fields: Vec<_> = (0..10u8)
        .map(|k| {
            let buttons = if k == 5 { Control::Cross.mask() } else { 0 };
            p2m2_entry(buttons, 0x10 + k, 0x7f)
        })
        .collect();
    let movie = replay::P2m2::parse(&p2m2_file("rycs", &fields)).unwrap();

    let first = movie.tape(replay::TickPhase::First);
    let second = movie.tape(replay::TickPhase::Second);
    assert_eq!(first.len(), 5, "fields 1, 3, 5, 7, 9 of the file");
    assert_eq!(second.len(), 5, "fields 2, 4, 6, 8, 10");
    let x_of = |tape: &PadTape| -> Vec<u8> {
        tape.frames.iter().map(|f| f.reports[0].axis(2)).collect()
    };
    assert_eq!(x_of(&first), vec![0x10, 0x12, 0x14, 0x16, 0x18]);
    assert_eq!(x_of(&second), vec![0x11, 0x13, 0x15, 0x17, 0x19]);
    assert!(first.name.contains("first") && second.name.contains("second"), "the tape says which");
    assert_eq!(first.name, "p2m2-rycs-10f-first");

    // The stick moved in every pair; the press happened between the two fields of one of them, and it
    // is the press that would land a tick early or late.
    assert_eq!(movie.ticks_the_phase_moves(), (5, 1));
    // And it is a real difference, not a bookkeeping one: the port holds cross on a different tick.
    let cross_on = |tape: &PadTape| -> Vec<bool> {
        let mut run = Playback::new(0);
        tape.frames
            .iter()
            .map(|f| {
                run.advance(f);
                run.input().pad(0).held(Control::Cross)
            })
            .collect()
    };
    assert_eq!(cross_on(&first), vec![false, false, false, false, false]);
    assert_eq!(cross_on(&second), vec![false, false, true, false, false]);
}

/// A tape of a recording runs at the 30 fps cap, because that is what the engine handed the frame.
///
/// The `.p2m2` holds no `dt` at all, and the three readings that settle what it was are in
/// `P2m2::tape`'s own documentation: this is the test that the reader actually uses them, since a tape
/// at the wrong step reproduces the buttons and not the millisecond timers the front end's repeat
/// rates are a function of.
#[test]
fn a_tape_of_a_recording_runs_at_the_cap() {
    let movie =
        replay::P2m2::parse(&p2m2_file("rycs", &[p2m2_entry(0, 0x7f, 0x7f); 8])).unwrap();
    let tape = movie.tape(replay::TickPhase::First);
    assert_eq!(tape.dt_range(), Some((replay::CAPPED_DT, replay::CAPPED_DT)));
    assert!(tape.frames.iter().all(|f| f.dt == replay::CAPPED_DT.to_bits()));

    // The timers follow from it: a flat +33 ms a frame, which is what the original's own clock
    // accumulates while the clamp is biting.
    let held = replay::P2m2::parse(&p2m2_file("rycs", &[p2m2_entry(Control::Cross.mask(), 0x7f, 0x7f); 8]))
        .unwrap()
        .tape(replay::TickPhase::First);
    let run = replay::play(&held, 0);
    assert_eq!(run.input().pad(0).control(Control::Cross).held_ms, 99, "three frames at 33 ms");

    // A step of one's own, for a recording the engine did not clamp (the front end runs under the cap).
    let slow = movie.tape_with_dt(replay::TickPhase::First, F(0.05));
    assert_eq!(slow.dt_range(), Some((F(0.05), F(0.05))));
}

/// **A recorded session drives the port**: file bytes in, the port's own input layer and simulation
/// out, with the controls the human held on the ticks they held them.
#[test]
fn a_recorded_session_drives_the_port() {
    // Forty fields of something like play: a button held in runs, the stick wandering, pairs of fields
    // carrying the same input most of the time and not always.
    let script: Vec<u16> = (0..40)
        .map(|k| match k / 6 {
            0 => 0,
            1 => Control::Cross.mask(),
            2 => Control::Cross.mask() | Control::L1.mask(),
            3 => Control::Circle.mask(),
            4 => Control::Up.mask(),
            5 => Control::Start.mask(),
            _ => 0,
        })
        .collect();
    let fields: Vec<_> = script
        .iter()
        .enumerate()
        .map(|(k, held)| p2m2_entry(*held, (0x40 + k) as u8, 0x7f))
        .collect();
    let bytes = p2m2_file("rycs", &fields);

    let tape = PadTape::from_p2m2(&bytes, replay::TickPhase::First).expect("a recording");
    assert_eq!(tape.len(), 20, "one tick per two of PCSX2's frames");

    let mut run = Playback::new(0x1234_5678);
    let mut transitions = 0;
    for (tick, frame) in tape.frames.iter().enumerate() {
        let before = run.input().pad(0).held_mask();
        run.advance(frame);
        assert_eq!(
            run.input().pad(0).held_mask(),
            script[tick * 2],
            "tick {tick} holds what field {} of the file recorded",
            tick * 2 + 1,
        );
        assert_eq!(run.tick() as usize, tick + 1, "and one tick is one frame of the recording");
        transitions += (before != run.input().pad(0).held_mask()) as usize;
    }
    assert_eq!(transitions, 6, "five changes of script plus the first press");
    assert_eq!(run.hash(), replay::play(&tape, 0x1234_5678).hash());

    // The same recording as a fixture: the port's run over a human's input, which is a regression
    // fixture of real play while the state a capture watches is still being ported.
    let f = replay::record(&tape.name, 1, &tape.inputs(), &WATCHED).unwrap();
    assert_eq!(f.hashes.len(), 20);
    replay::verify(&f).expect("the port agrees with its own run of the recorded input");
    assert_eq!(Fixture::from_tape(&tape, 1).inputs, f.inputs, "and the fixture's inputs are the tape's");

    // And through the tape file, so a recording read once need not be read again.
    assert_eq!(PadTape::parse(&tape.to_text()).expect("a tape of a recording"), tape);
}

/// **The human's recording in `context/recordings/`, played through the port.**
///
/// A `.p2m2` is game data — somebody's play of a disc — so it lives in the git-ignored `context/` and
/// this test says what it found and passes when there is nothing there, like the capture tests above.
///
/// What it proves when the file is there: the reader's three decisions hold on a real sixty seconds of
/// free riding (the header against the file's own length, the byte mapping against the file's own
/// redundancy, the tick phase as a counted difference), the tape plays through the port's stateful
/// input layer from end to end, and it survives its file form. What it cannot prove is the simulation
/// side: the tape moves the ported bike since P5, but with no capture of the original beside it there
/// is nothing to compare the result against, so this is the input half of the acceptance run and S8's
/// recording is the other half.
#[test]
fn the_human_recording_in_context_drives_the_port() {
    let dir = private_dir("recordings");
    let mut proved = 0;
    for path in files_in(&dir, "p2m2") {
        let bytes = std::fs::read(&path).expect("a readable recording");
        let movie = replay::P2m2::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(movie.fields(), movie.total_frames as usize);
        let agreements = movie
            .check_pressures()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(agreements, movie.fields() * 12, "twelve controls recorded twice, every frame");

        let tape = PadTape::from_p2m2(&bytes, replay::TickPhase::First)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(tape.len(), movie.ticks(replay::TickPhase::First));
        assert_eq!(
            tape.dt_range(),
            Some((replay::CAPPED_DT, replay::CAPPED_DT)),
            "a recording of play runs at the clamped step",
        );

        let mut run = Playback::new(0);
        let (mut transitions, mut seen) = (0usize, 0u16);
        for frame in &tape.frames {
            let before = run.input().pad(0).held_mask();
            run.advance(frame);
            seen |= run.input().pad(0).held_mask();
            transitions += (before != run.input().pad(0).held_mask()) as usize;
        }
        assert_eq!(run.tick() as usize, tape.len());
        // A recording of a pad nobody touched would pass every assertion above.
        assert!(
            transitions >= 4,
            "{}: only {transitions} ticks where a control changed — not a recording of play",
            path.display(),
        );
        assert_eq!(PadTape::parse(&tape.to_text()).expect("a tape of a recording"), tape);
        assert_eq!(replay::play(&tape, 0).hash(), run.hash(), "and plays back to the same state");

        // The other phase is a different session, and this says by how much: the ticks whose two
        // fields disagree, and the few where the disagreement is a button rather than the stick.
        let (moved, buttons_moved) = movie.ticks_the_phase_moves();
        let other = movie.tape(replay::TickPhase::Second);
        if buttons_moved > 0 {
            assert_ne!(
                replay::play(&other, 0).hash(),
                run.hash(),
                "{}: the two phases should not play to the same state",
                path.display(),
            );
        }
        println!(
            "{}: {} fields by {} on {}, savestate {} → {} ticks, {transitions} transitions, \
             controls {seen:#06x}, {agreements} pressure/bit agreements, phase moves {moved} ticks \
             ({buttons_moved} of them a button)",
            path.display(),
            movie.total_frames,
            movie.author,
            movie.emulator,
            movie.from_savestate,
            tape.len(),
        );
        proved += 1;
    }
    if proved == 0 {
        println!(
            "no PCSX2 recording in {} — make one with PCSX2's own input recording (Tools → Input \
             Recording → New), which writes the `.p2m2` and its `_SaveState.p2s` together",
            dir.display(),
        );
    }
}

// ---------------------------------------------------------------------------------------------------
// The 2p recording: the human reference (C10, A6)
// ---------------------------------------------------------------------------------------------------

/// A name of [`replay::two_p`]'s, in this worktree's `context/`.
fn two_p_file(rel: &str) -> PathBuf {
    private_dir("").join(rel)
}

/// The 2p capture with the columns the reference's own tests read: the counter, `dt`, both pads, and
/// each rider's guards, wreck reason and bike place. A slim read of a ~215 MB file.
fn two_p_capture() -> Option<Capture> {
    let at = two_p_file(replay::two_p::CAPTURE);
    let Ok(file) = std::fs::File::open(&at) else {
        println!("no {} — C10's journal says how it is captured", at.display());
        return None;
    };
    let lane = ["type", "index", "ctl_src", "bike_type", "bike_player", "wreck_reason", "bike_xform.m30", "bike_xform.m31", "bike_xform.m32"];
    let keep = |c: &str| {
        ["frame_counter", "dt"].contains(&c)
            || c.starts_with("pad")
            || c.starts_with("stamp")
            || ["p1_", "p2_"].iter().any(|p| c.strip_prefix(p).is_some_and(|rest| lane.contains(&rest)))
    };
    let cap = replay::two_p::read(std::io::BufReader::new(file), keep).unwrap_or_else(|e| panic!("{}: {e}", at.display()));
    Some(cap)
}

/// The 2p state's two riders down the game manager's actor list (`watch/2p.toml`): entry 3 is P1's
/// ridden vehicle, entry 2 P2's. Research addresses, in a test that reads a RAM image.
const TWO_P_RIDERS: [&[u32]; 2] = [&[0x34, 0x34, 0x8, 0x8, 0x8, 0x0], &[0x34, 0x34, 0x8, 0x8, 0x0]];

/// The 2p riders' bike, as the state's loader read it (C10's journal: both load `Race500P.phy`).
const TWO_P_PHY: &str = "race500p.phy";

/// **The 2p recording, the human reference: both riders' tapes drive the port from the 2p state.**
///
/// The tape is read the one way this ride needs (a field a tick, [`replay::P2m2::tape_per_field`]) and
/// held against the capture's own two pads ([`replay::two_p::tape`]) field for field; each rider's lane
/// reads as a one-rider capture whose tape is that rider's pad alone ([`replay::two_p::lane`],
/// [`PadTape::alone`]); and the port, seeded from the tape's own state (both bikes' whole ride off
/// `2p-start.ram`) on the disc's Race 500 over free09, rides the whole tape with two players, each
/// holding on every tick what its own pad held.
///
/// What it does not prove: that the port rides where the original did. The positions are printed
/// beside the capture's, not held to them — the wreck, ragdoll and collision tasks (S155..S160) are
/// where the 2p ride's lanes become verdicts. Skips with a line when the tape, capture, state or disc
/// is not here.
#[test]
fn the_2p_recording_drives_both_riders_from_its_state() {
    use replay::two_p::{self, Rider as Seat};
    let at = two_p_file(two_p::TAPE);
    let Ok(bytes) = std::fs::read(&at) else {
        println!("no {} — the user's 2p recording (C10)", at.display());
        return;
    };
    let movie = replay::P2m2::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", at.display()));
    assert!(movie.from_savestate, "the tape starts from its state");
    assert_eq!(movie.fields(), two_p::ROWS, "a field a tick, as many as the capture's rows");
    assert_eq!(movie.check_pressures().expect("the file agrees with itself"), two_p::ROWS * 2 * 12, "both pads, every field");
    let fields = movie.tape_per_field(F(1.0 / 60.0));
    assert!(fields.frames.iter().all(|f| f.reports.iter().all(|r| r.status() == 0)), "both pads written on every field");

    let Some(cap) = two_p_capture() else { return };
    assert_eq!(cap.rows.len(), two_p::ROWS);
    assert_eq!(cap.frames.len(), two_p::ROWS);
    let tape = two_p::tape(&cap).expect("both pads off the capture");
    assert_eq!(tape.len(), two_p::ROWS - 1, "row 0 is the tick before");
    // The capture's dt is the engine's own sixtieth, never the 30 Hz clamp.
    let (least, most) = tape.dt_range().unwrap();
    assert!(least > F(0.0166) && most < F(0.0168), "dt {least:?}..{most:?}");

    // The tape's fields are the capture's polled pads: row r's polled pad (frames[r - 1]) is field
    // r + d for one small d, the same for both riders.
    let mut best = None;
    for d in -3i64..=3 {
        let agree = |pad: usize| {
            (0..tape.len())
                .filter(|&k| {
                    let f = k as i64 + 1 + d;
                    f >= 0 && (f as usize) < fields.len() && {
                        let (a, b) = (&tape.frames[k], &fields.frames[f as usize]);
                        a.held_mask(pad) == b.held_mask(pad) && a.stick(pad) == b.stick(pad)
                    }
                })
                .count()
        };
        let (p1, p2) = (agree(0), agree(1));
        println!("2p: capture pads against tape fields at offset {d:+}: P1 {p1}, P2 {p2} of {}", tape.len());
        if best.is_none_or(|(_, n)| p1 + p2 > n) {
            best = Some((d, p1 + p2));
        }
    }
    let (d, n) = best.unwrap();
    println!("2p: the capture's polled pads are the tape's fields at offset {d:+} ({n} of {} agree)", 2 * tape.len());
    assert!(n * 100 >= 2 * tape.len() * 95, "the tape and the capture are not the same pads: best {n} at {d:+}");
    assert_eq!(d, -(two_p::POLLED_FIELD_LAG as i64), "row r polls field r - {}", two_p::POLLED_FIELD_LAG);

    // Either rider's lane is a one-rider capture, and its tape is that rider's pad alone.
    for seat in Seat::BOTH {
        let lane = two_p::lane(&cap, seat).expect("a lane");
        let col = |name: &str| column(&lane, name).unwrap();
        for r in 0..lane.rows.len() {
            assert_eq!(cell(&lane, r, col("type")), "2", "{seat:?} row {r}: the ridden vehicle");
            assert_eq!(cell(&lane, r, col("index")), seat.pad().to_string(), "{seat:?} row {r}: the player");
            assert_eq!(cell(&lane, r, col("ctl_src")), "1", "{seat:?} row {r}: a local human");
            assert_eq!(cell(&lane, r, col("bike_player")), seat.pad().to_string(), "{seat:?} row {r}: the bike's player");
        }
        let alone = two_p::tape_of(&cap, seat).expect("a lane's tape");
        assert_eq!(alone.frames, tape.alone(seat.pad()).frames, "{seat:?}'s lane tape is its pad of the two");
        assert!(alone.frames.iter().all(|f| f.reports[1] == Report::missing()));
    }

    // The port from the 2p state.
    let ram = two_p_file(two_p::START_RAM);
    let states: Vec<SaveState> = TWO_P_RIDERS.iter().filter_map(|hops| SaveState::open_image(&ram, hops)).collect();
    let [p1, p2] = &states[..] else {
        println!("no {} with both riders in it", ram.display());
        return;
    };
    assert_eq!(u64::from(p1.word(SaveState::FRAME_COUNTER).unwrap()) + 1, two_p::FIRST_FRAME, "the state is the tick before row 0");
    let p1_lane = two_p::lane(&cap, Seat::P1).unwrap();
    let ran = in_the_world_on(&p1_lane, TWO_P_PHY, "500", |world, named, bike| {
        let Some(Ridden { tables, phy, .. }) = bike else {
            println!("no {TWO_P_PHY} on this disc");
            return None;
        };
        let col = Disc::open().and_then(|mut disc| the_bike_col(&mut disc)).map(Arc::new);
        println!("2p: riding {named} on {TWO_P_PHY}, body rays {}", if col.is_some() { "on" } else { "off (not on this disc)" });
        let rider_of = |st: &SaveState| {
            let [front, rear] = st.wheels().expect("the state's wheel mounts").map(|q| Vf(q.map(f32::from_bits)));
            let mut r = Rider::new(&phy, (front, rear), Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
            if let Some(c) = &col {
                r.carries_bike_col(Arc::clone(c));
            }
            r
        };
        let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider_of(p1));
        p1.seed_ride(&mut sim.rider.ride).expect("P1's whole bike");
        sim.add_player(phy.clone(), rider_of(p2));
        p2.seed_ride(&mut sim.others[0].rider.ride).expect("P2's whole bike");
        // A Sim plugs pad 0 alone; the 2p state has both in (the capture polls both, every row).
        sim.input.pad_mut(1).connected = true;
        let start: Vec<Vf> = (0..2).map(|i| sim.rider_of(i).ride.chassis.cofg_world).collect();

        let place = |r: usize, p: &str| -> [f32; 3] {
            ["m30", "m31", "m32"].map(|m| cell(&cap, r, column(&cap, &format!("{p}bike_xform.{m}")).unwrap()).parse::<f32>().unwrap())
        };
        for (k, frame) in tape.frames.iter().enumerate() {
            let controls = sim.poll_all(frame.dt(), &frame.reports);
            sim.step_all(frame.dt(), &controls, world);
            for seat in Seat::BOTH {
                assert_eq!(
                    sim.input.pad(seat.pad()).held_mask(),
                    frame.held_mask(seat.pad()),
                    "tick {}: {seat:?} holds what its pad held",
                    k + 1
                );
            }
            if [1, 30, 300, 3000, tape.len() - 1].contains(&k) {
                for seat in Seat::BOTH {
                    let c = sim.rider_of(seat.pad()).ride.chassis.cofg_world;
                    println!(
                        "2p tick {}: {seat:?} port centre {:?}, original's bike {:?}",
                        k + 1,
                        [c.x(), c.y(), c.z()],
                        place(k + 1, seat.prefix())
                    );
                }
            }
        }
        let moved: Vec<f32> = (0..2)
            .map(|i| {
                let (a, b) = (start[i], sim.rider_of(i).ride.chassis.cofg_world);
                ((a.x() - b.x()).powi(2) + (a.z() - b.z()).powi(2)).sqrt()
            })
            .collect();
        Some((sim.ticks, moved))
    });
    let Some(Some((ticks, moved))) = ran else { return };
    assert_eq!(ticks as usize, tape.len(), "the whole tape, a tick a field");
    for (i, m) in moved.iter().enumerate() {
        assert!(*m > 10.0, "player {} rode {m} ft from the state: the pad never moved the bike", i + 1);
    }
    println!("2p: both riders rode the whole tape from the state ({moved:?} ft from where they started)");
}

// ===================================================================================================
// S8 — the acceptance run, against the human's own recording
// ===================================================================================================
//
// Everything above is machinery and synthetic rows. This section is the one place in the file where
// the **original** speaks: `context/capture/acceptance-freeride.csv`, 1,811 ticks of
// `watch/acceptance.toml` recorded off the very save state `context/recordings/first_recording.p2m2`
// was made from, with the recording itself beside it in `context/recordings/`.
//
// What that capture settled:
//
//   * **The step.** `dt` is the 30 fps clamp ([`replay::CAPPED_DT`], 0.033330001) on all 1,811 ticks
//     of a run *in a level*. I4 argued this from one frame of a save state plus the session's 975 ms
//     clock deficit and left it as the thing S8 had to measure; it is measured now, frame by frame, so
//     a fixed-step port can replay this minute at all.
//   * **One row per tick.** The game's own `frame_counter` rises by exactly 1 — and the recorder's
//     vsync `frame` by exactly 2 — on every one of the 1,811 rows, with no torn read. A capture this
//     dense is a fixture; a capture with one gap in it is not, and [`ticks_of`] says so.
//   * **Which actor is the human's.** The five-hop chain `watch/acceptance.toml` inherited reads an
//     *AI rider*: a freeride area on Free09 holds four type-4 vehicles, and `bike_type == 4` with a
//     non-null `bike_player` is true of all four. The human's is the eighth list entry, and the
//     column that proves it is the player record's control source (`+0x20 -> +0x34`), 1 for a local
//     human against 2 for an AI. [`acceptance_fixture`] refuses a capture whose rows are not 1.
//   * **That the recording's bytes really are the bytes the original reads.** `report_of_p2m2` and the
//     capture's `pad_a`/`pad_b` columns are two entirely separate paths — a PCSX2 file on disk, and
//     PINE reads of libpad's buffer in EE RAM — and over the ticks where both runs had the pad at rest
//     they produce byte-identical [`PadFrame`]s, `dt` and absent second pad included.
//
// What it did not settle then, and what has settled it since:
//
//   * **The original driven by the recording's own input.** PCSX2 plays a `.p2m2` back from its GUI
//     only — no CLI flag, no ini key, no hotkey — and no keystroke this desktop can synthesise reaches
//     its window. What does reach it is the desktop's accessibility bus: the menu item Tools → Input
//     Recording → Play can be pressed through it and the file dialog it opens filled the same way. So
//     `acceptance-freeride.csv` is now the human's minute driven by the human's input, sampled tick by
//     tick from the recording's own start state: 1,811 rows, 1,449 of them with a control held, the
//     bike to 77 units/s over 3,071 units of ground, and two crashes in it.
//
//     That also settled the tick phase I4 left open, which an undriven capture never could: every
//     field before the first press is the same neutral pad, so both phases agree about the prefix and
//     only a driven run can disagree past it. `TickPhase::Second` matches RAM on all 1,810 overlapping
//     ticks and `First` loses it at the press — the game reads the **second** field of each tick
//     (`the_capture_is_the_recording_played_back_and_the_game_reads_the_second_field`).
//
//     The scripted alternative is still there and still useful, kept under `context/capture/s8/`:
//     thirty seconds from the same save state with the kit's virtual pad riding it, which any agent
//     can remake with no human and no GUI (`./kit pad script` beside `./kit record`). What it cannot
//     do is crash, scrape a slope or land sideways on purpose.
//   * **The bike is now the thing the harness drives** (P5): `Sim` is the ride, the rider and the
//     world the shell builds, and its walk names the capture's eight lanes, so the verdict of the real
//     fixture is a tick and a field rather than `NoSuchField bike.pos[0]` at tick 0.
//   * **The run starts where the recording starts** (S8). P5's first real verdict was `tick 0:
//     bike.pos[0] 4309.563 vs 0` — the capture is the game's ticks 2774..4584 of a session already
//     minutes old, and the fixture fell back to `Sim::new`'s start line, so the comparison was two
//     spawns and not a simulation. [`acceptance_start`] seeds the state from the capture's own first
//     row: the bike's whole world transform and the body's velocity, bit for bit. Tick 0 agrees on all
//     eight lanes now, and `bike.pos[3]` and `bike.vel[3]` — the quads' `w` lanes, 1.0 in the original
//     — agree on all 1,810 recorded ticks, through both crashes.
//   * **And the ground is under it** (S28): a fixture can name the track and the area it was recorded
//     on ([`Where`]) and [`replay::verify_in`] replays it over any [`World`], so the acceptance run is
//     a minute of Free09 on the disc's own Race 125 rather than a minute of nothing at y = 0. Nothing
//     in the capture says which area it was, so it is read off the ride itself
//     ([`recorded_on`]): of the disc's five freeride areas, exactly one keeps the recorded bike above
//     its dirt all the way along and lets it touch down.
//   * **And the gate reports rather than panics, down to a recorded tick**
//     ([`ACCEPTANCE_AGREES_THROUGH`]), which is the decision P5 left to S8. The run still parts from
//     the original at tick 1 — over the right ground it is the bike's own ride height that parts now,
//     and the constant's own note has the measurement.
//     That is the status of the port, not a regression, and a gate that panics on it reddens every
//     unrelated task's `./kit check` in any checkout holding the data — which is how the captures came
//     to be parked out of the readers' way to begin with. So the agreement is a constant the run must
//     reach, getting further asks for it to be raised, and falling short of it fails.

/// The `frame_counter` (app+0x374) in `first_recording.p2m2`'s own save state, read with
/// `./kit state extract` — the tick the recording starts from.
///
/// This is what aligns the recording against a capture taken from the same state: a capture row whose
/// counter is *c* was produced by the tick the recording's frame `c - 2773 - 1` drove. The recorder's
/// first row lands a few ticks in (it loads the state, waits for its warmup and samples after a tick),
/// so the first handful of the recording's ticks can be missing from a capture. On the capture in hand
/// the load was caught with a tick to spare, so its first row is the recording's frame 0 and the
/// agreement window below starts at 0; a slower load would simply start it further in.
const RECORDING_START_TICK: u64 = 2_773;

/// The recording whose save state the acceptance capture was taken from. Only this file can be
/// compared against that capture tick by tick; another recording is another session.
const RECORDING: &str = "first_recording.p2m2";

/// How many ticks that recording is, and so how long a capture of its minute runs: 3,621 video fields
/// at two fields a tick.
const RECORDING_TICKS: usize = 1_811;

/// The first video field of the recording holding anything at all — measured, 1-based as PCSX2 numbers
/// its frames, and asserted below rather than trusted.
///
/// Fields 1..=55 are a written, neutral pad: button word `0xffff` (active low, so nothing down), both
/// sticks centred, every pressure 0. The human's first press is a button, on field 56. Under
/// `TickPhase::First` (fields 1, 3, 5…) the game never reads field 56, so the first tick that carries
/// input is field 57's — tick 28 — and under `Second` it is field 56's, tick 27.
const FIRST_PLAYED_FIELD: usize = 56;

/// How to make the acceptance capture, as a sentence a test can print. Kept in one place because two
/// tests need it and because it is the actual recipe that produced the capture in hand — the recording
/// supplies its own start state, which is what makes the run repeatable.
fn acceptance_recipe(capture: &PathBuf) -> String {
    format!(
        "no acceptance capture on this machine. To make one:\n\
         \x20 1. cp context/recordings/{RECORDING}_SaveState.p2s \\\n\
         \x20      \"<PCSX2 copy>/savestates/SLUS-20767 (9866FE54).08.p2s\"\n\
         \x20 2. ./kit pcsx2 start && ./kit pine\n\
         \x20 3. ./kit record watch/acceptance.toml --state 8 -n {RECORDING_TICKS} -o {}\n\
         then run this test again. It needs no code.",
        capture.display(),
    )
}

/// Where an acceptance capture lives, in the order the readers look.
///
/// Two places, because both are in use and neither is wrong:
///
/// - **`context/capture/acceptance/`** is the home of the captures of this recording, and the one
///   whoever records more of them should use. `context/capture/` is scanned by half a dozen tests that
///   each pick up any CSV carrying their columns — the input decode, the pad tape, the whole-frame
///   lag tests — so a capture sitting loose in it is read by every one of them, and an acceptance
///   capture is the one kind whose verdict is *the status of the port* rather than a pass or a fail.
///   Keeping the set in a subfolder is what lets `./kit check` be green about unrelated work in a
///   checkout that has the data (V16 moved them there for exactly that reason, and the main checkout
///   keeps them there).
/// - **`context/capture/`** is still read, because that is where `./kit record -o` puts a file by
///   default and a capture dropped straight out of the recorder should be found, not skipped.
///
/// S8 decided this rather than moving the file: the reader looks in both, the recipe below names the
/// subfolder, and nothing in `context/` is moved by a test.
fn acceptance_dirs() -> [PathBuf; 2] {
    [private_dir("capture").join("acceptance"), private_dir("capture")]
}

/// The acceptance capture, from the first of [`acceptance_dirs`] that holds it — or `None`, with the
/// recipe printed.
///
/// Game data, so git-ignored and absent on every machine but the one that recorded it, and absent in
/// every worktree but the one it was recorded in (`context/` is per-worktree). A test cannot fail over
/// a file the repository is not allowed to hold.
fn acceptance_capture() -> Option<(PathBuf, Capture)> {
    let Some((path, text)) = acceptance_capture_text() else {
        println!("{}", acceptance_recipe(&acceptance_dirs()[0].join(format!("{ACCEPTANCE}.csv"))));
        return None;
    };
    let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Some((path, cap))
}

/// The acceptance capture's path and bytes, wherever it is.
fn acceptance_capture_text() -> Option<(PathBuf, String)> {
    for dir in acceptance_dirs() {
        let path = dir.join(format!("{ACCEPTANCE}.csv"));
        if let Ok(text) = std::fs::read_to_string(&path) {
            return Some((path, text));
        }
    }
    None
}

/// **The last tick the port is known to agree with the original on**, and so the gate the acceptance
/// run is: it must still agree through this tick, and a divergence at or before it is a regression and
/// a failure. Raising this number is the progress every task after S8 makes.
///
/// It is 0, and 0 is not nothing: tick 0 is the state the run starts from, and the port agreeing with
/// the original there on all eight lanes of [`BIKE`] — the bike's whole world transform and the body's
/// velocity, bit for bit — is what [`acceptance_start`] bought. It was `NoSuchField` before P5 and a
/// 4,309-unit teleport before S8.
///
/// Tick 1 is where it stops, and the reason has moved once already. Over a flat floor — all a fixture
/// could carry through S8 — the port's bike was in free fall: `bike.pos[1]` 207.98192 against the
/// original's 208.02884, losing 0.0475, 0.0809, 0.1165, 0.1519… units a tick, second differences of a
/// flat 0.0353, which at the 30 Hz step is an acceleration of 31.8 units/s², i.e.
/// [`game_sim::phy::G`] and nothing else; the original's bike lost 0.00058 a tick down a gentle slope.
///
/// S28 put the ground under it — the capture is made to name the area it was recorded on and the
/// fixture to carry it ([`Where`], [`in_the_world_of`]), and the bike is the Race 125 the human rode,
/// read off the disc instead of a stand-in 250 (D7) — and the run stopped falling. `bike.pos[0]` holds
/// to tick 29 now, and to 32 on the shorter `acceptance-ride` capture, where every lane used to part at
/// 1: that is what the world bought.
///
/// S8 settled S28's question — the mounts were wrong, not the ground: the original hangs its wheels off
/// the bike model's own tire nodes, 1.48 units higher than the `.phy`'s, and the port now does too, bit
/// for bit ([`the_wheels_the_human_rode`]), and seeding the drivetrain from the save state
/// ([`SaveState::drivetrain`]) took the clutch's spurious launch out of tick 1 as well. Tick 1's
/// `bike.pos[1]` went from 208.10735 to 208.0297, against the original's 208.02884.
///
/// **What holds it at 0 now is the capture itself**: its eight lanes are not the chassis. `bike_xform`
/// is the transform of the bike as it is *drawn* — the model's root, posed off the chassis by about a
/// degree and a half of roll and 0.065 units sideways on a bike resting on the rider's foot — and
/// `bike_vel` is the chassis' velocity times 0.8
/// ([`the_acceptance_capture_watches_the_drawn_bike_not_the_chassis`]). The port has a chassis and no
/// drawn root, so it starts its chassis where the drawn root was and compares the chassis against it;
/// no lane can agree past tick 0 that way except by not moving. `bike.pos[0]`'s 27-odd ticks are
/// exactly that: x creeping by less than an ulp of 4309.
///
/// Measured against the chassis' own state instead (`watch/s8-start.toml`, which records it beside the
/// drawn root), and started from the save state's chassis, the port still parts at its first step:
/// the front wheel finds ground 0.036 higher than the original does, because
/// [`game_sim::world::Ground::at`] blends the cell bilinearly where the original's height is the
/// cell's triangle split along its (+x)-(+z) diagonal (208.45439 against the original's 208.45465 under
/// the front tire; bilinear gives 208.4905). S8's journal entry has the numbers. In the order they will
/// bite after that: the ground's in-cell arithmetic (D6), a fixture lane for the chassis rather than
/// the drawn bike (a re-recording, or the drawn root's pose ported), the wheels' spin and compression
/// and the trick state at row 0, which nothing in the capture carries; and S3's movement and S10's air
/// are not yet 1:1.
///
/// S64 took the ground's arithmetic and the wheels off that list: [`game_sim::world::World::ground`]
/// is the triangle split now, and started from the save state's whole bike (both wheels and the air
/// state too) the chassis' run agrees with the original's on every lane at tick 0 and parts at tick 1
/// only by ulps — and on its position not until tick 23
/// ([`the_save_state_run_starts_from_the_originals_wheels`]). What holds this one at 0 is the drawn
/// bike alone.
const ACCEPTANCE_AGREES_THROUGH: u64 = 0;

/// What a replay of the real acceptance fixture means today, in one line, and what makes it fail.
///
/// **This reports rather than fails, down to [`ACCEPTANCE_AGREES_THROUGH`].** P5 left the choice to
/// S8 and this is it, for one reason: a divergence here is the *status of the port*, not a regression
/// in whoever ran the tests. The acceptance run is the only test in the project whose red means "the
/// port is not finished yet", and the port is not finished yet — the world is not in a fixture, the
/// bike is a stand-in and two movement tasks are open. A gate that panics on that reddens `./kit
/// check` for every unrelated task in any checkout that has the data, which is how the data came to
/// be parked out of the readers' way in the first place ([`acceptance_dirs`]); and a gate that is
/// merely silent proves nothing. So it is a ratchet: the agreement so far is a constant, the run has
/// to reach it, and getting further prints a line asking for the constant to be raised.
///
/// Four outcomes:
///
/// - `Ok` — the port agreed with every recorded tick. That is the acceptance run passing, and the
///   point of the whole file.
/// - a divergence **after** the agreed tick — printed, with how far the run got and the first tick
///   each lane parts company on, which is where to start reading. Not a failure.
/// - a divergence **at or before** the agreed tick — panics. The port has gone backwards, or the
///   fixture has stopped carrying its start state.
/// - `NoSuchField` — the fixture watches a field the port's state does not have. Since P5 the eight
///   bike lanes of [`BIKE`] are all fields of the state the harness drives, so this means a fixture
///   file older than the walk, or a capture carrying a column the reader aims somewhere the walk does
///   not go. Printed: the port is not what is wrong in that case.
fn acceptance_verdict(f: &Fixture, world: &mut dyn World) {
    acceptance_verdict_against(f, world, ACCEPTANCE_AGREES_THROUGH, "ACCEPTANCE_AGREES_THROUGH");
}

/// [`acceptance_verdict`] against a ratchet of the caller's: `agreed` is the constant, `gate` its
/// name for the line that asks for it to be raised. S32's second run is the other caller.
fn acceptance_verdict_against(f: &Fixture, world: &mut dyn World, agreed: u64, gate: &str) {
    let d = match replay::verify_in(f, world) {
        Ok(end) => {
            println!(
                "the port agrees with every recorded tick of {}: {} ticks, {} checks",
                f.name,
                end.ticks,
                f.checks(),
            );
            return;
        }
        Err(d) => d,
    };
    if let Reason::NoSuchField { path } = &d.reason {
        println!(
            "the state has no '{path}': the capture names a field this walk does not, which is the \
             fixture's own fault and not a divergence — a fixture file recorded against an older \
             walk, or a column aimed at a path nothing renames. {d}",
        );
        return;
    }
    assert!(
        d.tick > agreed,
        "the acceptance run has gone backwards: it agreed through tick {agreed} ({gate}) and now \
         diverges at {d}. If this is tick 0 and the run came \
         from a `.fixture` file, that is the text format rather than the port: it cannot carry a \
         start state yet (`Fixture::start`), so an acceptance fixture written out with `to_text` \
         replays from the spawn — replay the capture instead",
    );
    println!(
        "the acceptance run of {} reaches tick {} of {} before it parts from the original: {d}",
        f.name,
        d.tick,
        f.last_tick(),
    );
    for (path, tick) in first_divergence_per_lane(f, world) {
        match tick {
            Some(tick) => println!("  {path}: agrees through tick {}, parts at {tick}", tick - 1),
            None => println!("  {path}: agrees on every recorded tick"),
        }
    }
    if d.tick > agreed + 1 {
        println!(
            "  — further than {gate} ({agreed}) says: raise it to {} and the gate holds the new \
             ground",
            d.tick - 1,
        );
    }
}

/// Which lane parts company with the original first, and on which tick — A1's field-level diff, over
/// the one fixture that compares the port against the game.
///
/// One replay per lane, each a copy of the fixture watching that lane alone, because `verify` stops at
/// the first disagreement of any kind: the run's own first divergence says *a* lane and this says all
/// eight. A lane that never disagrees is `None`, which on a divergent run is the interesting answer —
/// it means the lane the whole-fixture verdict named is the only one that moved.
fn first_divergence_per_lane(f: &Fixture, world: &mut dyn World) -> Vec<(String, Option<u64>)> {
    f.watches
        .iter()
        .map(|w| {
            let one = Fixture { watches: vec![w.clone()], hashes: Vec::new(), ..f.clone() };
            (w.path.clone(), replay::verify_in(&one, world).err().map(|d| d.tick))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------
// S28 — the ground the recording was ridden over, and the bike it was ridden on
// ---------------------------------------------------------------------------------------------------
//
// A fixture is the rider's half of a session; the other half is the world under it, and until S28 the
// acceptance run was replayed over `world::Flat`'s floor at y = 0 while the recording is a minute over
// Free09 at a ground height of about 208. That is why it parted from the original at tick 1 with the
// port's bike free-falling at a flat 31.8 units/s² (`phy::G`) where the original's lost 0.00058 a tick
// down a slope: no wheel could reach the ground, so nothing in the ride's ground step ever ran.
//
// What this section adds is the seam and the two things that go through it:
//
//   * **the world**, borrowed out of the user's own disc — the area's terrain for the suspension's ray
//     ([`game_sim::world::Ground`], D6) and its collision meshes for the sweep ([`Props`], S4) — handed
//     to [`replay::verify_in`] by the caller, because a track is not a value a fixture can carry;
//   * **the bike**, `Bike\Race125P.phy` off the same disc rather than [`game_sim::stock`]'s stand-in
//     250 (D7 settled which file a freeride boot loads, and `tests/phy.rs` proves the port's read of it
//     is the bike the human's own save state was carrying, word for word).
//
// And **which** world, which the capture has to say or the seam is a guess. `watch/acceptance.toml`
// records no track name — the chain that would read one was not in the spec when the human's minute was
// played back, and re-recording it needs the GUI again — so the area is identified from the capture
// itself, which needs no emulator and works for any future capture of any area: of the disc's five
// freeride areas, exactly one has ground the recorded bike is never *under* and does come down *on*
// ([`recorded_on`]). The others bury it by 18 to 149 units on hundreds of rows, and the one that does
// not is Free09, whose start line is 3.7 units from the capture's own first row.

/// The freeride areas on the disc, as `MX.PSR` spells their containers. Every one of them is a
/// candidate for a capture of freeride play until the capture's own bike says otherwise
/// ([`recorded_on`]); the `_ss` containers beside them are the same tracks' supercross stadiums, which
/// a freeride session is not in.
const FREERIDE_TRACKS: [&str; 5] = [
    "Tracks\\free06.res",
    "Tracks\\free07.res",
    "Tracks\\free09.res",
    "Tracks\\free10.res",
    "Tracks\\free11.res",
];

/// How far under an area's ground the recorded bike may be before that area is not the one it was
/// ridden on, in world units.
///
/// The same margin both ways, because it is the same quantity: no single row where the bike is above an
/// area's ground says anything — a minute of freeride is half air — so an area is refused for the rows
/// that bury the bike and for coming no closer than this to it on any row at all. The margin is for the
/// two ways a point on a bike is not a point on the terrain: the recorded lane
/// is the chassis transform's origin rather than a contact patch, and it sits about half a unit below
/// [`Ground::at`]'s surface while the bike is standing still on it (208.0294 against 208.5231 on the
/// capture's first row — and that blend is this port's own arithmetic, not the game's). Two units
/// leaves that alone and still refuses the wrong area by a factor of ten.
const BURIED: F = F(2.0);

/// How rarely [`recorded_on`] lets a row be [`BURIED`] in the area it was ridden on: one in this many.
/// A suspension bottomed out on a hard landing puts the body origin under the ground for a tick (S32).
const BURIED_ONE_IN: usize = 100;

/// The bike a freeride boot loads, as D7 proved the loader asks for it. Named here as the archives
/// spell their members, which is lowercased and without the `Bike\` the loader prepends.
const FREERIDE_PHY: &str = "race125p.phy";

/// The user's own disc, open: `MX.PSR`'s directory and a reader for one member at a time.
///
/// The same two steps `tests/track.rs` and `tests/phy.rs` each take — the front of the file is the
/// directory, and a member is a seek and a read — kept here rather than shared because a test reading
/// the disc should be readable on its own, and because this one has to decide *which* member to read.
struct Disc {
    file: std::fs::File,
    psr: Psr,
}

impl Disc {
    /// The disc, or `None` with the command that produces it: game data, so absent in a fresh
    /// checkout and in every worktree that has not extracted it.
    fn open() -> Option<Disc> {
        let at = private_dir("iso").join("MX.PSR");
        let mut file = match std::fs::File::open(&at) {
            Ok(f) => f,
            Err(_) => {
                println!(
                    "no {} — `./kit extract MX.PSR` writes it; the acceptance run needs the disc \
                     for the ground the recording was ridden over and for the bike it was ridden on",
                    at.display(),
                );
                return None;
            }
        };
        let mut front = vec![0u8; 64 << 10];
        std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
        let psr = Psr::parse(&front).expect("MX.PSR's directory");
        Some(Disc { file, psr })
    }

    /// One member's bytes, by the name the directory spells.
    fn member(&mut self, name: &str) -> Option<Vec<u8>> {
        let e = self.psr.find(name)?;
        std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut self.file, &mut bytes).expect("read");
        Some(bytes)
    }

    /// Every `Bike\*.res` on the disc, which is where the physics files live (D7: a track's own second
    /// archive carries the 250 pair, and the pairs ship inside the bike *models*).
    fn bike_archives(&self) -> Vec<String> {
        self.psr
            .entries
            .iter()
            .map(|e| e.name.clone())
            .filter(|n| {
                let n = n.to_ascii_lowercase();
                n.starts_with("bike\\") && n.ends_with(".res")
            })
            .collect()
    }
}

/// A member by the name whoever asks for it spells, which is a leaf where the archive's own is a path
/// and either case: the lookup the app layer does, folded the same way.
fn by_suffix<'a>(res: &'a Res, want: &str) -> Option<&'a game_sim::archive::Entry> {
    let want = want.to_ascii_lowercase();
    res.entries.iter().find(|e| {
        let name = e.name.to_ascii_lowercase();
        name == want || name.ends_with(&format!("\\{want}"))
    })
}

/// One area of one track, with the bytes its world is borrowed out of.
///
/// The terrain stays as bytes because [`Terrain`] borrows them and [`Ground`] borrows the `Terrain`:
/// the two have to be locals at the call site, in that order, so that both outlive the [`Place`] the
/// replay rides.
struct Area {
    named: Where,
    scene: Scene,
    terrain: Vec<u8>,
}

impl Area {
    /// Every area of one track container: one per scene, which on each of the disc's freeride tracks
    /// is exactly one. An area whose terrain member is missing is skipped — there is no world to ride.
    fn all_of(disc: &mut Disc, track: &str) -> Vec<Area> {
        let Some(bytes) = disc.member(track) else { return Vec::new() };
        let Ok(res) = Res::parse(&bytes) else { return Vec::new() };
        let scenes: Vec<String> = res
            .entries
            .iter()
            .map(|e| e.name.clone())
            .filter(|n| n.to_ascii_lowercase().ends_with(".scn"))
            .collect();
        let mut out = Vec::new();
        for name in scenes {
            let entry = res.entries.iter().find(|e| e.name == name).expect("the scene listed");
            let Ok(scene_bytes) = res.read(&bytes, entry) else { continue };
            let Ok(scene) = Scene::parse(&scene_bytes) else { continue };
            // The scene names its terrain `Free09.trn` and the disc stores the platform's own copy of
            // it beside that name, which is what the shell loads too.
            let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
            let Some(te) = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(&leaf))
            else {
                continue;
            };
            let Ok(terrain) = res.read(&bytes, te) else { continue };
            out.push(Area {
                named: Where { track: track.to_string(), area: name.clone() },
                scene,
                terrain,
            });
        }
        out
    }

    /// Everything solid the scene stands on this area's ground, read out of the track's own two
    /// archives the way the shell reads them: the second one carries the vehicles' collision.
    ///
    /// A mesh the archives do not have is counted and skipped, because a track is rideable without one
    /// of its fences and a world that refuses to load is worse than a world missing a sign post.
    fn props(&self, disc: &mut Disc) -> (Props, Vec<String>) {
        let mut parts = Vec::new();
        for name in [self.named.track.clone(), self.named.track.replace(".res", "_2.res")] {
            if let Some(bytes) = disc.member(&name) {
                if let Ok(res) = Res::parse(&bytes) {
                    parts.push((bytes, res));
                }
            }
        }
        Props::load(&self.scene, |want| {
            parts
                .iter()
                .find_map(|(bytes, res)| {
                    by_suffix(res, want).and_then(|e| res.read(bytes, e).ok())
                })
                .ok_or_else(|| "no archive of this track has it".to_string())
        })
    }
}

/// **Which area a capture of freeride play was recorded on, decided from the capture itself.**
///
/// The capture carries the bike's world position on every row and the disc carries five freeride
/// areas' heightfields, and a ride stays *above* its ground and comes down on it: an area that puts
/// the recorded bike metres underneath its dirt is not the area the recording was made on, and nor is
/// an area whose dirt the bike never comes within a wheel of. Both halves are needed — a bike in the
/// air is a bike doing what a freeride recording is for, so no single row decides anything, and a ride
/// that is in the air over an area from end to end is a ride somewhere else.
///
/// On the acceptance capture the first half is decisive on its own: Free09 never buries the bike by
/// more than 1.53 units and buries it by more than one on 8 of 1,811 rows, where the other four areas
/// bury it by 18, 77, 84 and 149 units on 206 to 1,160 rows each. On the shorter `acceptance-ride`
/// capture it is not — that ride also stays over ground Free11 has, and Free11 has it 60 units below
/// the wheels the whole way, which is what the second half throws out. A third signal agrees with both
/// and is not used: Free09's own start line is 3.7 units from the acceptance capture's first row, where
/// the nearest of the others is 320.
///
/// Returns the area and the lines of the measurement, for printing next to the verdict.
fn recorded_on(cap: &Capture, disc: &mut Disc) -> Option<(Area, Vec<String>)> {
    let rows = bike_rows(cap)?;
    let mut candidates = Vec::new();
    let mut lines = Vec::new();
    for track in FREERIDE_TRACKS {
        for area in Area::all_of(disc, track) {
            let terrain = match Terrain::parse(&area.terrain) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let ground = Ground::new(&terrain, &area.scene.terrain);
            let (mut buried, mut deepest, mut off_field) = (0usize, F::ZERO, 0usize);
            // How close the recorded bike ever came to this area's dirt, which is the other half of
            // the test: a ride that never comes within a wheel of an area's ground was not ridden
            // there either, however much air it leaves above it.
            let mut nearest: Option<F> = None;
            for &(x, y, z) in &rows {
                match ground.at(x, z) {
                    Some(hit) => {
                        let under = hit.y - y;
                        if under > BURIED {
                            buried += 1;
                        }
                        if under > deepest {
                            deepest = under;
                        }
                        nearest = Some(match nearest {
                            Some(n) if n < y - hit.y => n,
                            _ => y - hit.y,
                        });
                    }
                    None => off_field += 1,
                }
            }
            let landed = nearest.is_some_and(|n| n < BURIED);
            lines.push(format!(
                "  {}: {buried} of {} rows more than {BURIED:?} under the ground (deepest \
                 {deepest:?}, closest {:?} over it, {off_field} off the field)",
                area.named,
                rows.len(),
                nearest.unwrap_or(F::ZERO),
            ));
            // A row or two in a hundred buried is a landing, not another area: recording2's hard
            // landing at tick 342 (vy -32.5) bottoms the bike out 2.43 under Free09's blended
            // heightfield for one tick, and the wrong areas bury it on 37 to 906 rows of 641 (S32).
            if buried * BURIED_ONE_IN <= rows.len() && off_field == 0 && landed {
                candidates.push(area);
            }
        }
    }
    for line in &lines {
        println!("{line}");
    }
    assert!(
        candidates.len() <= 1,
        "{} of the disc's freeride areas could be the one this capture was recorded on, so the \
         capture does not say which world to replay it over:\n{}",
        candidates.len(),
        lines.join("\n"),
    );
    let area = candidates.pop()?;
    println!("the capture was recorded on {}", area.named);
    Some((area, lines))
}

/// The bike's recorded world position, row by row: the three lanes [`recorded_on`] compares against a
/// heightfield. `None` when the capture has no bike in it, which is an input-only capture.
fn bike_rows(cap: &Capture) -> Option<Vec<(F, F, F)>> {
    let lane = |column: &str| -> Option<Vec<F>> {
        let w = cap.float(column, column).ok()?;
        Some(
            w.samples
                .iter()
                .map(|s| match s.value {
                    Value::Float(bits) => F(f32::from_bits(bits)),
                    _ => F::ZERO,
                })
                .collect(),
        )
    };
    let (x, y, z) =
        (lane("bike_xform.m30")?, lane("bike_xform.m31")?, lane("bike_xform.m32")?);
    if x.len() != cap.rows.len() || y.len() != x.len() || z.len() != x.len() {
        return None;
    }
    Some((0..x.len()).map(|k| (x[k], y[k], z[k])).collect())
}

/// **The bike the human rode, off the disc**: `Bike\Race125P.phy`, read by the port's own loader.
///
/// D7 settled that a freeride boot loads the Race 125 pair and nothing else, and `tests/phy.rs`
/// settled that this port's read of that file is bit-for-bit the bike the original's loader had built
/// in the save state the recording starts from. Every bike model's archive carries the whole set of
/// physics files and the copies are identical (`tests/phy.rs` dedupes them to prove it), so the first
/// archive that has it is the file.
///
/// Without it the fixture rides [`game_sim::stock`]'s stand-in — a 250 assembled out of this crate's
/// own numbers — whose spring rates, tire forces and mass are not the recording's, so no amount of
/// right ground would make the suspension agree.
fn the_bike_the_human_rode(disc: &mut Disc, tables: &RiderTables) -> Option<BikePhy> {
    the_bike_named(disc, tables, FREERIDE_PHY)
}

/// [`the_bike_the_human_rode`] for any `.phy` (S89's free06 state rides `race250a.phy`).
fn the_bike_named(disc: &mut Disc, tables: &RiderTables, name: &str) -> Option<BikePhy> {
    for archive in disc.bike_archives() {
        let Some(bytes) = disc.member(&archive) else { continue };
        let Ok(res) = Res::parse(&bytes) else { continue };
        if let Some(phy) = by_suffix(&res, name).and_then(|e| res.read(&bytes, e).ok()) {
            println!("the bike: {name} out of {archive}, {} bytes", phy.len());
            return Some(BikePhy::read_bytes(&phy, &tables.ride.curve));
        }
    }
    println!("no {name} in any Bike\\*.res on this disc");
    None
}

/// The bike an acceptance fixture rides when the disc is here: the tables the `.phy` was read
/// against, the `.phy` itself, and where its two wheels hang off the chassis.
///
/// The mounts travel with the `.phy` rather than being taken off it because they are not in it: the
/// original's `bike_attach_nodes` looks the bike *model's* "TireFront" and "TireRear" nodes up by name
/// and writes them into each wheel's `+0xa0` (S22), and the `.phy`'s `BodyTireForcePosition` lands
/// at `+0xb0`, which no step reads.
struct Ridden {
    tables: Arc<RiderTables>,
    phy: BikePhy,
    mounts: (Vf, Vf),
}

/// The bike model the human's freeride was ridden on, as the archive names it. Not the `.phy`'s
/// name: the physics file is a class (`Race125P`, D7) and the model is a make — the recording's own
/// save state carries `125_faisst_engine` and `125_faisst_plasticsall` and no other 125's parts, and
/// its wheels' mounts are this model's tire nodes to the bit
/// (`the_acceptance_bike_hangs_its_wheels_where_the_original_does`).
const FREERIDE_MODEL: &str = "125_faisst";

/// **Where the human's wheels hang off the chassis**: the `TireFront` and `TireRear` nodes of the
/// `125_faisst`'s `bike.psg`, each node's place in the model's root frame — the read the shell makes
/// (`crates/openmx/src/main.rs`), but in the original's VU0 order ([`game_render::psg::Model::place_in`])
/// rather than through the renderer's host-float `world`.
///
/// This is the number S28 measured and could not attribute: with the `.phy`'s mounts at
/// `(0, -0.25, ±0.75)` both wheels start three quarters of a unit inside Free09 and tick 1 is shoved
/// up, and a floor 1.42 units lower would have fitted. The model's nodes sit at y ≈ 1.23 and 1.19,
/// 1.48 and 1.44 units higher — it was the mounts, and the ground is what D6 says it is.
fn the_wheels_the_human_rode(disc: &mut Disc) -> Option<(Vf, Vf)> {
    the_wheels_of(disc, FREERIDE_MODEL)
}

/// [`the_wheels_the_human_rode`] for any bike model (S89's free06 state rides `250_motoxxx`).
fn the_wheels_of(disc: &mut Disc, model: &str) -> Option<(Vf, Vf)> {
    let archives: Vec<String> = disc
        .bike_archives()
        .into_iter()
        .filter(|a| a.to_ascii_lowercase().contains(model))
        .collect();
    for archive in archives {
        let Some(bytes) = disc.member(&archive) else { continue };
        let Ok(res) = Res::parse(&bytes) else { continue };
        let Some(psg) = by_suffix(&res, "bike.psg").and_then(|e| res.read(&bytes, e).ok()) else {
            continue;
        };
        let Ok(model) = game_render::psg::Model::parse(&psg) else { continue };
        // The frame the place is asked in is the bike's own node — the model's root, which is what
        // `bike+0x18` holds — and the arithmetic is the original's node lookup, not the renderer's
        // `world`: the two differ in the last bit of the front mount's `z`.
        let node = |name: &str| -> Option<Vf> {
            let i = model.nodes.iter().position(|n| n.name == name)?;
            let mut root = i;
            while let Some(up) = model.nodes[root].parent {
                root = up;
            }
            Some(Vf(model.place_in(i, root)))
        };
        if let (Some(front), Some(rear)) = (node("TireFront"), node("TireRear")) {
            println!("the wheels: TireFront {front:?} and TireRear {rear:?} out of {archive}");
            return Some((front, rear));
        }
    }
    None
}

/// The EE image saved beside the human's recording, and the human's rider in it: the rider R6's chain
/// lands on (the game manager's actor list, two hops in), the same chain `watch/s22-standing.toml` and
/// `watch/s8-start.toml` record live. The addresses here are research addresses in a test that reads a
/// RAM image — never in the port.
///
/// The image is the frame *before* the acceptance capture's first row: its frame counter is one short
/// of row 0's, and the bike in it is one physics step behind row 0's. So it can only seed what the
/// original holds still across that step, and [`SaveState::leads`] says whether a capture is the one
/// it leads into.
struct SaveState {
    ram: Vec<u8>,
    rider: u32,
}

impl SaveState {
    /// The game manager singleton's slot (`@game_manager` in `kit.toml`).
    const GAME_MANAGER: u32 = 0x004e_1e98;
    /// The simulated-frame counter (`@frame_counter`).
    const FRAME_COUNTER: u32 = 0x0047_9d04;

    /// `None`, with a line saying why, when the image is not in this worktree or the chain does not
    /// land on the human's rider.
    fn open() -> Option<SaveState> {
        let at = private_dir("states").join(format!("{RECORDING}_SaveState.eeMemory.bin"));
        Self::open_image(&at, &[0x34, 0x34, 0x8, 0x8, 0x0])
    }

    /// Any save state's RAM image whose rider chain lands where the recording's does (S106's
    /// teleported states keep the same actors).
    fn open_at(at: PathBuf) -> Option<SaveState> {
        Self::open_image(&at, &[0x34, 0x34, 0x8, 0x8, 0x0])
    }

    /// [`SaveState::open`] on another image, whose human rider is `hops` down from the game manager.
    fn open_image(at: &std::path::Path, hops: &[u32]) -> Option<SaveState> {
        let Ok(ram) = std::fs::read(at) else {
            println!("no {} — `./kit state ram` of the recording's save state writes it", at.display());
            return None;
        };
        let mut state = SaveState { ram, rider: 0 };
        let mut a = state.word(Self::GAME_MANAGER)?;
        for &hop in hops {
            a = state.word(a + hop)?;
        }
        // A rider (type 2) and the local human's (control source 1), or the chain is pointing elsewhere.
        if state.word(a)? != 2 || state.word(state.word(a + 0x20)? + 0x34)? != 1 {
            println!("the chain in {} does not land on the human's rider", at.display());
            return None;
        }
        state.rider = a;
        Some(state)
    }

    fn word(&self, a: u32) -> Option<u32> {
        let a = (a & 0x01ff_ffff) as usize;
        Some(u32::from_le_bytes(self.ram.get(a..a + 4)?.try_into().ok()?))
    }

    fn float(&self, a: u32) -> Option<F> {
        Some(F(f32::from_bits(self.word(a)?)))
    }

    fn quad(&self, a: u32) -> Option<[u32; 4]> {
        Some([self.word(a)?, self.word(a + 4)?, self.word(a + 8)?, self.word(a + 12)?])
    }

    /// The pad's control state (`@pad0_control_state`): sixteen controls, 0x18 bytes each, `held`
    /// the first word.
    const CONTROL_STATE: u32 = 0x0067_8f08;

    /// The controls the game believed were held when the state was saved, as the game's bitmask.
    fn held_controls(&self) -> Option<u16> {
        let mut held = 0;
        for (i, c) in Control::ALL.iter().enumerate() {
            if self.word(Self::CONTROL_STATE + 0x18 * i as u32)? == 1 {
                held |= c.mask();
            }
        }
        Some(held)
    }

    /// The rider's word at `+off`, followed as a pointer.
    fn rider_at(&self, off: u32) -> Option<u32> {
        self.word(self.rider + off)
    }

    /// The type-4 actor `watch/acceptance.toml` records as `bike_xform`/`bike_vel`: eight hops down
    /// the same list.
    fn vehicle(&self) -> Option<u32> {
        let mut a = self.word(Self::GAME_MANAGER)?;
        for hop in [0x34, 0x34, 0x8, 0x8, 0x8, 0x8, 0x8, 0x8, 0x8, 0x8, 0x0] {
            a = self.word(a + hop)?;
        }
        (self.word(a)? == 4).then_some(a)
    }

    /// This image is the frame just before `cap`'s first row.
    fn leads(&self, cap: &Capture) -> bool {
        let first = cap.uint("frame_counter", "frame_counter").ok().and_then(|w| match w.samples.first() {
            Some(s) => match s.value {
                Value::Uint(n) => Some(n),
                _ => None,
            },
            None => None,
        });
        match (self.word(Self::FRAME_COUNTER), first) {
            (Some(saved), Some(first)) => u64::from(saved) + 1 == first,
            _ => false,
        }
    }

    /// The human's two wheel mounts as the original held them: the front wheel at `+0x7b0` and the
    /// rear at `+0x7b4`, each mount at the wheel's `+0xa0`.
    fn wheels(&self) -> Option<[[u32; 4]; 2]> {
        Some([self.quad(self.rider_at(0x7b0)? + 0xa0)?, self.quad(self.rider_at(0x7b4)? + 0xa0)?])
    }

    /// The part of the drivetrain the original holds still from this frame through the capture's
    /// first row, read off the rider (`bike_clutch`'s `+0x6c0`/`+0x6c4`/`+0x6cc`, the rate-limited
    /// throttle and clutch at `+0x658`/`+0x65c`) and its engine (`+0x7b8`: `+0x64` locked, `+0x68` the
    /// gear, `+0x88` the launch multiplier, `+0x8c` the curve cursor, `+0xa8` the revs).
    /// `watch/s8-start.toml` shows every one of them unchanged across the frames before the human
    /// first touches the pad, which is what makes them fit to seed a run from row 0 with.
    fn drivetrain(&self) -> Option<Drivetrain> {
        let r = self.rider;
        let e = self.rider_at(0x7b8)?;
        Some(Drivetrain {
            clutch_timer: self.float(r + 0x6c0)?,
            clutch_scale_start: self.float(r + 0x6c4)?,
            clutch_cofg_weight: self.float(r + 0x6cc)?,
            gas: self.float(r + 0x658)?,
            clutch: self.float(r + 0x65c)?,
            locked: self.word(e + 0x64)? != 0,
            gear: self.word(e + 0x68)? as i32,
            launch: self.float(e + 0x88)?,
            curve: (self.word(e + 0x8c)?, self.word(e + 0x90)?, self.float(e + 0x94)?),
            rpm: self.float(e + 0xa8)?,
        })
    }
}

/// See [`SaveState::drivetrain`].
#[derive(Clone, Copy, Debug)]
struct Drivetrain {
    clutch_timer: F,
    clutch_scale_start: F,
    clutch_cofg_weight: F,
    gas: F,
    clutch: F,
    locked: bool,
    gear: i32,
    launch: F,
    curve: (u32, u32, F),
    rpm: F,
}

impl Drivetrain {
    /// Puts it into a ride that has just been placed. The gear goes through `set_gear`, so its ratio
    /// and reciprocal come with it.
    fn seed(&self, ride: &mut game_sim::ride::Ride) {
        ride.mv.clutch_timer = self.clutch_timer;
        ride.mv.clutch_scale_start = self.clutch_scale_start;
        ride.mv.clutch_cofg_weight = self.clutch_cofg_weight;
        ride.held.gas = self.gas;
        ride.held.clutch = self.clutch;
        ride.mv.clutch_in = self.clutch;
        game_sim::r#move::set_gear(&mut ride.engine, self.gear);
        ride.engine.clutch_locked = self.locked;
        ride.engine.clutch_torque_scale = self.launch;
        ride.engine.curve = game_sim::r#move::Curve::at(self.curve.0, self.curve.1, self.curve.2);
        ride.engine.rpm = self.rpm;
    }
}

/// **The acceptance bike hangs its wheels where the original does, bit for bit.** The mounts the
/// fixture is built with ([`the_wheels_the_human_rode`], off the disc) against the two the original's
/// rider was carrying in the recording's own save state. The disc's answer is the one the run uses;
/// the save state is the proof that it is the right model and the right read. Skipped with a line
/// where either is missing.
#[test]
fn the_acceptance_bike_hangs_its_wheels_where_the_original_does() {
    let Some(mut disc) = Disc::open() else { return };
    let Some(saved) = SaveState::open().and_then(|s| s.wheels()) else { return };
    let (front, rear) = the_wheels_the_human_rode(&mut disc)
        .unwrap_or_else(|| panic!("no {FREERIDE_MODEL} bike.psg with TireFront/TireRear nodes"));
    let bits = |v: Vf| [v.x().to_bits(), v.y().to_bits(), v.z().to_bits(), v.w().to_bits()];
    assert_eq!(bits(front), saved[0], "the front mount: model {front:?}");
    assert_eq!(bits(rear), saved[1], "the rear mount: model {rear:?}");
}

/// **The acceptance capture watches the bike as it is drawn, not the chassis the physics moves** — which
/// is why no lane of it can be matched by the port's chassis alone, and why `ACCEPTANCE_AGREES_THROUGH`
/// is held where it is. Proved on the recording's own save state:
///
/// - `bike_xform` (the type-4 actor's `+0x180`) is, word for word, the world transform of the node at
///   rider `+0x964` — the *drawn* bike's root, whose children are the model's `Dummy_Foot_L`/`_R` —
///   which the rider's frame-end copy (`+0xbd0`) hands the actor. The chassis' own node is rider
///   `+0x18` (the chassis' `+0x254`), and the drawn root hangs off it through a pose of its own: about
///   a degree and a half of roll and 0.065 units of sideways offset on a bike standing on the rider's
///   foot, held rigid across `watch/s8-start.toml`'s 120 frames.
/// - `bike_vel` (`+0x130`) is the chassis' velocity (`+0x260`) times 0.8, lane for lane.
#[test]
fn the_acceptance_capture_watches_the_drawn_bike_not_the_chassis() {
    let Some(st) = SaveState::open() else { return };
    let vehicle = st.vehicle().expect("eight hops down the actor list is the human's type-4 bike");
    let drawn = st.rider_at(0x964).expect("the drawn bike's root node");
    let body = st.rider_at(0x18).expect("the chassis' node");
    let chassis = st.rider_at(0x7a4).expect("the chassis");
    assert_eq!(st.rider_at(0x7a4).and_then(|c| st.word(c + 0x254)), Some(body), "the chassis' own node");
    for row in 0..4 {
        let at = |base: u32| st.quad(base + 16 * row).unwrap();
        assert_eq!(at(vehicle + 0x180), at(drawn + 0x80), "bike_xform row {row} is the drawn root's");
    }
    let lane = |base: u32, i: u32| st.float(base + 4 * i).unwrap();
    let apart = (0..3).map(|i| (lane(vehicle + 0x1b0, i).0 - lane(body + 0xb0, i).0).abs()).fold(0.0, f32::max);
    assert!(apart > 0.05, "the drawn root and the chassis are {apart} apart: the capture would be the chassis");
    for i in 0..3 {
        let want = lane(chassis + 0x260, i) * F(0.8);
        assert_eq!(lane(vehicle + 0x130, i).0.to_bits(), want.0.to_bits(), "bike_vel lane {i}");
    }
}

/// **Everything the disc's side of an acceptance run is**, handed to `then`: the world the capture was
/// recorded on, the name of it for the fixture to carry, and the bike it was ridden on.
///
/// A callback rather than a return value because the three things a world is made of borrow each other
/// in order — the archive's bytes, the [`Terrain`] parsed out of them, the [`Ground`] that reads the
/// terrain — so all three have to be locals of one stack frame. `None` means the disc is not in this
/// worktree, and then there is no acceptance verdict to take at all: replaying a ride over a flat floor
/// is what S8 measured and S28 is here to stop.
fn in_the_world_of<R>(
    cap: &Capture,
    then: impl FnOnce(&mut dyn World, &Where, Option<Ridden>) -> R,
) -> Option<R> {
    in_the_world_on(cap, FREERIDE_PHY, FREERIDE_MODEL, then)
}

/// [`in_the_world_of`] on a bike of its own: the `.phy` and the model whose tire nodes hang its wheels.
fn in_the_world_on<R>(
    cap: &Capture,
    phy: &str,
    model: &str,
    then: impl FnOnce(&mut dyn World, &Where, Option<Ridden>) -> R,
) -> Option<R> {
    let mut disc = Disc::open()?;
    let (area, _) = recorded_on(cap, &mut disc)?;
    let (props, missed) = area.props(&mut disc);
    let terrain = Terrain::parse(&area.terrain).ok()?;
    let ground = Ground::new(&terrain, &area.scene.terrain);
    let (wide, long) = ground.extent();
    println!(
        "the world: {} x {} ground samples {:?} apart ({wide:?} x {long:?} of it), {} of {} \
         collision meshes",
        ground.samples_x,
        ground.samples_z,
        ground.spacing,
        props.props.len(),
        area.scene.collision.len(),
    );
    for line in missed.iter().take(3) {
        println!("  {line}");
    }
    // One table built here and shared by the `.phy` read and the state that runs on it: the loader
    // folds the game's own {cos, sin} curve into half a dozen parameters, so a bike read against one
    // table and ridden against another is a different bike.
    let tables = Arc::new(RiderTables::build());
    let bike = the_bike_named(&mut disc, &tables, phy).map(|phy| {
        let mounts = the_wheels_of(&mut disc, model).unwrap_or_else(|| {
            println!(
                "no {model} model with TireFront/TireRear nodes on this disc: the wheels \
                 hang off the .phy's BodyTireForcePosition instead, which is not where the \
                 original hangs them, and the run will part at tick 1",
            );
            (phy.front.body_tire_force_position, phy.rear.body_tire_force_position)
        });
        Ridden { tables: Arc::clone(&tables), phy, mounts }
    });
    let mut place = Place { ground, props };
    Some(then(&mut place, &area.named, bike))
}

/// **The acceptance recording was made on Free09, on the Race 125 the loader builds for a freeride
/// boot** — the two halves of S28's answer to "replayed over what?", each stated on its own so that a
/// failure says which one moved.
///
/// Neither comes from a column in the capture: the watch spec records the bike, not the level, and
/// re-recording the minute to add one needs the human and the GUI. The area comes out of the ride's own
/// geometry ([`recorded_on`]) and the bike out of the loader's rule for a freeride boot (D7:
/// `Bike\Race125P.phy`, the `P` half of the Race 125 pair, which is the class
/// [`game_sim::track::FREERIDE_ENGINE`] names). Skipped with a line, not failed, where the disc is not
/// in the worktree.
#[test]
fn the_acceptance_recording_was_made_on_free09_on_the_race_125() {
    let Some((_, cap)) = acceptance_capture() else { return };
    let Some(mut disc) = Disc::open() else {
        println!("no MX.PSR in this worktree: which world the capture was recorded on is unchecked");
        return;
    };
    let (area, lines) = recorded_on(&cap, &mut disc)
        .unwrap_or_else(|| panic!("the capture's own rows fit none of the disc's freeride areas"));
    assert_eq!(
        (area.named.track.as_str(), area.named.area.as_str()),
        ("Tracks\\free09.res", "tracks\\free09\\free09.scn"),
        "the capture's rows fit a different area than the one S28 measured:\n{}",
        lines.join("\n"),
    );
    let tables = RiderTables::build();
    let phy = the_bike_the_human_rode(&mut disc, &tables)
        .unwrap_or_else(|| panic!("no {FREERIDE_PHY} in any of the disc's bike archives"));
    // Not an assertion about the numbers — `tests/phy.rs` proves this file against the live bike in the
    // recording, word for word. This is the statement that the fixture is built on *that* read: a
    // `.phy` that parsed to nothing would leave a bike with no suspension and no tires.
    assert!(
        phy.front.spring > F::ZERO
            && phy.rear.spring > F::ZERO
            && phy.front.tire_radius > F::ZERO
            && phy.mass.mass > F::ZERO,
        "{FREERIDE_PHY} read as a bike with no springs, no wheels or no mass",
    );
}

/// **Every acceptance capture in `context/capture/` is the original at a step the port can match.**
///
/// The three properties `watch/acceptance.toml` asks a human to check by eye, checked instead, on every
/// capture that carries its bike columns: the step is the clamp on every tick, there is a row per tick
/// with no gap, and the actor the chain resolves to is a vehicle with a *local human* on it rather than
/// one of the area's three AI riders. All three are invisible in a CSV of 1,811 rows, and each of them
/// is a way a capture can look like an acceptance recording and quietly not be one.
///
/// It also reports how much *input* each capture holds. `acceptance-freeride.csv` is the human's own
/// minute played back on the original (see this section's header), so that count is 1,449 of 1,811
/// ticks with a control held rather than the zero it was while the recording had no way into the
/// emulator. A count of zero is still not a failure — a capture of the same state under no input is
/// the original at a step the port can match, and it is the kind of capture an agent can make without
/// a GUI — it is printed where whoever runs the tests will see it.
#[test]
fn every_acceptance_capture_is_the_original_at_a_step_the_port_can_match() {
    let dirs = acceptance_dirs();
    let mut found = 0;
    for path in dirs.iter().flat_map(|d| files_in(d, "csv")) {
        let text = std::fs::read_to_string(&path).expect("a readable capture");
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        // The acceptance spec's own column. A capture of `watch/input.toml` has the pad and no bike,
        // and is the business of the input tests above, not of this one.
        if column(&cap, "bike_ctl_src").is_err() {
            continue;
        }
        found += 1;

        let ticks = ticks_of(&cap).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let (least, most) = dt_spread(&cap).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let f = acceptance_fixture(&cap).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let played = f.inputs.iter().filter(|i| i.buttons != 0 || i.stick != (0, 0)).count();
        println!(
            "{}: {} rows, frames {}..{}, dt {least}..{most}, {} inputs ({played} with anything \
             held), {} checks",
            path.display(),
            cap.rows.len(),
            cap.frames[0],
            cap.frames[cap.frames.len() - 1],
            f.inputs.len(),
            f.checks(),
        );
        if played == 0 {
            println!(
                "  — and nobody was touching the pad for any of it: this capture proves the step, \
                 the density and the actor, and its recorded input has no original to agree with \
                 (S8's journal entry says why)",
            );
        }

        // One row per tick, from the game's own counter: `ticks_of` refuses a gap, so the last tick
        // being the row count minus one is the statement that none were dropped either.
        assert_eq!(ticks.len(), cap.rows.len());
        assert_eq!(
            *ticks.last().expect("a non-empty capture"),
            cap.rows.len() as u64 - 1,
            "{}: the capture's ticks are not consecutive",
            path.display(),
        );
        // The 30 fps clamp, on every tick: the measurement I4 left to this task.
        assert_eq!(
            (least.to_bits(), most.to_bits()),
            (replay::CAPPED_DT.to_bits(), replay::CAPPED_DT.to_bits()),
            "{}: dt is not the clamp throughout ({least}..{most}) — a run the original measured \
             frame by frame cannot be replayed by a fixed-step port",
            path.display(),
        );
        assert_eq!(f.inputs.len() + 1, cap.rows.len(), "one input per tick but the first");
        assert!(
            f.inputs.len() >= 30 * 10,
            "{}: {} ticks is not a run of anything",
            path.display(),
            f.inputs.len(),
        );
        assert_eq!(f.watches.len(), BIKE.len(), "every bike column became a watch");
        assert_eq!(f.checks(), BIKE.len() * cap.rows.len(), "no empty cell in any of them");

        // The verdict itself is taken over the ground this capture was recorded on and on the bike
        // it was recorded with, both off the disc (S28); `f` above is the same fixture over the flat
        // floor, which is enough for the shape assertions and not for the ride.
        let taken = in_the_world_of(&cap, |world, named, bike| {
            let on_it = acceptance_fixture_on(&cap, bike)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            acceptance_verdict(&Fixture { world: Some(named.clone()), ..on_it }, world);
        });
        if taken.is_none() {
            println!(
                "  — and the world it was ridden on is not in this worktree, so there is no \
                 verdict for it here: the ratchet is unchecked, not met",
            );
        }
    }
    if found == 0 {
        println!("{}", acceptance_recipe(&dirs[0].join(format!("{ACCEPTANCE}.csv"))));
    }
}

/// **The recording's bytes are the bytes the original read** — `report_of_p2m2` against libpad's own
/// buffer in EE RAM, over the whole minute the human played.
///
/// Two paths to the same twenty bytes, sharing no code: a `.p2m2` on disk read by I4's reader, and the
/// `pad_a`/`pad_b` columns of a capture taken through PINE after each tick (the half with the smaller
/// stamp being the one the game read — C5's measurement, which `PadTape::from_capture` applies). The
/// capture is that recording *played back on the original*, so the comparison is no longer limited to
/// the ticks where both pads happened to be at rest: over all 1,810 overlapping ticks the two produce
/// byte-identical `PadFrame`s — status, the mode byte, the button word active-low, the four axes, the
/// twelve pressures, the `dt`, and the absent second pad.
///
/// **And which of a tick's two fields the game reads**, which is the question I4 could not answer and
/// an undriven capture never could: fields 1..=55 of the recording are a written, neutral pad, so
/// before the human's first press both phases say the same thing and either one "agrees". Past the
/// press only one can. `TickPhase::Second` matches RAM on every one of the 1,810 overlapping ticks;
/// `First` matches the neutral prefix and then sits one field behind for ever, its tick 28 being what
/// RAM already held on tick 27. **The game reads the second field of each tick** — field 56 of this
/// recording is the first input it ever saw.
///
/// Both halves are asserted, so a reader that mangled the neutral word, the mode byte or the second
/// pad, an alignment that slipped, or a future capture taken from a different state would each show up
/// here rather than as a divergence somewhere in the physics.
#[test]
fn the_capture_is_the_recording_played_back_and_the_game_reads_the_second_field() {
    let Some((cpath, cap)) = acceptance_capture() else { return };
    let rpath = private_dir("recordings").join(RECORDING);
    let Ok(bytes) = std::fs::read(&rpath) else {
        println!(
            "{} is not in this worktree, so the capture has nothing to be compared against; copy \
             it in from the main checkout's context/recordings/",
            rpath.display(),
        );
        return;
    };
    let movie = replay::P2m2::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", rpath.display()));
    assert_eq!(movie.ticks(replay::TickPhase::First), RECORDING_TICKS, "3621 fields, two a tick");

    // The recording's idle prefix, measured from the file: every field up to here is a good read of a
    // pad with nothing down and both sticks centred, which is exactly `Report::idle`.
    let idle_fields = (0..movie.fields())
        .position(|k| movie.field(k, 0) != Report::idle())
        .expect("the human pressed something in sixty seconds");
    assert_eq!(
        idle_fields + 1,
        FIRST_PLAYED_FIELD,
        "the recording's first input is on field {} now, not {FIRST_PLAYED_FIELD}: the agreement \
         window below is measured against that number",
        idle_fields + 1,
    );

    // Where the capture starts inside the recording. The recorder samples after the tick, so row 0's
    // counter is the tick that *produced* it and the first frame of a tape from the capture is row 1's.
    let at = column(&cap, "frame_counter").expect("the spec's own counter column");
    let first: u64 = cell(&cap, 0, at).parse().expect("a number");
    assert!(
        first > RECORDING_START_TICK,
        "the capture starts at tick {first}, before the recording's own start state \
         ({RECORDING_START_TICK}): it is not a capture of this recording's minute",
    );
    let offset = (first - RECORDING_START_TICK - 1) as usize;

    let from_ram = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{}: {e}", cpath.display()));
    assert_eq!(
        from_ram.dt_range(),
        Some((replay::CAPPED_DT, replay::CAPPED_DT)),
        "the capture's own dt column is the clamp, which is the step a tape of the recording assumes",
    );

    for phase in [replay::TickPhase::First, replay::TickPhase::Second] {
        let tape = movie.tape(phase);
        let played = tape
            .frames
            .iter()
            .position(|f| f.held_mask(0) != 0 || f.stick(0) != (0, 0))
            .expect("the human pressed something in sixty seconds");
        let overlap = tape.len().saturating_sub(offset).min(from_ram.len());
        let agreed = (0..overlap)
            .take_while(|&k| tape.frames[offset + k] == from_ram.frames[k])
            .count();
        println!(
            "{} vs {}: tape {} ticks, capture {} ticks from the recording's tick {offset}, \
             {overlap} overlapping; phase {}: the human's first input is tick {played} (field {}), \
             and the two agree byte for byte on {agreed} ticks",
            rpath.file_name().unwrap().to_string_lossy(),
            cpath.file_name().unwrap().to_string_lossy(),
            tape.len(),
            from_ram.len(),
            phase.name(),
            FIRST_PLAYED_FIELD,
        );
        assert!(
            agreed > 0,
            "phase {}: the recording's tick {offset} and the capture's first tick are both an \
             untouched pad and they do not match byte for byte — one of the two readers is wrong \
             about the neutral encoding:\n  tape    {:?}\n  capture {:?}",
            phase.name(),
            tape.frames[offset],
            from_ram.frames[0],
        );
        match phase {
            // The phase the game reads: the whole overlap, byte for byte.
            replay::TickPhase::Second => assert_eq!(
                agreed,
                overlap,
                "phase second: the capture is this recording played back on the original, so all \
                 {overlap} overlapping ticks must match byte for byte; tick {} does not:\n  \
                 tape    {:?}\n  capture {:?}",
                offset + agreed,
                tape.frames[offset + agreed],
                from_ram.frames[agreed],
            ),
            // The other phase, and why it is the other phase: it holds for the neutral prefix, and
            // loses it one tick before its *own* first input, because RAM already carries field 56's.
            replay::TickPhase::First => {
                assert_eq!(
                    offset + agreed,
                    played - 1,
                    "phase first: the agreement ends at tick {}, and the only place it can end is \
                     the tick before this phase's first input (tick {played}) — RAM has field \
                     {FIRST_PLAYED_FIELD}'s press on the tick before the first-field tape sees it",
                    offset + agreed,
                );
                assert_eq!(
                    tape.frames[offset + agreed + 1],
                    from_ram.frames[agreed],
                    "phase first: past the press the wrong phase should be a one-field shift of the \
                     right one, not a different pad — the capture's tick {} is the first-field \
                     tape's next tick or the two readers disagree about more than the phase",
                    offset + agreed,
                );
            }
        }
    }
}

/// `GasRateLimit` in `Bike\\Race125P.phy` — the bike the human rode — read off the disc with the
/// port's own `.phy` reader. Ten a second, so the ride walks the throttle from shut to pinned in a
/// tenth of a second: three ticks of the engine's 30 Hz cap, whatever the pad did.
const GAS_RATE_LIMIT: F = F(10.0);

/// **How much of a human's minute is part throttle.** P9's third half: the answer is the recording's,
/// and it is not the one the task expected.
///
/// The question is whether the throttle wants to be a pressure in this port — the original reads Cross
/// from its pressure byte, so the disc could ride part throttle, and the shell could not. The one
/// minute of real play the project has is `first_recording.p2m2`, and asked directly it says:
///
/// * **Every pressure byte in it is 0 or 255.** Not just Cross — all twelve analogue controls, on all
///   1,810 ticks of the phase the game read. The human's throttle was a switch.
/// * **The left stick in the same file is analogue**, 154 distinct bytes of it. So this is
///   not a keyboard recording: it is somebody on a pad whose sticks travel and whose face buttons do
///   not, which is every pad sold since the DualShock 2 — and PCSX2 writes what the host pad gave it.
/// * **The throttle moved 18 times in sixty seconds** — nine stabs, the longest held for 346 ticks
///   (11.5 s) and the shortest for 8 — so what the rider did with it was pin it and shut it.
/// * **What part throttle there is, the bike makes itself.** Run the recording through the port's own
///   poll and the 125's `GasRateLimit`, and `Held::gas` is strictly between shut and pinned on 27 of
///   the 1,810 ticks: three ticks after each of the nine stabs, 1.5% of the minute and not one tick of
///   it asked for by the rider. A release shuts it the same tick: the original's limiter only holds
///   the gas back on the way up (S58; it was 54, three either side of all 18 edges, before).
///
/// That is a measurement and not a verdict on the shell: a recording made through PCSX2 can only
/// carry what the host pad reported, so this says nothing about what a DualShock 2 on a PlayStation 2
/// would have written, and the game's own binding still reads a pressure. What it does say is that
/// the part throttle this project can *prove* the original was ridden on is the rate limit's, and that
/// a shell with no analogue throttle at all was not failing to reproduce anything in this file. That
/// 3% is the floor the shell's keyboard ramp lifts: it opens the throttle over 0.3 s against the
/// limit's 0.1, so a stab on a key spends nine ticks on part throttle where this minute spent three.
#[test]
fn the_human_recording_is_a_switch_and_the_part_throttle_in_it_is_the_rate_limit_s() {
    use game_sim::input::Manager;
    use game_sim::phy::Joystick;
    use game_sim::ride::{Controls, Held};

    let rpath = private_dir("recordings").join(RECORDING);
    let Ok(bytes) = std::fs::read(&rpath) else {
        println!(
            "{} is not in this worktree, so how the human rode the throttle cannot be counted; \
             copy it in from the main checkout's context/recordings/",
            rpath.display(),
        );
        return;
    };
    let movie = replay::P2m2::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", rpath.display()));
    // The phase the game read, measured from this file and its save state against RAM (S8).
    let phase = replay::TickPhase::Second;
    let ticks = movie.ticks(phase);
    // One tick fewer than [`RECORDING_TICKS`]: the second-field phase starts a field in, so the
    // file's last field has no partner and the tape is 1,810 ticks rather than 1,811.
    assert_eq!(ticks, RECORDING_TICKS - 1, "3621 fields, two a tick, the first one skipped");

    // Every pressure byte of every analogue control, and the throttle's own three counts.
    let mut between = Vec::new();
    let (mut shut, mut pinned) = (0usize, 0usize);
    // The left stick's bytes, to tell a pad from a keyboard: report byte 6 is left X.
    let mut stick_bytes = std::collections::BTreeSet::new();
    // The gas the port's poll makes of it, and the gas the bike is left holding after the limit.
    let mut manager = Manager::connected();
    let joystick = Joystick { gas_rate_limit: GAS_RATE_LIMIT, ..Joystick::default() };
    let mut held = Held::default();
    let dt = replay::CAPPED_DT;
    let (mut asked_part, mut held_part, mut held_pinned) = (0usize, 0usize, 0usize);
    // And how often the throttle changed its mind at all.
    let (mut edges, mut stabs) = (0usize, 0usize);
    let mut was_on = false;

    for tick in 0..ticks {
        let field = tick * replay::P2M2_FIELDS_PER_TICK + phase.offset();
        let report = movie.field(field, 0);
        for control in Control::ALL {
            let binding = control.binding();
            if !binding.is_analogue() {
                continue;
            }
            let pressure = report.pressure(binding.pressure);
            if pressure != 0 && pressure != 0xff {
                between.push((tick, control, pressure));
            }
        }
        let throttle = report.pressure(Control::Cross.binding().pressure);
        match throttle {
            0 => shut += 1,
            0xff => pinned += 1,
            _ => {}
        }
        stick_bytes.insert(report.0[6]);

        manager.tick(dt, &[report, movie.field(field, 1)]);
        let asked = Controls::from_pad(manager.pad(0));
        if asked.gas > F::ZERO && asked.gas < F::ONE {
            asked_part += 1;
        }
        held.step(&asked, &joystick, dt);
        if held.gas > F::ZERO && held.gas < F::ONE {
            held_part += 1;
        } else if held.gas == F::ONE {
            held_pinned += 1;
        }

        let on = throttle != 0;
        if on != was_on {
            edges += 1;
            stabs += usize::from(on);
            was_on = on;
        }
    }

    println!(
        "the human's throttle over {ticks} ticks: {pinned} pinned, {shut} shut, {} between, \
         {edges} edges in {stabs} stabs; the rate limit holds part throttle on {held_part} ticks \
         ({:.1}%) and full on {held_pinned}; left stick {} distinct bytes",
        between.len(),
        100.0 * held_part as f32 / ticks as f32,
        stick_bytes.len(),
    );

    assert!(
        between.is_empty(),
        "{} of this recording's pressure bytes are between the stops (first: tick {}, {} at {:#x}) \
         — a human's part throttle is in this file after all, and the counts in this test's own \
         comment are of a different recording",
        between.len(),
        between[0].0,
        between[0].1.name(),
        between[0].2,
    );
    assert_eq!((pinned, shut), (1_303, 507), "the throttle's own two states");
    assert_eq!(asked_part, 0, "a switch asks for nothing in between, by definition");
    assert_eq!((edges, stabs), (18, 9), "how often the human touched the throttle");
    assert!(
        stick_bytes.len() > 100,
        "the left stick holds {} distinct bytes, so this recording's source may be digital \
         throughout and the finding above — an analogue stick beside a switched throttle — is not \
         what this file shows",
        stick_bytes.len(),
    );
    assert_eq!(
        held_part, 27,
        "the 125's rate limit leaves part throttle on {held_part} of {ticks} ticks, not 27 — three \
         ticks after each of the {stabs} stabs: either the limit, the step or the recording \
         is not the one this was measured on"
    );
    assert_eq!(held_pinned, 1_276, "and full throttle on the rest of the time the button was down");
}

// ---------------------------------------------------------------------------------------------------
// A5 — one whole frame of the original's bike, compared against the port's same frame
// ---------------------------------------------------------------------------------------------------
//
// Everything above this line compares the port against the original *one function at a time*, or
// against a fixture the port itself recorded. S6's entry says what that cannot catch, in its own
// words: "a whole-frame comparison would catch an ordering mistake between the animation header and
// the rest of the rider update that these per-function proofs cannot". Call three ported functions in
// the wrong order and every per-function proof still passes.
//
// S8's answer to that — replaying a human's recording tick by tick — now has its half of the data,
// the recording captured off the original (I3's header), and no bike in the port to replay it against
// until S3 lands. This section is the whole-frame proof that waits for neither: `watch/
// frame-bike.toml` samples the human's rider **once per tick** on the live emulator, from the save
// state the recording is anchored to, with the kit's own pad riding it (`watch/pads/
// frame-bike-ride.pad`). Each row carries, for the same tick:
//
//   the physics' outputs     `crossup` (+0x600) and `lean` (+0x674), written by the air state and the
//                            ground step — calls 1 and 2 of the rider's frame
//   the actor update's       `rider_pos` (+0x820), written by the attachment walk out of the chassis,
//                            beside the chassis' own world transform — call 4
//   the animation's outputs  `action`, `ramp`, `faded`, `fore_aft`, `pose` and `shown_crossup` — the
//                            six fields `motion::Driver` is, written by the animation header in the
//                            tail of the stunt clocks — the last call of the frame
//
// A producer and its consumer sampled on the same row is what makes the order measurable: feed the
// port the physics of **this** row and it reproduces the animation bit for bit; feed it the physics of
// the row before and it does not. That is the ordering mistake S6 could not catch, caught by a test
// rather than argued from a disassembly listing.
//
// What the capture cannot say is in A5's journal entry under "Not verified": the chassis integrate is
// not compared numerically (the port has no bike yet), the attachment walk is proved by a distance
// rather than bit-exactly, and the rider's lean was zero on every grounded frame of the capture, so
// `Pose::lean` is fed zero rather than measured.

/// The columns a capture must have to be a whole-frame capture of the bike. `rider_ctl_src` rather
/// than `bike_ctl_src` deliberately: that is the name S8's acceptance scanner keys on, and this
/// capture is not an acceptance recording — it holds no input columns at all.
const WHOLE_FRAME_COLUMNS: [&str; 5] =
    ["rider_ctl_src", "crossup", "lean_fore_aft", "shown_crossup", "action"];

/// What the original held on one tick: the inputs of the rider's frame, and what its animation header
/// left behind. One row of `watch/frame-bike.toml`.
struct WholeFrame {
    /// The tick, from the game's own counter with the first row at 0.
    tick: u64,
    dt: F,
    // --- the physics (calls 1 and 2 of the rider's frame) ---
    crossup: F,
    lean_roll: F,
    lean_fore_aft: F,
    /// The ride state's reason, 0 while riding and the wreck reason while the rider is down.
    wreck_reason: u32,
    // --- the actor update (call 4) ---
    rider_pos: [f32; 3],
    veh_pos: [f32; 3],
    // --- what the header was asked for ---
    requested: u8,
    seq: u32,
    prev_seq: i32,
    blend_rate: F,
    max_crossup: F,
    // --- what the header left behind ---
    action: u8,
    ramp: F,
    faded: bool,
    fore_aft: F,
    pose: F,
    shown_crossup: F,
}

fn float_at(cap: &Capture, row: usize, at: usize, name: &str) -> Result<F, String> {
    let text = cell(cap, row, at);
    // The recorder writes floats through `repr()`, so this parse is the capture's exact bits back.
    text.parse::<f32>()
        .map(F)
        .map_err(|_| format!("row {row}: {name} is '{text}', which is not a number"))
}

fn int_at(cap: &Capture, row: usize, at: usize, name: &str) -> Result<u32, String> {
    let text = cell(cap, row, at);
    text.parse::<u32>()
        .map_err(|_| format!("row {row}: {name} is '{text}', which is not a whole number"))
}

/// Read a whole-frame capture, with every check that makes it evidence rather than a CSV.
///
/// The actor the chains resolve to has to be the *human's rider* (type 2, control source 1) and the
/// *human's vehicle* (type 4, control source 1) on every row — Free09 holds three AI riders on three
/// more vehicles, and the only field that tells them apart is the control source on the player record.
/// The step has to be the engine's 30 fps clamp throughout, bit for bit, or the frame the original ran
/// is not a frame the port can be handed. And the rows have to be consecutive ticks of one simulation
/// (`ticks_of`, off the game's counter rather than the recorder's vsync `frame`), because a lag test
/// across a gap compares two frames that were never neighbours.
fn whole_frame(cap: &Capture) -> Result<Vec<WholeFrame>, String> {
    let ticks = ticks_of(cap)?;
    let at = |name: &str| column(cap, name);
    let (dt, crossup, wrecked) = (at("dt")?, at("crossup")?, at("wreck_reason")?);
    let (lean_roll, lean_fore_aft) = (at("lean_roll")?, at("lean_fore_aft")?);
    let rider_pos = [at("rider_pos.x")?, at("rider_pos.y")?, at("rider_pos.z")?];
    let veh_pos = [at("veh_xform.m30")?, at("veh_xform.m31")?, at("veh_xform.m32")?];
    let (requested, seq, prev_seq) = (at("requested")?, at("seq")?, at("prev_seq")?);
    let (blend_rate, max_crossup) = (at("blend_rate")?, at("max_crossup")?);
    let (action, ramp, faded) = (at("action")?, at("ramp")?, at("faded")?);
    let (fore_aft, pose, shown) = (at("fore_aft")?, at("pose")?, at("shown_crossup")?);
    let (rider_type, rider_src) = (at("rider_type")?, at("rider_ctl_src")?);
    let (veh_type, veh_src) = (at("veh_type")?, at("veh_ctl_src")?);

    let mut out = Vec::with_capacity(cap.rows.len());
    for row in 0..cap.rows.len() {
        // The actor the chain found, before anything is read out of it.
        let (kind, src) = (int_at(cap, row, rider_type, "rider_type")?, int_at(cap, row, rider_src, "rider_ctl_src")?);
        if (kind, src) != (2, 1) {
            return Err(format!(
                "row {row}: the rider chain resolves to type {kind} control source {src}, not the \
                 human's rider (type 2, source 1) — change the number of '-> 0x8' hops in \
                 watch/frame-bike.toml until it does, and re-record",
            ));
        }
        let (kind, src) = (int_at(cap, row, veh_type, "veh_type")?, int_at(cap, row, veh_src, "veh_ctl_src")?);
        if (kind, src) != (4, 1) {
            return Err(format!(
                "row {row}: the vehicle chain resolves to type {kind} control source {src}, not the \
                 human's vehicle (type 4, source 1) — one of the area's AI riders, or no vehicle",
            ));
        }
        let step = float_at(cap, row, dt, "dt")?;
        if step.to_bits() != replay::CAPPED_DT.to_bits() {
            return Err(format!(
                "row {row}: dt is {} ({:#010x}), not the engine's 30 fps clamp {} ({:#010x}) — the \
                 original integrated this frame with a step the port would not use",
                step.0,
                step.to_bits(),
                replay::CAPPED_DT.0,
                replay::CAPPED_DT.to_bits(),
            ));
        }
        let pos = |at: [usize; 3]| -> Result<[f32; 3], String> {
            Ok([
                float_at(cap, row, at[0], "a position")?.0,
                float_at(cap, row, at[1], "a position")?.0,
                float_at(cap, row, at[2], "a position")?.0,
            ])
        };
        out.push(WholeFrame {
            tick: ticks[row],
            dt: step,
            crossup: float_at(cap, row, crossup, "crossup")?,
            lean_roll: float_at(cap, row, lean_roll, "lean_roll")?,
            lean_fore_aft: float_at(cap, row, lean_fore_aft, "lean_fore_aft")?,
            wreck_reason: int_at(cap, row, wrecked, "wreck_reason")?,
            rider_pos: pos(rider_pos)?,
            veh_pos: pos(veh_pos)?,
            requested: int_at(cap, row, requested, "requested")? as u8,
            seq: int_at(cap, row, seq, "seq")?,
            prev_seq: int_at(cap, row, prev_seq, "prev_seq")? as i32,
            blend_rate: float_at(cap, row, blend_rate, "blend_rate")?,
            max_crossup: float_at(cap, row, max_crossup, "max_crossup")?,
            action: int_at(cap, row, action, "action")? as u8,
            ramp: float_at(cap, row, ramp, "ramp")?,
            faded: int_at(cap, row, faded, "faded")? != 0,
            fore_aft: float_at(cap, row, fore_aft, "fore_aft")?,
            pose: float_at(cap, row, pose, "pose")?,
            shown_crossup: float_at(cap, row, shown, "shown_crossup")?,
        });
    }
    Ok(out)
}

/// Every whole-frame capture in `context/capture/`, read. Like the acceptance scanner next door: a
/// capture that carries the columns is checked, a malformed one is a failure, and no captures at all
/// is a printed sentence rather than a pass that checked nothing — the directory is game data and is
/// never committed.
fn whole_frame_captures() -> Vec<(PathBuf, Vec<WholeFrame>)> {
    let dir = private_dir("capture");
    let mut out = Vec::new();
    for path in files_in(&dir, "csv") {
        let text = std::fs::read_to_string(&path).expect("a readable capture");
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if !WHOLE_FRAME_COLUMNS.iter().all(|c| column(&cap, c).is_ok()) {
            continue;
        }
        let rows = whole_frame(&cap).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        out.push((path, rows));
    }
    if out.is_empty() {
        println!(
            "no whole-frame capture in {} — record one:\n  \
             cp context/recordings/first_recording.p2m2_SaveState.p2s \\\n    \
             \"<pcsx2 copy>/PCSX2/savestates/SLUS-20767 (9866FE54).08.p2s\"\n  \
             ./kit pcsx2 with -- ./kit record watch/frame-bike.toml --state 8 \\\n    \
             --pad watch/pads/frame-bike-ride.pad -n 1400 -o context/capture/frame-bike-ride.csv",
            dir.display(),
        );
    }
    out
}

/// How much the capture has to hold before any of the proofs below mean anything.
///
/// A capture of a bike standing still reads action 28, ramp 0 and a constant pose on every row: the
/// lag tests all "pass" on it because nothing in them ever changes, which is the one way this whole
/// section could be green and prove nothing. So the floor is in a constant, checked once, and quoted
/// in the failure: airborne frames (the only arm that writes `shown_crossup`), cross-fade restarts,
/// and distinct lean values (the header's other live input).
const NEEDS: (usize, usize, usize) = (100, 5, 10);

fn airborne(rows: &[WholeFrame]) -> Vec<usize> {
    (0..rows.len()).filter(|&t| rows[t].action == Trick::WHEELIE.0).collect()
}

fn restarts(rows: &[WholeFrame]) -> Vec<usize> {
    (1..rows.len())
        .filter(|&t| rows[t].ramp == F::ONE && rows[t - 1].ramp < F::ONE)
        .collect()
}

/// **A whole-frame capture is one tick of the human on the bike, with something happening in it.**
#[test]
fn a_whole_frame_capture_is_the_human_riding() {
    for (path, rows) in whole_frame_captures() {
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(rows.len() > 1, "{name}: a capture of {} row(s) proves no ordering", rows.len());
        // `whole_frame` has already refused a gap, an AI's actor and a dt that is not the clamp.
        assert_eq!(
            rows.last().unwrap().tick,
            rows.len() as u64 - 1,
            "{name}: {} rows but the last tick is {} — the rows are not consecutive ticks",
            rows.len(),
            rows.last().unwrap().tick,
        );
        let air = airborne(&rows);
        let back = restarts(&rows);
        let leans: std::collections::BTreeSet<u32> =
            rows.iter().map(|r| r.lean_fore_aft.to_bits()).collect();
        let (need_air, need_restarts, need_leans) = NEEDS;
        assert!(
            air.len() >= need_air && back.len() >= need_restarts && leans.len() >= need_leans,
            "{name}: {} airborne frame(s), {} restart(s), {} distinct lean(s) — a capture has to hold \
             at least {need_air}/{need_restarts}/{need_leans} or the proofs below hold nothing still \
             against nothing moving. Ride it: watch/pads/frame-bike-ride.pad",
            air.len(),
            back.len(),
            leans.len(),
        );
        println!(
            "{name}: {} ticks of the human's rider, dt the clamp throughout; {} airborne, {} \
             restart(s), {} distinct lean(s), {} distinct crossup(s), {} frame(s) down",
            rows.len(),
            air.len(),
            back.len(),
            leans.len(),
            rows.iter().map(|r| r.crossup.to_bits()).collect::<std::collections::BTreeSet<u32>>().len(),
            rows.iter().filter(|r| r.wreck_reason != 0).count(),
        );
    }
}

/// What one frame of the port's animation driving disagreed with the original about.
///
/// `aside` is why the disagreement is not the port's fault, where the capture itself says so — the
/// two cases are in [`driving_against_the_capture`]. One with an `aside` is reported and not failed;
/// one without is the port and the original disagreeing about a number, which is a failure.
struct Disagreement {
    tick: u64,
    field: &'static str,
    port: String,
    game: String,
    aside: Option<&'static str>,
}

impl std::fmt::Display for Disagreement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tick {}: {} {} vs {}", self.tick, self.field, self.port, self.game)?;
        match self.aside {
            Some(why) => write!(f, " ({why})"),
            None => Ok(()),
        }
    }
}

/// Whether two rows hold the same frame: every sampled field equal, bit for bit.
///
/// A capture of a *moving* bike cannot produce two such rows honestly — the chassis moves 1.8 units
/// in the median frame of A5's ride — so a pair of them means the sample at the second one caught a
/// frame the simulation had not stepped. Measured: one row in 1,400, and the frame after it carries
/// two frames of cross-fade decay against one frame of chassis travel. See A5's journal entry.
fn the_same_frame(a: &WholeFrame, b: &WholeFrame) -> bool {
    let bits = |r: &WholeFrame| {
        (
            [r.crossup.to_bits(), r.lean_roll.to_bits(), r.lean_fore_aft.to_bits()],
            [r.ramp.to_bits(), r.fore_aft.to_bits(), r.pose.to_bits(), r.shown_crossup.to_bits()],
            [r.action, r.requested],
            r.rider_pos.map(f32::to_bits),
            r.veh_pos.map(f32::to_bits),
        )
    };
    bits(a) == bits(b)
}

/// Run `motion::Driver` over a captured ride and compare all six of its fields against what the
/// original's animation header actually wrote, bit for bit, on every tick.
///
/// `lag` is the ordering knob and the whole point of this function: at 0 the driving is handed the
/// physics outputs sampled on the **same** tick, which is what the original's frame order means; at 1
/// it is handed the previous tick's, which is what a port that ran the animation header before the
/// physics would compute.
///
/// Three inputs the capture cannot see, and what is fed instead:
///
/// - `Request::force` — the trick machine's "asked for again" flag, which lives in `Tricks` and not in
///   the rider. Fed `false`. It is not a hole in the proof: a forced restart would put the captured
///   ramp back to 1 on a tick where the port left it falling, and the ramp is compared bit-exactly.
/// - `Request::progress` — only the stunt arm reads it, and the capture holds no stunt frames (the
///   ride never lands a trick). Fed zero.
/// Everything else is the capture's own: the step, the two request ids, the request count and its
/// previous copy, both of the engine's `MaxCrossupAngle` copies, the blend rate, the crossup angle and
/// both leans. Where those came from is worth knowing — the two lean fields (+0x670 and +0x674) are
/// not in any disassembly listing of the header; they were found by searching a capture of the whole
/// 2,688-byte rider object for the field each output is a function of.
///
/// `Blend` — what the motion library was asked for — is not compared, because nothing in the rider
/// object records it; S6's oracle proofs cover that side.
fn driving_against_the_capture(
    rows: &[WholeFrame],
    lag: usize,
) -> (usize, Vec<Disagreement>, Vec<Disagreement>) {
    let first = &rows[0];
    let mut driver = Driver {
        action: Trick(first.action),
        ramp: first.ramp,
        faded: first.faded,
        fore_aft: first.fore_aft,
        pose: first.pose,
        shown_crossup: first.shown_crossup,
    };
    // The rows whose sample caught a frame the simulation had not stepped yet.
    let stalled: Vec<bool> = (0..rows.len())
        .map(|t| t > 0 && the_same_frame(&rows[t], &rows[t - 1]))
        .collect();
    let mut found = Vec::new();
    let mut aside = Vec::new();
    for t in 1..rows.len() {
        let row = &rows[t];
        let physics = &rows[t - lag];
        // The rider is down and being put back on the bike: the ride state's reason (+0x748) is
        // non-zero, and for a frame or two after it clears, the reset puts the pose state back —
        // including the shown crossup, which it sets to 0.5, straight ahead. `Driver` has no reset
        // (S6 ported the driving, not the state machine), so a disagreement in that window is
        // reported separately rather than failing the comparison.
        let respawning = (t.saturating_sub(2)..=t).any(|s| rows[s].wreck_reason != 0);
        // A stalled sample is two frames the port cannot step through: the one that did not move,
        // and the one after it, which moved twice.
        let why = if respawning {
            Some("the rider was down and being put back on the bike, which Driver does not model")
        } else if stalled[t] || stalled[t - 1] {
            Some("the capture's sample caught a frame the simulation had not stepped")
        } else {
            None
        };
        driver.frame(
            row.dt,
            Params { blend_rate: row.blend_rate, max_crossup: row.max_crossup },
            Request {
                action: Trick(row.requested),
                force: false,
                seq: row.seq as u8,
                prev_seq: row.prev_seq,
                frozen: false,
                progress: F::ZERO,
            },
            Pose {
                lean: physics.lean_roll,
                fore_aft: physics.lean_fore_aft,
                crossup: physics.crossup,
            },
            Clips::default(),
        );
        let mut note = |mut d: Disagreement| {
            d.aside = why;
            if why.is_some() {
                aside.push(d);
            } else {
                found.push(d);
            }
        };
        let mut bad = |field: &'static str, port: F, game: F| {
            if port.to_bits() != game.to_bits() {
                note(Disagreement {
                    tick: row.tick,
                    field,
                    port: format!("{} ({:#010x})", port.0, port.to_bits()),
                    game: format!("{} ({:#010x})", game.0, game.to_bits()),
                    aside: None,
                });
            }
        };
        bad("ramp", driver.ramp, row.ramp);
        bad("fore_aft", driver.fore_aft, row.fore_aft);
        bad("pose", driver.pose, row.pose);
        bad("shown_crossup", driver.shown_crossup, row.shown_crossup);
        if driver.action.0 != row.action {
            note(Disagreement {
                tick: row.tick,
                field: "action",
                port: driver.action.0.to_string(),
                game: row.action.to_string(),
                aside: None,
            });
        }
        if driver.faded != row.faded {
            note(Disagreement {
                tick: row.tick,
                field: "faded",
                port: driver.faded.to_string(),
                game: row.faded.to_string(),
                aside: None,
            });
        }
        // The capture is the original's own state, so a diverged driver is re-anchored to it before
        // the next frame: without this, one bad tick reports as a thousand and the first disagreement
        // — the one that matters — is buried.
        driver.action = Trick(row.action);
        driver.ramp = row.ramp;
        driver.faded = row.faded;
        driver.fore_aft = row.fore_aft;
        driver.pose = row.pose;
        driver.shown_crossup = row.shown_crossup;
    }
    (rows.len() - 1, found, aside)
}

/// **The port's animation driving is the original's, frame for frame, on the live game.**
///
/// Not a generated frame and not an oracle call: the captured ride's own dt, request ids, cross-fade
/// state, lean and crossup angle, against the six fields the original's header wrote on that tick.
#[test]
fn the_ported_driving_matches_the_whole_captured_frame() {
    for (path, rows) in whole_frame_captures() {
        let name = path.file_name().unwrap().to_string_lossy();
        let (frames, bad, aside) = driving_against_the_capture(&rows, 0);
        assert!(
            bad.is_empty(),
            "{name}: the port's animation driving disagrees with the original on {} of {frames} \
             frame(s). First: {}",
            bad.len(),
            bad[0],
        );
        println!(
            "{name}: the port's animation driving agrees with the original on every one of {frames} \
             live frames — six fields, {} restart(s), {} airborne frame(s), max crossup {}",
            restarts(&rows).len(),
            airborne(&rows).len(),
            rows[0].max_crossup.0,
        );
        // Not a pass and not a failure: what the live frame holds that the port has no code for. The
        // lines are printed in full because this is the one thing in the section a reader could not
        // work out from the source.
        // Not a pass and not a failure: the frames the capture itself excuses. They are printed in
        // full, and there have to be few of them — an excuse that covers a tenth of the ride is not
        // an excuse, it is a port that does not match.
        assert!(
            aside.len() * 50 < frames,
            "{name}: {} of {frames} frame(s) disagree for a reason the capture excuses, which is too \
             many to excuse. First: {}",
            aside.len(),
            aside[0],
        );
        if !aside.is_empty() {
            println!(
                "{name}: and {} disagreement(s) the capture itself accounts for:{}",
                aside.len(),
                aside.iter().map(|d| format!("\n    {d}")).collect::<String>(),
            );
        }
    }
}

/// **The animation header runs after the physics.** The ordering proof, and the reason this section
/// exists: the same replay, handed the physics outputs of the *previous* tick, stops matching.
///
/// This is the mistake no per-function proof can catch. A port that calls the animation header before
/// the air state would compute exactly what `lag = 1` computes here — every ported function right, the
/// frame wrong — and it diverges on the live capture within a frame or two of the rider moving.
#[test]
fn the_driving_reads_the_physics_of_its_own_frame() {
    for (path, rows) in whole_frame_captures() {
        let name = path.file_name().unwrap().to_string_lossy();
        let (frames, late, _) = driving_against_the_capture(&rows, 1);
        assert!(
            !late.is_empty(),
            "{name}: the driving matched on all {frames} frames when it was handed the *previous* \
             frame's crossup and lean, which means this capture cannot tell the two orders apart — \
             the physics outputs never moved. Ride it harder.",
        );
        // A tenth of the moving frames is a floor with room in it: the lean holds still for seconds
        // at a time under a scripted pad, and a held value is the same at both lags.
        let moving = (1..rows.len())
            .filter(|&t| {
                rows[t].crossup.to_bits() != rows[t - 1].crossup.to_bits()
                    || rows[t].lean_roll.to_bits() != rows[t - 1].lean_roll.to_bits()
                    || rows[t].lean_fore_aft.to_bits() != rows[t - 1].lean_fore_aft.to_bits()
            })
            .count();
        assert!(
            late.len() * 10 >= moving,
            "{name}: handing the driving last frame's physics only broke {} of the {moving} frame(s) \
             where the physics moved — too few to call an ordering proof",
            late.len(),
        );
        println!(
            "{name}: fed the previous frame's physics the same driving breaks on {} frame(s) of \
             {frames} ({moving} with the physics moving); first {}",
            late.len(),
            late[0],
        );
    }
}

/// **The header latches the id the state pack asked for, in the same frame.**
///
/// `requested` (+0x864) is written by the stunt state pack, call 5 of the rider's frame; `action`
/// (+0xa78) is what the animation header latched to run on. They are equal on every captured tick and
/// unequal on 1 in 60 when the comparison is shifted a frame, which puts the header after the pack.
#[test]
fn the_header_latches_this_frames_requested_id() {
    for (path, rows) in whole_frame_captures() {
        let name = path.file_name().unwrap().to_string_lossy();
        let same = |lag: usize| {
            (1..rows.len()).filter(|&t| rows[t].action == rows[t - lag].requested).count()
        };
        let frames = rows.len() - 1;
        assert_eq!(
            same(0),
            frames,
            "{name}: the id the header ran on is not the id asked for on the same tick, on {} of \
             {frames} frame(s)",
            frames - same(0),
        );
        assert!(
            same(1) < frames,
            "{name}: the ids are equal a frame apart too, on all {frames} frames — the requested id \
             never changed, so this capture cannot say which frame the latch happened in",
        );
        println!(
            "{name}: action == requested on {frames}/{frames} ticks, and on {}/{frames} a frame \
             apart — the header latches the pack's id in its own frame",
            same(1),
        );
    }
}

/// **The actor update carries this frame's chassis, not last frame's.**
///
/// The rider's seat position (+0x820) is written by the attachment walk, call 4 of the rider's frame,
/// out of the chassis the physics has already moved. Sampled against the chassis' own
/// world transform it is a rigid pair *within one tick*: the gap between them stays a small constant
/// offset, while the gap to the previous tick's chassis is a whole frame of travel.
///
/// This one is a tolerance argument and not a bit-exact proof, and it is the weakest thing in this
/// section — A5's journal entry says so under "Not verified". Nothing in the capture reproduces the
/// chassis transform from the rider's own fields (no 16-float block in the rider object matches it,
/// and no field satisfies `pos[t] - pos[t-1] == vel * dt`), so the comparison is between distances:
/// the same-frame gap has to be a small fraction of how far the bike moves in a frame, and the
/// one-frame-late gap has to be of the order of that motion.
#[test]
fn the_attachment_update_carries_this_frames_chassis() {
    for (path, rows) in whole_frame_captures() {
        let name = path.file_name().unwrap().to_string_lossy();
        let dist = |a: [f32; 3], b: [f32; 3]| {
            let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        };
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        let gap = |lag: usize| -> Vec<f32> {
            (1..rows.len()).map(|t| dist(rows[t].rider_pos, rows[t - lag].veh_pos)).collect()
        };
        let (here, late) = (gap(0), gap(1));
        let mut motion: Vec<f32> =
            (1..rows.len()).map(|t| dist(rows[t].veh_pos, rows[t - 1].veh_pos)).collect();
        motion.sort_by(f32::total_cmp);
        let median = motion[motion.len() / 2];
        let (here_mean, late_mean) = (mean(&here), mean(&late));
        assert!(
            median > 0.1,
            "{name}: the bike moved {median} units in the median frame — it was parked, and a \
             distance test on a parked bike says nothing about the order of the frame",
        );
        assert!(
            here_mean * 4.0 < median && late_mean > median / 2.0,
            "{name}: the rider's seat is {here_mean} from this frame's chassis and {late_mean} from \
             last frame's, against {median} units of travel in the median frame — that is not the \
             signature of an attachment walk that ran after this frame's physics",
        );
        println!(
            "{name}: the rider's seat sits {here_mean} from this tick's chassis and {late_mean} from \
             the previous tick's, with {median} units of travel in the median frame — the attachment \
             walk ran after the physics, in the same frame",
        );
    }
}

// ---------------------------------------------------------------------------------------------------
// P2 — a freeride fixture, replayed
// ---------------------------------------------------------------------------------------------------
//
// The shell's ride is `game_sim::ride::Ride` on the disc's parameters, driven by the four ported
// modules, and what P2's brief asks for is the one property that makes it worth porting anything
// else into: the same session, ridden twice, is the same session. The three above it are the
// harness' own, and this one is the harness pointed at the thing it was built for.
//
// It is a fixture in the file sense — `Fixture`'s inputs, hashes and watches, round-tripping through
// the same text format as every other fixture in the project — but not in the `verify` sense:
// `replay::verify` builds a `Sim`, which is the demo simulation, and the ride is not one. So the
// walk, the comparison and the failure line are the module's; the loop that steps the state is this
// section's, because there is nothing to hand the ride to.
//
// The bike is `tests/freeride-bike.phy.txt`, hand-written (the disc's `.phy` cannot be committed),
// and the world is a flat floor: what is being replayed is the rider, not the track.

use game_sim::hash::{self, Dump};
use game_sim::input::{Manager, AXIS_CENTRE};
use game_sim::phy::Phy;
use game_sim::replay::Verdict;
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::world::Flat;

/// The frame the fixture is recorded at: the engine's clamped step.
const FREERIDE_DT: F = F(1.0 / 60.0);

/// How many frames of it — ten seconds, which is long enough that a divergence in the middle has
/// hundreds of frames of ride behind it and hundreds in front.
const FREERIDE_FRAMES: usize = 600;

/// The fields the fixture watches, on top of the whole-state hash: a handful of columns, the way a
/// capture of the original would arrive. They are here so that a failure can say *what* diverged and
/// not only that something did.
const FREERIDE_WATCHED: [&str; 8] = [
    "chassis.cofg_world[1]",
    "chassis.cofg_world[2]",
    "chassis.speed",
    "engine.rpm",
    "engine.gear",
    "front.w.compression",
    "rear.mv.spin",
    "air.airborne",
];

/// A session at the pad: the clutch in and the throttle open off the line, the stick working, the
/// brake on and off, a trick asked for now and then. Written as the game's own held-button mask
/// ([`Control::mask`]) plus the left stick as it arrives from the pad, because that is all a
/// [`Fixture`] stores.
fn freeride_inputs(frames: usize) -> Vec<Input> {
    (0..frames)
        .map(|k| {
            let mut buttons = Control::Cross.mask();
            if k < 15 {
                buttons |= Control::L2.mask();
            }
            if k % 150 >= 130 {
                buttons |= Control::Square.mask();
            }
            if k % 180 >= 168 {
                buttons |= Control::Circle.mask() | Control::Up.mask();
            }
            if k % 240 >= 228 {
                buttons |= Control::L3.mask();
            }
            let t = k as f32;
            let stick = (
                ((t * 0.09).sin() * 110.0) as i8,
                ((t * 0.05).cos() * 90.0) as i8,
            );
            Input { buttons, stick }
        })
        .collect()
}

/// One frame of pad as the platform layer would have handed it over: a digital press is full pressure
/// ([`Report::from_buttons`]), and the stick's two bytes are the arrival value the fixture stored.
///
/// This is why the fixture's lossy `Input` is enough here — the session was played on buttons, so the
/// reports can be rebuilt from the mask exactly, and the ported poll derives the strengths and the
/// press timers from them as it did when the fixture was recorded.
fn freeride_frame(i: &Input) -> PadFrame {
    let mut report = Report::from_buttons(i.buttons);
    let byte = |v: i8| (i16::from(AXIS_CENTRE) + i16::from(v)).clamp(0, 255) as u8;
    report.set_axis(2, byte(i.stick.0));
    report.set_axis(3, byte(i.stick.1));
    PadFrame::new(FREERIDE_DT, [report, Report::missing()])
}

/// A freeride, driven exactly the way the shell drives one: the pad through the ported input layer,
/// the rider's hands off the pad ([`Controls::from_pad`]), the bike on a floor.
struct Freeride {
    tables: Tables,
    phy: Phy,
    input: Manager,
    world: Flat,
    ride: Ride,
}

impl Freeride {
    fn new() -> Freeride {
        let tables = Tables::build();
        let phy = Phy::read_bytes(include_str!("freeride-bike.phy.txt").as_bytes(), &tables.curve);
        let mounts = game_sim::stock::MOUNTS;
        let ride =
            Ride::new(&phy, mounts, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0));
        let mut input = Manager::new();
        input.pad_mut(0).connected = true;
        Freeride { tables, phy, input, world: Flat::default(), ride }
    }

    /// One frame, in the shell's order: read the pad, then ride.
    fn advance(&mut self, frame: &PadFrame) {
        self.input.tick(frame.dt(), &frame.reports);
        let controls = Controls::from_pad(self.input.pad(0));
        self.ride.frame(&self.phy, &self.tables, &controls, frame.dt(), &mut self.world);
    }

    fn hash(&self) -> u64 {
        hash::of(&self.ride)
    }
}

/// Ride the session and write down what happened: a hash every tick and the watched fields every
/// thirtieth, which is the fixture.
fn freeride_fixture(frames: usize) -> Fixture {
    let inputs = freeride_inputs(frames);
    let mut run = Freeride::new();
    let mut hashes = Vec::with_capacity(frames);
    let mut samples: Vec<Vec<Sample>> = vec![Vec::new(); FREERIDE_WATCHED.len()];
    for (k, i) in inputs.iter().enumerate() {
        run.advance(&freeride_frame(i));
        hashes.push(run.hash());
        let tick = k as u64 + 1;
        if tick % 30 == 0 {
            let dump = Dump::of(&run.ride);
            for (w, path) in FREERIDE_WATCHED.iter().enumerate() {
                let leaf = dump.get(path).unwrap_or_else(|| panic!("{path} is not in the ride"));
                samples[w].push(Sample { tick, value: Value::of_leaf(leaf) });
            }
        }
    }
    let watches = FREERIDE_WATCHED
        .iter()
        .zip(samples)
        .map(|(path, samples)| Watch::new(*path, samples))
        .collect();
    // No world: the floor these numbers were taken over is the flat one `verify` rides, which is the
    // whole point of the section — what is being replayed is the rider, not the track.
    Fixture { name: "freeride".into(), seed: 0, start: None, inputs, hashes, watches, world: None }
}

/// Replay a freeride fixture: ride its inputs and compare every hash and every sample, reporting the
/// first tick that disagrees in the same shape [`Divergence`] would.
///
/// `nudge` is the deliberate break the test below needs — "at this tick, move the bike" — and is
/// `None` for an honest replay.
fn replay_freeride(f: &Fixture, nudge: Option<u64>) -> Result<usize, String> {
    let mut run = Freeride::new();
    // One cursor per watch, as `verify` walks them.
    let mut cursor = vec![0usize; f.watches.len()];
    let mut checked = 0;
    for (k, i) in f.inputs.iter().enumerate() {
        run.advance(&freeride_frame(i));
        let tick = k as u64 + 1;
        if nudge == Some(tick) {
            let v = run.ride.chassis.vel;
            run.ride.chassis.vel = Vf::new(v.x(), v.y(), v.z() + 0.5, v.w());
        }
        if let Some(&want) = f.hashes.get(k) {
            checked += 1;
            if want != run.hash() {
                // The watches say which field it was, where the hash can only say that it was one.
                let dump = Dump::of(&run.ride);
                let named = f.watches.iter().find_map(|w| {
                    let s = w.samples.iter().find(|s| s.tick == tick)?;
                    let got = dump.get(&w.path)?;
                    (s.value.compare(got) != Verdict::Same).then(|| {
                        format!("{} {} vs {}", w.path, s.value.as_leaf(), got)
                    })
                });
                return Err(match named {
                    Some(line) => format!("tick {tick}: {line}"),
                    None => format!("tick {tick}: state hash {want:#018x} vs {:#018x}", run.hash()),
                });
            }
        }
        let dump = Dump::of(&run.ride);
        for (w, watch) in f.watches.iter().enumerate() {
            while let Some(s) = watch.samples.get(cursor[w]).filter(|s| s.tick == tick) {
                cursor[w] += 1;
                checked += 1;
                let got = dump
                    .get(&watch.path)
                    .ok_or_else(|| format!("tick {tick}: {} is not in the walk", watch.path))?;
                if s.value.compare(got) != Verdict::Same {
                    return Err(format!(
                        "tick {tick}: {} {} vs {got}",
                        watch.path,
                        s.value.as_leaf()
                    ));
                }
            }
        }
    }
    // A sample past the end of the run is the fixture's own problem, not a pass.
    for (w, watch) in f.watches.iter().enumerate() {
        if cursor[w] != watch.samples.len() {
            return Err(format!(
                "{}: {} of {} samples were never reached — the run ended at tick {}",
                watch.path,
                cursor[w],
                watch.samples.len(),
                f.inputs.len(),
            ));
        }
    }
    Ok(checked)
}

/// **The brief's test.** A freeride replays to the same hash twice.
#[test]
fn a_freeride_fixture_replays_to_the_same_hash_twice() {
    let f = freeride_fixture(FREERIDE_FRAMES);
    let checked = replay_freeride(&f, None).expect("the recorded ride replays");
    assert_eq!(checked, f.checks());
    assert_eq!(checked, FREERIDE_FRAMES + FREERIDE_WATCHED.len() * (FREERIDE_FRAMES / 30));

    // And the recording itself is reproducible: a second ride of the same session writes the same
    // fixture, which is the property a fixture recorded today and replayed next year rests on.
    let again = freeride_fixture(FREERIDE_FRAMES);
    assert_eq!(f.hashes, again.hashes, "two recordings of one session");
    assert_eq!(f.watches, again.watches);
}

/// It is a ride and not a parked bike: a fixture of a frozen state would replay perfectly and prove
/// nothing.
#[test]
fn the_freeride_fixture_is_a_ride() {
    let f = freeride_fixture(FREERIDE_FRAMES);
    let distinct: std::collections::BTreeSet<u64> = f.hashes.iter().copied().collect();
    assert!(distinct.len() > FREERIDE_FRAMES * 8 / 10, "{} distinct hashes", distinct.len());

    let moved = |path: &str| {
        let w = f.watches.iter().find(|w| w.path == path).expect(path);
        let first = w.samples.first().expect("samples").value;
        w.samples.iter().any(|s| s.value != first)
    };
    assert!(moved("chassis.cofg_world[2]"), "the bike never went anywhere");
    assert!(moved("chassis.speed"), "the bike never changed speed");
    // The engine's own revs, read upstream of the idle clamp. `engine.rpm` is the wrong column to
    // ask this of on *this* bike: with the clutch locked the engine turns at the road's revs
    // (`9.549 * spin * ratio`), the fixture bike's first gear is 2.1 against a 3.6 final drive, and
    // a flat floor leaves it cruising at about 17 rad/s of wheel — some 1700 road rpm, under the
    // file's `MinRPM = 1800`. So `engine.rpm` sits on its floor for the whole session and steps off
    // it only for the odd single frame, which no 30-frame sample lands on. It stays in the watch
    // list, where the replay compares it tick for tick; what says the engine is turning here is the
    // driven wheel it is locked to. (Before S22 this read `engine.rpm` and passed on a different
    // accident: the old solver threw the bike into the air for half the session, and a wheel with no
    // ground under it free-revs the engine. The fixture bike's `EngineBraking = 0.3` is its own
    // oddity, 0.3 * rpm^2 of crank braking, and it is what knocks the spin to -51000 rad/s in the
    // first ten frames — before this task's changes and after them alike.)
    assert!(moved("rear.mv.spin"), "the engine never changed revs");
    assert!(moved("front.w.compression"), "the front suspension never moved");
}

/// The file format carries a freeride fixture exactly, floats and all, so one recorded in the port
/// today is replayable from a file.
#[test]
fn a_freeride_fixture_round_trips_as_a_file() {
    let f = freeride_fixture(120);
    let text = f.to_text();
    let read = Fixture::parse(&text).expect("a fixture this module wrote");
    assert_eq!(read.name, f.name);
    assert_eq!(read.inputs, f.inputs);
    assert_eq!(read.hashes, f.hashes);
    assert_eq!(read.watches, f.watches);
    assert_eq!(read.checks(), f.checks());
    replay_freeride(&read, None).expect("the fixture read back off the file replays too");
}

/// A ride that diverges fails at the tick it diverged on, with the field named — not at tick 1, and
/// not at the end.
#[test]
fn a_diverging_freeride_fails_at_the_tick_it_diverged_on() {
    let f = freeride_fixture(300);
    let why = replay_freeride(&f, Some(150)).expect_err("a nudged bike is a different bike");
    assert!(why.starts_with("tick 150:"), "{why}");

    // A nudge at a tick the fixture does not hash is still caught, at the next tick that is checked —
    // which here is the next frame, because this fixture hashes every one of them.
    let sparse = Fixture { hashes: Vec::new(), ..freeride_fixture(300) };
    let why = replay_freeride(&sparse, Some(150)).expect_err("the watches catch it on their own");
    assert!(why.starts_with("tick 180:"), "{why}");
    assert!(why.contains("chassis."), "the failure names the field: {why}");
}

// ---------------------------------------------------------------------------------------------------
// The shell's own tape: what `--record` writes, and what `--play` rides (P7)
// ---------------------------------------------------------------------------------------------------
//
// The playable shell can now write every frame's pad to a tape and ride a tape instead of a pad. The
// file is the same `PadTape` a capture of the original and a human's `.p2m2` become, which is the
// point: one reader, one writer, and a tape of the port and a tape of the original are the same kind
// of file.
//
// Two things have to be true for that to be worth anything, and they are what this section proves:
//
//   * what the shell wrote reads back **frame for frame** — every report byte and every bit of every
//     `dt`, because a tape that rounds a frame length is a tape of a different run;
//   * a tape **plays the same every time** — the pad and the frame length both come off the tape and
//     nothing else reaches the simulation, so riding one twice lands on the same state hash.
//
// The shell's half (the window, the file, the loop) is not here; what is here is the recorder and the
// playback head it drives, so the discipline is testable without a disc, a window or a pad.

use game_sim::replay::{Head, Recorder};

/// A session as the shell would have recorded one: the frame lengths an engine actually hands out
/// (zero on the first frame, then the averaged ones, with the odd long frame the 30 fps clamp lets
/// through) and a pad somebody is using — the stick wandering, and the throttle picked up part way in
/// at a strength a button mask could not hold.
fn shell_session(frames: usize) -> Recorder {
    let mut tape = Recorder::new("thq1 on free06 / 125sx");
    for i in 0..frames {
        // The engine's first frame has no previous frame to measure against, so its dt is exactly
        // zero; after that the average, with a long frame every so often.
        let dt = match i {
            0 => F::ZERO,
            _ if i % 97 == 0 => F(0.041532),
            _ if i % 3 == 0 => F(0.03283332),
            _ => replay::CAPPED_DT,
        };
        let mut bytes = Report::idle().0;
        bytes[6] = (0x7f + (i as i32 % 61) - 30) as u8;
        bytes[7] = (0x7f - (i as i32 % 41) + 20) as u8;
        if i > frames / 3 {
            // Cross held: its bit in the active-low word, and the pressure the game reads it from.
            let low = !Control::Cross.mask();
            bytes[2] = (low >> 8) as u8;
            bytes[3] = low as u8;
            bytes[CONTROL_MAP[Control::Cross.index()].pressure as usize] = 0x40 + (i % 0x80) as u8;
        }
        tape.frame(dt, &[Report::from_bytes(bytes), Report::missing()]);
    }
    tape
}

/// A tape written by the shell's own writer reads back frame for frame.
///
/// Byte for byte and bit for bit: the reports as the pad library left them, the `dt` each frame was
/// handed, and the name the session was recorded under. The three things that would quietly survive a
/// weaker check are each asserted on a frame that has them — a `dt` that is not the 30 fps clamp, an
/// analogue strength that is neither zero nor full, and a stick that is off centre in both axes.
#[test]
fn a_tape_the_shell_wrote_reads_back_frame_for_frame() {
    let written = shell_session(300);
    assert_eq!(written.len(), 300, "one entry per simulation pass");
    assert!(!written.is_empty());

    let text = written.to_text();
    let back = PadTape::parse(&text).expect("the shell's own writer");
    assert_eq!(back.len(), written.len(), "a frame was dropped or invented");
    assert_eq!(back.name, written.tape.name, "what the session was");
    for (i, (read, wrote)) in back.frames.iter().zip(&written.tape.frames).enumerate() {
        assert_eq!(read, wrote, "frame {i}");
    }
    assert_eq!(back, written.tape);
    assert_eq!(back.to_text(), text, "and writing it again gives the same bytes");

    // Not vacuously: the frames differ from each other, and the three lossy things survived.
    assert_eq!(back.frames[0].dt(), F::ZERO, "the engine's first frame measures nothing");
    assert_eq!(back.frames[97].dt().to_bits(), F(0.041532).to_bits(), "a long frame, exactly");
    assert_eq!(back.frames[3].dt().to_bits(), F(0.03283332).to_bits(), "and an averaged one");
    assert_eq!(back.frames[200].reports[0].axis(2), written.tape.frames[200].reports[0].axis(2));
    assert_ne!(back.frames[200].stick(0), (0, 0), "the stick was off centre");
    let pressure = CONTROL_MAP[Control::Cross.index()].pressure as usize;
    let held = back.frames[200].reports[0].0[pressure];
    assert!(held > 0 && held < 0xff, "a throttle part way down, which a mask cannot hold: {held}");
    assert_eq!(back.frames[200].held_mask(0), Control::Cross.mask());
    assert_eq!(back.frames[0].held_mask(0), 0, "and nothing held before it was picked up");
}

/// The playback head hands a tape out in order, one frame per call, exactly once each.
///
/// This is the whole of what a live loop needs that [`replay::play`] does not give it: a frame at a
/// time, and a way to know the ride is over.
#[test]
fn a_playback_head_hands_out_every_frame_once() {
    let tape = shell_session(5).tape;
    let mut head = Head::new(tape.clone());
    assert_eq!(head.len(), 5);
    assert!(!head.done(), "a tape with frames in it is not over");
    assert_eq!(head.at(), 0);

    let mut out = Vec::new();
    while let Some(frame) = head.next() {
        out.push(frame);
    }
    assert_eq!(out, tape.frames, "in order, and all of them");
    assert_eq!(head.at(), 5);
    assert!(head.done(), "the loop's cue to stop");
    assert_eq!(head.next(), None, "and it stays stopped");

    // An empty tape is over before it starts, rather than hanging a loop that waits for a frame.
    let mut empty = Head::new(PadTape::new("nothing"));
    assert!(empty.done() && empty.is_empty() && empty.next().is_none());
}

/// A played tape produces the same state hash twice.
///
/// Both the pad and the frame length come off the tape, and nothing else in a playback reaches the
/// simulation, so riding one is a function of the file: same tape, same run, every tick of it. Two
/// rides are compared tick by tick rather than only at the end, so a pair that diverged in the middle
/// and came back could not pass.
#[test]
fn a_played_tape_is_the_same_run_every_time() {
    let tape = shell_session(240).tape;

    let hashes = |tape: &PadTape| -> Vec<u64> {
        let mut run = Playback::new(11);
        let mut head = Head::new(tape.clone());
        let mut out = Vec::new();
        while let Some(frame) = head.next() {
            run.advance(&frame);
            out.push(run.hash());
        }
        out
    };

    let first = hashes(&tape);
    assert_eq!(first.len(), 240);
    assert_eq!(first, hashes(&tape), "tick for tick, twice");
    assert_eq!(
        *first.last().unwrap(),
        replay::play(&tape, 11).hash(),
        "and a head-driven ride is the whole-tape call, frame for frame",
    );

    // Through the file, which is the case that matters: somebody else's copy of the tape.
    let copy = PadTape::parse(&tape.to_text()).expect("the writer's own file");
    assert_eq!(hashes(&copy), first, "a tape off disc rides the same as the tape in hand");

    // Not vacuously: the hash moves while the run goes on, and one different frame changes it.
    assert!(first.iter().collect::<std::collections::BTreeSet<_>>().len() > 100);
    let mut nudged = tape.clone();
    nudged.frames[120] = PadFrame::new(nudged.frames[120].dt(), [Report::idle(), Report::missing()]);
    assert_ne!(hashes(&nudged), first, "a tape with one frame changed is a different ride");
}

/// Riding a tape while recording one writes the tape that was ridden.
///
/// The shell's `--play t --record t2` case, which is the check that its two halves agree: a recorder
/// fed from inside the simulation pass sees exactly the `dt` and the reports the head handed out, so
/// `t2` is `t` — byte for byte as a file, and frame for frame as a tape. If the playback took its
/// frame length from the wall clock instead of the tape, this is the test that would fail.
#[test]
fn a_tape_ridden_and_recorded_at_once_is_the_tape_that_was_ridden() {
    let ridden = shell_session(120).tape;

    // The shell's loop, with the window, the drawing and the clock taken out: pull a frame, hand its
    // own dt and reports to the simulation, and write down what the simulation was handed.
    let mut again = Recorder::new(ridden.name.clone());
    let mut run = Playback::new(3);
    let mut head = Head::new(ridden.clone());
    while let Some(frame) = head.next() {
        let dt = frame.dt();
        again.frame(dt, &frame.reports);
        run.advance(&PadFrame::new(dt, frame.reports));
    }

    assert_eq!(again.tape, ridden, "frame for frame");
    assert_eq!(again.to_text(), ridden.to_text(), "and byte for byte as a file");
    assert_eq!(run.tick(), 120, "one tick per frame of the tape");
    assert_eq!(run.hash(), replay::play(&ridden, 3).hash());
}

// ---------------------------------------------------------------------------------------------------
// S32 — the second acceptance run: recording2, the human's second minute on Free09
// ---------------------------------------------------------------------------------------------------
//
// `recording2.p2m2` is a second human minute on the same ground and the same bike as the first (C6),
// played back on the original and captured with `watch/acceptance.toml`'s columns and more. It gets
// what `the_acceptance_run` has: a fixture out of the capture, seeded from its first row by
// [`acceptance_start`], ridden over the world [`recorded_on`] finds (S28), and a ratchet of its own.
//
// The difference is where it stops. S14's rule is that the tick-exact window ends at the first wreck or
// contact with an opponent: past it the player's own path draws on the rand() stream the area's AI
// riders draw from every tick, and the port has no opponents to keep that stream in step. The first
// recording happens never to reach either. This one wrecks at tick 641, a nose-dive and not a contact,
// and so the fixture is cut at 640 and the ratchet is over those ticks. The tick is read off the
// capture's own `wreck_reason` column, not assumed ([`second_window`]), and the AI bikes are measured
// as never coming near the human's before it.

/// The capture's file name in [`acceptance_dirs`] — C6's playback of `recording2.p2m2`.
const SECOND_ACCEPTANCE: &str = "recording2-freeride";

/// **The last tick of recording2 the port is known to agree with the original on.** The second run's
/// own ratchet, the same contract as [`ACCEPTANCE_AGREES_THROUGH`]: reaching further prints a line
/// asking for it to be raised, falling short of it fails. 0 is the start state [`acceptance_start`]
/// seeds from row 0.
const SECOND_ACCEPTANCE_AGREES_THROUGH: u64 = 0;

/// The last tick of recording2's tick-exact window: the tick before its first wreck. Measured off the
/// capture by [`second_window`] and asserted against it, so a different capture says so.
const SECOND_WINDOW_ENDS: u64 = 640;

/// recording2's first wreck: the tick `wreck_reason` first reads nonzero, and the reason — 8, the
/// nose-dive ([`game_sim::air::Wreck::NoseDive`]'s id). Not 7, the contact with another rider.
const SECOND_FIRST_WRECK: (u64, u32) = (641, 8);

/// The wreck id the original writes for a contact with another rider (`Wreck::Joust`).
const CONTACT: u32 = 7;

/// The second acceptance capture, from the first of [`acceptance_dirs`] that holds it — or `None`,
/// with where it comes from printed.
fn second_acceptance_capture() -> Option<(PathBuf, Capture)> {
    for dir in acceptance_dirs() {
        let path = dir.join(format!("{SECOND_ACCEPTANCE}.csv"));
        if let Ok(text) = std::fs::read_to_string(&path) {
            let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            return Some((path, cap));
        }
    }
    println!(
        "no {SECOND_ACCEPTANCE}.csv in {}: the second acceptance run is C6's playback of \
         context/recordings/recording2.p2m2 (plan/HUMAN.md has the playback recipe) captured with \
         watch/acceptance.toml, so there is no second verdict in this worktree",
        acceptance_dirs()[0].display(),
    );
    None
}

/// **Where the tick-exact window ends, read off the capture**: the last tick before the first row
/// whose `wreck_reason` is nonzero, with that tick and its reason. S14's rule closes the window at the
/// first wreck *or* contact; a contact is a wreck too (reason [`CONTACT`]), so the first nonzero
/// reason is whichever of the two comes first. A capture with no wreck in it is a window to its end.
fn second_window(cap: &Capture) -> Result<(u64, Option<(u64, u32)>), String> {
    let ticks = ticks_of(cap)?;
    let at = column(cap, "wreck_reason")?;
    for (row, &tick) in ticks.iter().enumerate() {
        let reason: u32 = cell(cap, row, at)
            .parse()
            .map_err(|e| format!("row {row}: wreck_reason '{}': {e}", cell(cap, row, at)))?;
        if reason != 0 {
            if tick == 0 {
                return Err("the capture starts mid-wreck: there is no window".into());
            }
            return Ok((tick - 1, Some((tick, reason))));
        }
    }
    Ok((*ticks.last().ok_or("an empty capture")?, None))
}

/// The ticks on which a riding bike's eight lanes of [`BIKE`] are exactly the previous row's: a row
/// read while the tick was still being simulated, carrying the last tick's bike under this tick's
/// counter. A bike with any velocity moves every tick, so on a clean capture there are none. This is
/// the detector S32 measured the five captures of recording2 with; it is a lower bound — a late row
/// whose bike did change between the two reads is not caught — and a capture it flags is not one a
/// tick-exact ratchet can be taken against.
fn repeated_bike_rows(cap: &Capture) -> Result<Vec<u64>, String> {
    let ticks = ticks_of(cap)?;
    let lanes: Vec<usize> = BIKE.iter().map(|(c, _)| column(cap, c)).collect::<Result<_, _>>()?;
    let wrecked = column(cap, "wreck_reason")?;
    let riding = |row: usize| {
        cell(cap, row, wrecked) == "0"
            && lanes[4..7].iter().any(|&l| cell(cap, row, l).parse::<f32>().is_ok_and(|v| v != 0.0))
    };
    Ok((1..cap.rows.len())
        .filter(|&r| riding(r) && riding(r - 1))
        .filter(|&r| lanes.iter().all(|&l| cell(cap, r, l) == cell(cap, r - 1, l)))
        .map(|r| ticks[r])
        .collect())
}

/// The capture's rows through tick `end` — the window, as a capture of its own. Rows are one per tick
/// from tick 0 ([`ticks_of`] refuses anything else), so that is the first `end + 1` of them. Both the
/// fixture and the world are taken from this and not from the whole minute: past the window the bike
/// wrecks three times, and one of those rows (a body origin 2.43 units under Free09's dirt) is enough
/// for [`recorded_on`] to rule every area out.
fn through_tick(cap: &Capture, end: u64) -> Capture {
    let n = (end as usize + 1).min(cap.rows.len());
    Capture {
        columns: cap.columns.clone(),
        frames: cap.frames[..n].to_vec(),
        rows: cap.rows[..n].to_vec(),
    }
}

/// **The second acceptance run** — recording2 replayed over Free09 on the Race 125, ticks 0 to
/// [`SECOND_WINDOW_ENDS`], against [`SECOND_ACCEPTANCE_AGREES_THROUGH`].
///
/// The same verdict `the_acceptance_run` prints ([`acceptance_verdict_against`]), on every `./kit
/// check` in a worktree that has the capture and the disc, and the same skip with a line where either
/// is missing. A capture with repeated bike rows ([`repeated_bike_rows`]) is refused with a line rather
/// than ridden: its late rows would be divergences of the recorder, not of the port.
///
/// ```sh
/// cargo test -p game-sim --test replay -- --nocapture the_second_acceptance_run
/// ```
#[test]
fn the_second_acceptance_run() {
    let Some((capture, cap)) = second_acceptance_capture() else { return };
    println!(
        "{}: {} rows x {} columns, frames {}..{}",
        capture.display(),
        cap.frames.len(),
        cap.columns.len(),
        cap.frames[0],
        cap.frames[cap.frames.len() - 1],
    );
    let late = repeated_bike_rows(&cap).unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
    if !late.is_empty() {
        println!(
            "  — {} of its rows repeat the previous row's bike while it is moving (ticks {:?}…): \
             they were read mid-tick, so this capture is not one to take a tick-exact verdict \
             against. Re-capture it with the recorder waking on the pad stamp, as C6's was",
            late.len(),
            &late[..late.len().min(8)],
        );
        return;
    }
    let (end, first) = second_window(&cap).unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
    match first {
        Some((tick, reason)) => println!(
            "the window: ticks 0..={end}, closed by wreck_reason {reason} at tick {tick}{}",
            if reason == CONTACT { " (a contact with another rider)" } else { "" },
        ),
        None => println!("the window: ticks 0..={end}, the whole capture — it never wrecks"),
    }
    let window = through_tick(&cap, end);
    let taken = in_the_world_of(&window, |world, named, bike| {
        let f = acceptance_fixture_on(&window, bike)
            .unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
        let f = Fixture {
            name: f.name.replacen(ACCEPTANCE, SECOND_ACCEPTANCE, 1),
            world: Some(named.clone()),
            ..f
        };
        println!(
            "{}: {} frames, {} checks, seed 0x{:08x}",
            f.name,
            f.inputs.len(),
            f.checks(),
            f.seed,
        );
        assert!(
            f.inputs.len() >= 30 * 20,
            "{} frames of window is not an acceptance run",
            f.inputs.len(),
        );
        acceptance_verdict_against(
            &f,
            world,
            SECOND_ACCEPTANCE_AGREES_THROUGH,
            "SECOND_ACCEPTANCE_AGREES_THROUGH",
        );
    });
    if taken.is_none() {
        println!(
            "— so there is no second acceptance verdict in this worktree: either the disc (MX.PSR) \
             is not here to ride it over or none of its areas fits the window's rows (the lines \
             above say which), and the ratchet is unchecked, not met",
        );
    }
}

/// **recording2's window ends at tick 640, at a nose-dive and not a contact** — found in the capture.
///
/// The first nonzero `wreck_reason` is at tick 641 and it is 8; no row of the whole capture reads
/// [`CONTACT`]. And where S32's second capture of the same playback is in the worktree
/// (`context/capture/s32/recording2-opponents.csv`, the acceptance columns plus the position of every
/// actor in the area's list), the three AI bikes are measured never to come within
/// [`OPPONENTS_KEEP_OFF`] of the human's through the window: they ride off the start line the other
/// way, 19 units away at tick 0 and hundreds by the wreck. So nothing in the window is a contact the
/// wreck column could have missed.
#[test]
fn the_second_recordings_window_ends_at_its_first_wreck_which_is_not_a_contact() {
    let Some((capture, cap)) = second_acceptance_capture() else { return };
    let (end, first) = second_window(&cap).unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
    assert_eq!(
        (end, first),
        (SECOND_WINDOW_ENDS, Some(SECOND_FIRST_WRECK)),
        "{}: the window is not where S32 measured it",
        capture.display(),
    );
    let at = column(&cap, "wreck_reason").expect("the spec's own wreck column");
    let contacts = (0..cap.rows.len()).filter(|&r| cell(&cap, r, at) == CONTACT.to_string()).count();
    assert_eq!(contacts, 0, "{}: a contact with another rider is in this capture", capture.display());

    let opponents = private_dir("capture").join("s32").join("recording2-opponents.csv");
    let Ok(text) = std::fs::read_to_string(&opponents) else {
        println!(
            "{} is not in this worktree: the opponents' distance is unchecked here",
            opponents.display(),
        );
        return;
    };
    let them = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", opponents.display()));
    let counter = |c: &Capture, r: usize| -> u64 {
        cell(c, r, column(c, "frame_counter").expect("a counter column")).parse().expect("a number")
    };
    let start = counter(&cap, 0);
    let (nearest, bikes) = nearest_opponent(&them, |r| counter(&them, r).checked_sub(start), end)
        .unwrap_or_else(|e| panic!("{}: {e}", opponents.display()));
    println!(
        "{} AI bikes; the nearest any comes to the human's through tick {end} is {:.2} units, at \
         tick {}",
        bikes, nearest.0, nearest.1,
    );
    assert_eq!(bikes, 3, "Free09 puts three AI riders in the area (S14)");
    assert!(
        nearest.0 > OPPONENTS_KEEP_OFF,
        "an AI bike comes within {:.2} units of the human's at tick {}: a contact the wreck column \
         may not show",
        nearest.0,
        nearest.1,
    );
}

/// How far apart two bikes' origins have to stay for S32 to call it no contact: twice a Race 125's
/// length or so. The measured nearest approach is 18.97, at tick 0.
const OPPONENTS_KEEP_OFF: f64 = 12.0;

/// The nearest any AI bike (`a{n}_type` 4, `a{n}_src` 2) comes to the human's (`a{n}_src` 1) through
/// tick `end`, as (distance, tick), and how many AI bikes there are. `tick` maps a row to the
/// acceptance capture's tick, `None` for a row before it.
fn nearest_opponent(
    cap: &Capture,
    tick: impl Fn(usize) -> Option<u64>,
    end: u64,
) -> Result<((f64, u64), usize), String> {
    let actors: Vec<usize> = (0..)
        .take_while(|n| column(cap, &format!("a{n}_type")).is_ok())
        .collect();
    let int = |r: usize, name: String| -> Result<u32, String> {
        cell(cap, r, column(cap, &name)?).parse().map_err(|e| format!("{name}: {e}"))
    };
    let pos = |r: usize, n: usize| -> Result<[f64; 3], String> {
        let mut p = [0.0; 3];
        for (k, c) in ["x", "y", "z"].iter().enumerate() {
            let name = format!("a{n}_pos.{c}");
            p[k] = cell(cap, r, column(cap, &name)?).parse().map_err(|e| format!("{name}: {e}"))?;
        }
        Ok(p)
    };
    let mut nearest: Option<(f64, u64)> = None;
    let mut bikes = 0;
    for r in 0..cap.rows.len() {
        let Some(t) = tick(r) else { continue };
        if t > end {
            break;
        }
        let mut human = None;
        let mut ai = Vec::new();
        for &n in &actors {
            // The list is shorter than the spec's sixteen entries: the chain past its end reads
            // nothing, and an empty cell is no actor.
            if cell(cap, r, column(cap, &format!("a{n}_type"))?).is_empty()
                || int(r, format!("a{n}_type"))? != 4
            {
                continue;
            }
            match int(r, format!("a{n}_src"))? {
                1 => human = Some(pos(r, n)?),
                2 => ai.push(pos(r, n)?),
                _ => {}
            }
        }
        let human = human.ok_or_else(|| format!("row {r}: no human bike in the list"))?;
        bikes = bikes.max(ai.len());
        for p in ai {
            let d = (0..3).map(|k| (p[k] - human[k]).powi(2)).sum::<f64>().sqrt();
            if nearest.is_none_or(|(best, _)| d < best) {
                nearest = Some((d, t));
            }
        }
    }
    Ok((nearest.ok_or("no row of the opponents' capture is in the window")?, bikes))
}


// ---------------------------------------------------------------------------------------------------
// S40 — the biplane acceptance run: C7's minute of the human flying the plane over Free06
// ---------------------------------------------------------------------------------------------------
//
// `biplane.p2m2` is the human in the freeride biplane: two takeoffs, two crashes and a respawn, played
// back on the original and captured with `watch/biplane.toml` (C7). The plane is a ride of its own
// ([`game_sim::flight::Flight`] in [`Sim::plane`]), so this run is the bike's acceptance run with the
// plane's lanes in place of the bike's: a fixture out of the capture, seeded from its first row, ridden
// over the ground it was flown over, against a ratchet of its own.
//
// S41 left the ratchet at 0 with the frame proved. The capture's first row is one plane
// frame stale ([`past_the_first_row`]), so the run starts on the second, seeded off C7's save state
// stepped there ([`off_the_save_state`], [`onto_the_row`]), over the original's own ground query
// ([`TheOriginalsGround`]). Over that ground the port's [`Flight::frame`](game_sim::flight::Flight::frame)
// is the game's `dynamics_world_step` word for word for a second
// ([`the_plane_flies_frame_for_frame_like_the_original`]), yet the run parted at tick 1, a few ulp of
// `body.vel`, because the game's frame is more than its world step: the plane's collision object
// (the record at the plane's `+0x170`) searches and resolves too. S48 ported it, and over the
// original's ground with free06's terrain and meshes for the search ([`WithTheTrack`]) every lane
// agrees through tick 22. Tick 23 is the human's first press of the gas, and every lane parted there
// at once: the press, not the collision. S94 found the order: the original steps the world before
// the player's input dispatch stores the pad, so a press reaches the commands on its tick and the
// controls on the next ([`Flight::frame`](game_sim::flight::Flight::frame)). A full stick (`0xff`)
// now survives the fixture (it was clamped to `0xfe`), and two kinds of C7 row are not checked: a
// row read before the game stepped ([`unstepped_rows`]), and the pad lanes of a row read between
// the world step and the input step ([`read_before_the_input_step`]). The run then agrees through
// tick 261 and parted at 262 by one ulp of `body.rate[0]`, on the first tick the pitch loop's output
// is neither clamped nor bypassed. S104 seeded the original's world step from the port's tick-261
// flight: every input of the autopilot agreed but the `{cos, sin}` table its turn term reads, which
// the port built in single precision. Built like the game's (fdlibm in double, chopped), the run
// agrees through tick 691 and parts at 692, the plane's first crash, on `engine.power`. Over the port's own ground ([`Ground::at`], one ulp low under
// the plane) it parts at tick 1; the run prints both. P187: the original's wreck zeroes the engine's
// note, not its power; with that, both grounds agree through tick 774 and part at 775 on the body's
// position, the port's plane already back at its spawn.

/// The capture's file name in [`acceptance_dirs`] — C7's playback of `biplane.p2m2`.
const BIPLANE_ACCEPTANCE: &str = "biplane";

/// **The last tick of the biplane recording the port is known to agree with the original on.** The
/// same contract as [`ACCEPTANCE_AGREES_THROUGH`]: further prints a line asking for it to be raised,
/// short of it fails. Tick 0 is the start state: the capture's second row, over the save state stepped
/// to it. 774 is the last before the chord's respawn parts it (P187; S104 left it at 691, the first crash, on `engine.power`).
const BIPLANE_AGREES_THROUGH: u64 = 774;

/// The container of the area the plane was flown over. Not found by [`recorded_on`]: that fits a
/// *bike's* rows, and a plane's minute is mostly air. Named instead, and checked against the plane's own
/// first row, which is standing on the ground ([`over_free06`]).
const BIPLANE_TRACK: &str = "Tracks\\free06.res";

/// The plane's lanes, capture column to the port's path, with the leaf each one is. The same object
/// the four listeners read and write (C7's columns; the offsets are in `watch/biplane.toml`): the body
/// the world integrates, the controls' surfaces, the engine's outputs, the measure's angles, and the
/// wreck — and the pad the input step made of the recorded controller (I6's mapping, read by
/// [`Sim::step`] since S41).
const PLANE: [(&str, &str, Kind); 34] = [
    ("body_pos.x", "plane.body.pos[0]", Kind::Float),
    ("body_pos.y", "plane.body.pos[1]", Kind::Float),
    ("body_pos.z", "plane.body.pos[2]", Kind::Float),
    ("body_vel.x", "plane.body.vel[0]", Kind::Float),
    ("body_vel.y", "plane.body.vel[1]", Kind::Float),
    ("body_vel.z", "plane.body.vel[2]", Kind::Float),
    ("body_quat.x", "plane.body.quat[0]", Kind::Float),
    ("body_quat.y", "plane.body.quat[1]", Kind::Float),
    ("body_quat.z", "plane.body.quat[2]", Kind::Float),
    ("body_quat.w", "plane.body.quat[3]", Kind::Float),
    ("body_rate.x", "plane.body.rate[0]", Kind::Float),
    ("body_rate.y", "plane.body.rate[1]", Kind::Float),
    ("body_rate.z", "plane.body.rate[2]", Kind::Float),
    ("throttle", "plane.surfaces.throttle", Kind::Float),
    ("thr_braking", "plane.surfaces.brake", Kind::Float),
    ("pitch_cmd", "plane.surfaces.elevator", Kind::Float),
    ("elevator", "plane.surfaces.elevator_command", Kind::Float),
    ("aileron", "plane.surfaces.aileron", Kind::Float),
    ("rudder", "plane.surfaces.rudder", Kind::Float),
    ("nose_steer", "plane.surfaces.steer", Kind::Float),
    ("pitch_int", "plane.surfaces.pitch_integral", Kind::Float),
    ("eng_power", "plane.engine.power", Kind::Float),
    ("prop_rpm", "plane.engine.rpm", Kind::Float),
    ("eng_thrust", "plane.engine.thrust", Kind::Float),
    ("prop_angle", "plane.engine.prop_angle", Kind::Float),
    ("alpha", "plane.air.alpha", Kind::Float),
    ("beta", "plane.air.beta", Kind::Float),
    ("on_ground", "plane.attitude.grounded", Kind::Bool),
    ("crash", "plane.crash", Kind::Uint),
    ("thr_up", "plane.pad.forward", Kind::Float),
    ("thr_down", "plane.pad.back", Kind::Float),
    ("elev_cmd", "plane.pad.elevator", Kind::Float),
    ("ail_cmd", "plane.pad.aileron", Kind::Float),
    ("rud_cmd", "plane.pad.rudder", Kind::Float),
];

/// What a [`PLANE`] lane's cells are.
#[derive(Clone, Copy)]
enum Kind {
    Float,
    Uint,
    Bool,
}

/// The biplane capture, from the first of [`acceptance_dirs`] that holds it — or `None`, with where it
/// comes from printed.
fn biplane_capture() -> Option<(PathBuf, Capture)> {
    for dir in acceptance_dirs() {
        let path = dir.join(format!("{BIPLANE_ACCEPTANCE}.csv"));
        if let Ok(text) = std::fs::read_to_string(&path) {
            let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            return Some((path, cap));
        }
    }
    println!(
        "no {BIPLANE_ACCEPTANCE}.csv in {}: the biplane acceptance run is C7's playback of \
         context/recordings/biplane.p2m2 captured with watch/biplane.toml, so there is no biplane \
         verdict in this worktree",
        acceptance_dirs()[0].display(),
    );
    None
}

/// `Biplane\biplane.phy`, out of `machine.res` where the plane's loader reads it.
fn the_plane_the_human_flew(disc: &mut Disc) -> Option<game_sim::phy::Plane> {
    let bytes = disc.member("machine.res")?;
    let res = Res::parse(&bytes).ok()?;
    let e = by_suffix(&res, game_sim::phy::BIPLANE_PHY)
        .or_else(|| by_suffix(&res, "biplane.phy"))?;
    Some(game_sim::phy::Plane::read_bytes(&res.read(&bytes, e).ok()?))
}

/// Row `row`'s cell of `name` as a float.
fn plane_float(cap: &Capture, row: usize, name: &str) -> Result<F, String> {
    let text = cell(cap, row, column(cap, name)?);
    text.parse::<f32>().map(F).map_err(|_| format!("row {row}: {name} '{text}' is not a float"))
}

fn plane_vec(cap: &Capture, row: usize, name: &str) -> Result<Vf, String> {
    let [x, y, z, w] =
        ["x", "y", "z", "w"].map(|c| plane_float(cap, row, &format!("{name}.{c}")).map(F::to_f32));
    Ok(Vf::new(x?, y?, z?, w?))
}

/// **Where the biplane run starts**: the plane built off the disc's file ([`Flight::new`], proved
/// word for word in `tests/flight.rs`) and put in the state the capture's first row records.
///
/// Every lane [`PLANE`] watches is seeded, because tick 0 has to agree, and so is everything the
/// listeners read that the capture carries: the measure's attitude (the roll is the third Euler angle,
/// the nose's `y` is row 2 of the drawn matrix, which is where the measure copies it from), and the
/// airspeed along the nose. What it does not carry is left at the build's value and listed in the
/// journal: the engine's noise clock, the pad's last commands (the pad lanes are this tick's, and
/// [`Flight::frame`](game_sim::flight::Flight::frame) overwrites them), the crash timer's companions.
///
/// The bike is [`Sim::new`]'s, parked: a plane ride does not step it.
///
/// [`Flight::new`]: game_sim::flight::Flight::new
fn biplane_start(
    cap: &Capture,
    seed: u32,
    plane: &game_sim::phy::Plane,
) -> Result<Sim, String> {
    let at = |name: &str| plane_float(cap, 0, name);
    let mut f = game_sim::flight::Flight::new(plane);
    f.body.position = plane_vec(cap, 0, "body_pos")?;
    f.body.velocity = plane_vec(cap, 0, "body_vel")?;
    f.body.quat = plane_vec(cap, 0, "body_quat")?;
    f.body.rates = plane_vec(cap, 0, "body_rate")?;
    (f.body.last_position, f.body.last_quat) = (f.body.position, f.body.quat);
    let s = &mut f.surfaces;
    s.throttle = at("throttle")?;
    s.brake = at("thr_braking")?;
    s.elevator = at("pitch_cmd")?;
    s.elevator_command = at("elevator")?;
    s.aileron = at("aileron")?;
    s.rudder = at("rudder")?;
    s.steer = at("nose_steer")?;
    s.pitch_integral = at("pitch_int")?;
    let p = &mut f.pad;
    p.forward = at("thr_up")?;
    p.back = at("thr_down")?;
    p.elevator = at("elev_cmd")?;
    p.aileron = at("ail_cmd")?;
    p.rudder = at("rud_cmd")?;
    let e = &mut f.engine;
    e.power = at("eng_power")?;
    e.rpm = at("prop_rpm")?;
    e.thrust = at("eng_thrust")?;
    e.prop_angle = at("prop_angle")?;
    e.note = at("eng_sound")?;
    if at("prop_turn")?.to_bits() != e.prop_rpm.to_bits() {
        return Err(format!(
            "the capture's prop_turn is {:?} where the engine's constructor writes {:?}: the column \
             is not the engine object the spec says it is",
            at("prop_turn")?,
            e.prop_rpm,
        ));
    }
    let a = &mut f.attitude;
    a.flown = cell(cap, 0, column(cap, "plane_flown")?) == "1";
    a.grounded = cell(cap, 0, column(cap, "on_ground")?) == "1";
    a.nose_y = at("plane_xform.m21")?;
    a.roll = at("plane_euler.z")?;
    a.speed = at("local_vel.z")?;
    a.pitch_rate = at("body_rate.x")?;
    let air = &mut f.air;
    air.alpha = at("alpha")?;
    air.beta = at("beta")?;
    air.speed = at("local_vel.z")?;
    air.elevator = f.surfaces.elevator_command;
    air.aileron = f.surfaces.aileron;
    air.rudder = f.surfaces.rudder;
    air.rates = plane_vec(cap, 0, "local_rate")?;
    let crash = cell(cap, 0, column(cap, "crash")?);
    f.crash.reason = crash.parse().map_err(|_| format!("row 0: crash '{crash}' is not a number"))?;
    f.crash.clock = at("crash_time")?;
    let mut sim = Sim::new(seed);
    // The measure off the seeded body, so the first frame's wheels stand where the plane does; the
    // attitude and the air the capture recorded go back over the measure's own.
    let (attitude, air) = (f.attitude, f.air);
    f.remeasure(&sim.tables.ride.acos);
    f.measure.grounded = attitude.grounded;
    (f.attitude, f.air) = (attitude, air);
    sim.plane = Some(f);
    Ok(sim)
}

/// **The biplane fixture**: C7's capture as a [`Fixture`] — the recorded pad, the original's RNG
/// state, [`biplane_start`], and every [`PLANE`] lane re-ticked off the game's own counter.
///
/// Refused, with the reason, where a row is not the human flying the plane: the capture's object must
/// be a plane (`plane_type` 1), the local human's (`plane_ctl_src` 1), and flown (`plane_flown` 1) on
/// every row, or its motion is not the recorded input's doing.
fn biplane_fixture(cap: &Capture, plane: &game_sim::phy::Plane) -> Result<Fixture, String> {
    let inputs = inputs_from_capture(cap)?;
    for (name, want, what) in [
        ("plane_type", "1", "a plane (machine kind 1)"),
        ("plane_ctl_src", "1", "the local human's machine"),
        ("plane_flown", "1", "flown"),
    ] {
        let at = column(cap, name)?;
        if let Some(row) = (0..cap.rows.len()).find(|&r| cell(cap, r, at) != want) {
            return Err(format!(
                "frame {}: {name} is '{}', not {want} — the object watch/biplane.toml points at is \
                 not {what} on this row",
                cap.frames[row],
                cell(cap, row, at),
            ));
        }
    }
    let (least, most) = dt_spread(cap)?;
    let rng = column(cap, "rng_state")?;
    let state: u64 = cell(cap, 0, rng)
        .parse()
        .map_err(|_| format!("rng_state '{}' is not a number", cell(cap, 0, rng)))?;
    let ticks = ticks_of(cap)?;
    let mut watches = Vec::new();
    for (name, path, kind) in PLANE {
        let w = match kind {
            Kind::Float => cap.float(name, path),
            Kind::Uint => cap.uint(name, path),
            Kind::Bool => cap.bool(name, path),
        };
        let mut w = w.map_err(|e| e.to_string())?;
        if w.samples.len() != cap.rows.len() {
            return Err(format!(
                "column '{name}' has {} samples for {} rows: an empty cell is a pointer chain that \
                 went null, and a fixture cannot be built across one",
                w.samples.len(),
                cap.rows.len(),
            ));
        }
        for (sample, &tick) in w.samples.iter_mut().zip(&ticks) {
            sample.tick = tick;
        }
        watches.push(w);
    }
    let (unstepped, early) = (unstepped_rows(cap)?, read_before_the_input_step(cap)?);
    for (w, (_, path, _)) in watches.iter_mut().zip(PLANE) {
        let pad = path.starts_with("plane.pad.");
        w.samples.retain(|s| {
            let at = |rows: &[usize]| rows.iter().any(|&r| ticks[r] == s.tick);
            !at(&unstepped) && !(pad && at(&early))
        });
    }
    Ok(Fixture {
        name: format!(
            "{BIPLANE_ACCEPTANCE} ({} ticks of real flight, rng 0x{state:016x}, dt {least}..{most})",
            inputs.len(),
        ),
        seed: state as u32,
        start: Some(biplane_start(cap, state as u32, plane)?),
        inputs,
        watches,
        ..Default::default()
    })
}

/// The rows of C7's capture the recorder read before the game stepped the plane that tick (S94), whose
/// checks [`biplane_fixture`] drops: rows that are the row before's word for word in the body and the
/// prop while the plane flies, where a flying plane's prop turns by its `prop_turn` every frame.
/// The row after one is two frames on (the prop two turns), as [`past_the_first_row`]'s first row
/// is, so the tick the row stands for is a frame behind and checks nothing; its frame counter has
/// still risen. One in this capture: tick 149 (frame counter 11121), whose `ail_cmd` is tick 148's
/// though the pad half the game reads moved. A crash (`crash` not 0) stops the prop, so its rows are
/// not this.
fn unstepped_rows(cap: &Capture) -> Result<Vec<usize>, String> {
    let lanes = [
        "body_pos.x", "body_pos.y", "body_pos.z", "body_quat.x", "body_quat.y", "body_quat.z", "body_quat.w",
        "prop_angle",
    ]
    .map(|name| column(cap, name));
    let lanes = lanes.into_iter().collect::<Result<Vec<_>, _>>()?;
    let crash = column(cap, "crash")?;
    Ok((1..cap.rows.len())
        .filter(|&r| cell(cap, r, crash) == "0" && lanes.iter().all(|&at| cell(cap, r, at) == cell(cap, r - 1, at)))
        .collect())
}

/// Free06's world, and the plane's first row checked against it: the run is replayed over the area
/// only if the plane the capture starts with, which says it is on the ground, is standing on this
/// area's dirt. `None`, with a line, where the disc or the area is not to be had.
fn over_free06<R>(
    cap: &Capture,
    disc: &mut Disc,
    then: impl FnOnce(&mut dyn World, &Where) -> R,
) -> Option<R> {
    let mut areas = Area::all_of(disc, BIPLANE_TRACK);
    if areas.len() != 1 {
        println!("{BIPLANE_TRACK} has {} areas on this disc, not one", areas.len());
        return None;
    }
    let area = areas.pop()?;
    let terrain = Terrain::parse(&area.terrain).ok()?;
    let ground = Ground::new(&terrain, &area.scene.terrain);
    // The drawn matrix's origin is where the plane stands; the body's is its centre of mass, higher.
    let (x, y, z) = (
        plane_float(cap, 0, "plane_xform.m30").ok()?,
        plane_float(cap, 0, "plane_xform.m31").ok()?,
        plane_float(cap, 0, "plane_xform.m32").ok()?,
    );
    let Some(hit) = ground.at(x, z) else {
        println!("the plane's first row ({x:?}, {z:?}) is off {}'s field", area.named);
        return None;
    };
    let over = y - hit.y;
    println!("the plane starts {over:?} over {}'s ground at ({x:?}, {z:?})", area.named);
    assert!(
        over.abs() < BURIED,
        "the plane's first row says it is on the ground, and it is {over:?} from {}'s: this is not \
         the area it was flown over",
        area.named,
    );
    let (props, _) = area.props(disc);
    let mut place = Place { ground, props };
    Some(then(&mut place, &area.named))
}

/// C7's capture without its first row, which is one plane frame behind the rest (S41).
///
/// Every row after the first is one frame of the plane on from the row before it, and the first is
/// two: its prop angle is 2 × 0.7583 rad short of the second row's where every later pair is
/// 0.7583 apart (the prop turns a constant 217.25 rpm), its `body_pos.z` is two frames' creep short
/// where every later pair is one, and the port stepped twice from it lands on the second row's prop
/// angle and position bit for bit. Its frame counter is still one short of the second row's, so the
/// recorder read the plane before the game had stepped it on that first frame and after it on every
/// later one. A run seeded from it is a frame behind from tick 1 on; seeded from the second row, it is
/// not. The rest of the fixture (the pad off each row, the ticks off the counter) is unchanged, so the
/// pad lanes still say whether the inputs line up.
fn past_the_first_row(mut cap: Capture) -> Capture {
    cap.frames.remove(0);
    cap.rows.remove(0);
    cap
}

/// The rows of C7's capture the recorder read after the game's world step but before its input step
/// (S94), whose pad lanes [`biplane_fixture`] does not check: the player record's two stick cubes
/// (`elev_cmd`, `ail_cmd`) on the row are not what the port's poll and input step
/// ([`game_sim::flight::pad`]) make of the row's own pad (the older half, [`PadTape::from_capture`]'s)
/// but are, bit for bit, what they make of the row before's. Their motion is the frame's: the
/// controls the next frame steps with are the row's own pad, the stale cubes notwithstanding
/// (tick 157's aileron, 0x50 in both halves and tick 156's 0x4d in the cubes, is 0x50's on tick 158).
/// [`unstepped_rows`]' rows are not these.
fn read_before_the_input_step(cap: &Capture) -> Result<Vec<usize>, String> {
    let at = |name: &str| column(cap, name);
    let (pad_a, stamp_a, pad_b, stamp_b) = (at("pad_a")?, at("stamp_a")?, at("pad_b")?, at("stamp_b")?);
    let (elevator, aileron) = (at("elev_cmd")?, at("ail_cmd")?);
    let unstepped = unstepped_rows(cap)?;
    let cubes = |r: usize| -> Option<(u32, u32)> {
        let stamp = |at| cell(cap, r, at).parse::<u64>().ok();
        let older = if stamp(stamp_a)? <= stamp(stamp_b)? { pad_a } else { pad_b };
        let hex = cell(cap, r, older);
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let mut bytes = [0u8; game_sim::input::REPORT_BYTES];
        for (b, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks(2)) {
            *b = u8::from_str_radix(core::str::from_utf8(pair).ok()?, 16).ok()?;
        }
        let mut pad = game_sim::input::Pad::new(0);
        pad.connected = true;
        pad.poll(F(1.0 / 30.0), &Report::from_bytes(bytes), &mut game_sim::input::EventQueue::new());
        let p = game_sim::flight::pad(&pad);
        Some((p.elevator.to_f32().to_bits(), p.aileron.to_f32().to_bits()))
    };
    let recorded = |r: usize| -> Option<(u32, u32)> {
        let read = |at: usize| cell(cap, r, at).parse::<f32>().map(f32::to_bits).ok();
        Some((read(elevator)?, read(aileron)?))
    };
    Ok((1..cap.rows.len())
        .filter(|r| !unstepped.contains(r))
        .filter(|&r| {
            let (own, before, row) = (cubes(r), cubes(r - 1), recorded(r));
            row.is_some() && own != row && before == row
        })
        .collect())
}

/// The save state beside C7's recording: the RAM of the moment the capture's first row was read.
const BIPLANE_STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";

/// MXBiplane, the player's plane, in that state (research/oracle.txt), and the fields of it, of its
/// rigid body, its tires and its engine object that [`off_the_save_state`] reads (the offsets
/// `tests/biplane.rs` and `tests/flight.rs` prove the listeners on).
const BIPLANE_PLANE: u32 = 0x00c4_4a40;
mod plane_at {
    pub const BODY: u32 = 0x160;
    pub const ENGINE: u32 = 0x400;
    pub const TIRES: [u32; 3] = [0x408, 0x40c, 0x410];
    pub const ENGINE_CLOCK: u32 = 0x74;
    pub const ENGINE_PROP_ANGLE: u32 = 0x48;
    /// The world a tire asks its ground query of.
    pub const TIRE_WORLD: u32 = 0x134;
}

/// [`BIPLANE_STATE`]'s RAM: this worktree's `context/` first, then the one `kit.toml` names. `None`,
/// with a line, where neither holds it.
fn biplane_state() -> Option<ps2_oracle::Ee> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(BIPLANE_STATE);
    let path = if here.exists() {
        here
    } else {
        match ps2_oracle::Kit::load() {
            Ok(kit) => kit.ctx("states").join(BIPLANE_STATE),
            Err(why) => {
                println!("no kit.toml ({why}), so no {BIPLANE_STATE}: there is no biplane verdict here");
                return None;
            }
        }
    };
    match ps2_oracle::Ee::from_ram(&path) {
        Ok(ee) => Some(ee),
        Err(why) => {
            println!(
                "{why}: the biplane run starts from C7's save state, which `./kit state ram \
                 context/recordings/biplane.p2m2_SaveState.p2s` writes, so there is no biplane verdict here"
            );
            None
        }
    }
}

/// **The original's ground**: [`World::ground`] answered by the game's own `ground_query`, run in the
/// interpreter on [`BIPLANE_STATE`]'s RAM against the world the plane's tires ask (the tire's
/// world pointer), the call the original's tire probe makes with the flags it makes it with (0).
///
/// The biplane run is flown over it rather than over [`Ground`] because the port has no ground query
/// of the original's: [`Ground::at`] blends a terrain cell bilinearly, which `world.rs` marks as its
/// own arithmetic, and under C7's plane its height is one ulp low (134.99857 where the game answers
/// 134.99858), which parts the strut force, and with it the body, on the first frame. Over this
/// ground the plane's frame is the original's word for word
/// ([`the_plane_flies_frame_for_frame_like_the_original`]), so the run measures the plane and not the
/// terrain; [`the_biplane_acceptance_run`] prints the port's own ground's verdict beside it.
///
/// What it does not answer: the surface (the original keeps it in the contact record and nothing of
/// the plane's reads it, so it is 0), and the sweep (the plane's collision search is S48's).
struct TheOriginalsGround {
    ee: ps2_oracle::Ee,
    world: u32,
    query: u32,
}

impl TheOriginalsGround {
    /// Over a RAM image of its own: the queries' scratch writes stay out of the state the run is
    /// seeded from.
    fn load() -> Option<TheOriginalsGround> {
        let mut ee = biplane_state()?;
        let (Ok(gp), Ok(query)) = (ps2_oracle::addr("boot_gp"), ps2_oracle::addr("ground_query")) else {
            println!("research/oracle.txt has no boot_gp or ground_query: there is no biplane verdict here");
            return None;
        };
        ee.set(28, u64::from(gp));
        let tire = ee.u32_at(BIPLANE_PLANE + plane_at::TIRES[0]);
        let world = ee.u32_at(tire + plane_at::TIRE_WORLD);
        Some(TheOriginalsGround { ee, world, query })
    }
}

impl World for TheOriginalsGround {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let ee = &mut self.ee;
        let (point, normal, record) = (ee.scratch(0), ee.scratch(1), ee.scratch(4));
        for k in 0..4 {
            ee.set_f32(point + 4 * k, at[k as usize]);
            ee.set_f32(normal + 4 * k, 0.0);
        }
        let args = [self.world, point, normal, 0, record].map(u64::from);
        ee.call(self.query, &args, &[], 50_000_000);
        let quad = |a: u32| Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12));
        (quad(point), quad(normal))
    }

    fn solid(&mut self, _: &game_sim::collide::Bundle, _: &mut game_sim::collide::Record) -> bool {
        false
    }
}

/// [`TheOriginalsGround`] under the wheels, with a track's terrain and meshes for the collision
/// object's search (S48): the heightfield rays read the terrain as the terrain object does
/// ([`Ground::field`], the reading `tests/wreck_bike.rs` holds to the game's), and the world rays the
/// track's collision meshes ([`game_sim::flight_search`]).
struct WithTheTrack<'a> {
    wheels: &'a mut TheOriginalsGround,
    track: &'a mut dyn World,
}

impl World for WithTheTrack<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.wheels.ground(at)
    }
    fn solid(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.wheels.solid(bundle, record)
    }
    fn heightfield(&mut self) -> Option<Box<dyn game_sim::contact_search::Heightfield + '_>> {
        self.track.heightfield()
    }
    fn collision(&mut self) -> (Option<Box<dyn game_sim::contact_search::Heightfield + '_>>, Option<&Props>) {
        self.track.collision()
    }
}

/// The plane's ray probe off the disc's `biplanem.col` (S48).
fn the_planes_rays(disc: &mut Disc) -> Option<game_sim::flight_search::Rays> {
    let bytes = disc.member("machine.res")?;
    let res = Res::parse(&bytes).ok()?;
    let e = by_suffix(&res, game_sim::flight_search::BIPLANE_RAYS)?;
    let rays = game_sim::track::Rays::parse(&res.read(&bytes, e).ok()?).ok()?;
    Some(game_sim::flight_search::Rays::of(&rays))
}

/// **What the capture's first row does not carry, off the save state of the same moment** (S41):
/// the whole rigid body (its accumulators, last position and orientation, shock and twist), the three
/// tires' state (the last compression the strut opens from, the last normal the probe leans toward,
/// the contact the friction falls back on), the engine's noise clock, and the noise tables the session
/// built off the wall-clock seed.
///
/// The state *is* the first row: its body's position, velocity, orientation and rates and its
/// engine's prop angle are held to the row's word for word first, so a state from another moment is
/// refused rather than mixed in.
fn off_the_save_state(f: &mut game_sim::flight::Flight, ee: &ps2_oracle::Ee) -> Result<(), String> {
    let quad = |at: u32| Vf::new(ee.f32_at(at), ee.f32_at(at + 4), ee.f32_at(at + 8), ee.f32_at(at + 12));
    let word = |at: u32| F(ee.f32_at(at));
    let b = ee.u32_at(BIPLANE_PLANE + plane_at::BODY);
    let e = ee.u32_at(BIPLANE_PLANE + plane_at::ENGINE);
    for (what, row, state) in [
        ("body_pos", f.body.position, quad(b + 0x60)),
        ("body_vel", f.body.velocity, quad(b + 0x70)),
        ("body_quat", f.body.quat, quad(b + 0x170)),
        ("body_rate", f.body.rates, quad(b + 0x1b0)),
    ] {
        if row.to_bits() != state.to_bits() {
            return Err(format!("{what} is {state:?} in the state and {row:?} on the capture's first row: not the same moment"));
        }
    }
    let prop = word(e + plane_at::ENGINE_PROP_ANGLE);
    if prop.to_bits() != f.engine.prop_angle.to_bits() {
        return Err(format!("the prop angle is {prop:?} in the state and {:?} on the first row", f.engine.prop_angle));
    }
    let body = &mut f.body;
    body.enabled = ee.u32_at(b) != 0;
    body.inv_mass = word(b + 0x10);
    for (v, o) in [
        (&mut body.damping, 0x20),
        (&mut body.last_position, 0x50),
        (&mut body.last_force, 0x80),
        (&mut body.force, 0x90),
        (&mut body.accel, 0xa0),
        (&mut body.shock_peak, 0xb0),
        (&mut body.shock, 0xc0),
        (&mut body.step, 0xf0),
        (&mut body.inv_inertia, 0x120),
        (&mut body.ang_damping, 0x130),
        (&mut body.last_quat, 0x150),
        (&mut body.cofg, 0x180),
        (&mut body.moment, 0x190),
        (&mut body.last_moment, 0x1a0),
        (&mut body.ang_accel, 0x1c0),
        (&mut body.twist_peak, 0x1d0),
        (&mut body.twist, 0x1e0),
    ] {
        *v = quad(b + o);
    }
    for (tire, at) in f.tires.iter_mut().zip(plane_at::TIRES) {
        let t = ee.u32_at(BIPLANE_PLANE + at);
        let flag = |o: u32| ee.u32_at(t + o) != 0;
        (tire.below, tire.held, tire.touching) = (flag(0x80), flag(0x84), flag(0x88));
        (tire.compression, tire.last_compression, tire.spring_force) = (word(t + 0x8c), word(t + 0x90), word(t + 0x94));
        (tire.angle, tire.spin) = (word(t + 0x120), word(t + 0x124));
        for (v, o) in [
            (&mut tire.offset, 0x50),
            (&mut tire.bottom, 0xa0),
            (&mut tire.ground, 0xc0),
            (&mut tire.normal, 0xd0),
            (&mut tire.contact, 0xe0),
            (&mut tire.hub, 0xf0),
            (&mut tire.low, 0x100),
            (&mut tire.contact_velocity, 0x110),
            (&mut tire.last_normal, 0x140),
            (&mut tire.slip, 0x170),
            (&mut tire.friction, 0x180),
        ] {
            *v = quad(t + o);
        }
        tire.frame = [0x190, 0x1a0, 0x1b0].map(|o| quad(t + o));
    }
    f.engine.clock = word(e + plane_at::ENGINE_CLOCK);
    let table = |name: &str| ps2_oracle::addr(name).map_err(|why| format!("{name}: {why}"));
    let (perm, g1) = (table("smooth_noise_perm")?, table("smooth_noise_g1")?);
    if ee.u32_at(table("smooth_noise_unbuilt")?) != 0 {
        return Err("the noise tables are not built in the state".to_string());
    }
    f.noise = Some(SmoothNoise::from_tables(
        &std::array::from_fn(|k| ee.u32_at(perm + 4 * k as u32)),
        &std::array::from_fn(|k| F(ee.f32_at(g1 + 4 * k as u32))),
    ));
    Ok(())
}

/// **The second row's lanes over the save state stepped to it** (S41): what the capture records of the
/// plane put back over a flight the port stepped there itself, so the run's tick 0 is the row's word
/// for word and everything the row does not carry (the rigid body's accumulators, the tires, the
/// measure, the engine's clock, the noise) is the save state's, stepped.
///
/// Not the stepped flight alone: the port's two frames over the original's ground are the original's
/// world step to the bit, but the original's frame also searches the plane's collision record (S48's),
/// so the velocity it lands on is a few ulp away from the row's.
fn onto_the_row(f: &mut game_sim::flight::Flight, row: &game_sim::flight::Flight) {
    let (b, r) = (&mut f.body, &row.body);
    (b.position, b.velocity, b.quat, b.rates) = (r.position, r.velocity, r.quat, r.rates);
    (f.pad, f.surfaces, f.attitude, f.air) = (row.pad, row.surfaces, row.attitude, row.air);
    let (e, r) = (&mut f.engine, &row.engine);
    (e.power, e.rpm, e.thrust, e.prop_angle, e.note) = (r.power, r.rpm, r.thrust, r.prop_angle, r.note);
    (f.crash.reason, f.crash.clock) = (row.crash.reason, row.crash.clock);
}

/// **The biplane acceptance run** — C7's flight replayed over Free06 on the disc's own plane, against
/// [`BIPLANE_AGREES_THROUGH`].
///
/// The same verdict the bike's two runs print ([`acceptance_verdict_against`]), on every `./kit check`
/// in a worktree that has the capture and the disc, and the same skip with a line where either is
/// missing.
///
/// ```sh
/// cargo test -p game-sim --test replay -- --nocapture the_biplane_acceptance_run
/// ```
#[test]
fn the_biplane_acceptance_run() {
    let Some((capture, cap)) = biplane_capture() else { return };
    println!(
        "{}: {} rows x {} columns, frames {}..{}",
        capture.display(),
        cap.frames.len(),
        cap.columns.len(),
        cap.frames[0],
        cap.frames[cap.frames.len() - 1],
    );
    let Some(mut disc) = Disc::open() else { return };
    let Some(plane) = the_plane_the_human_flew(&mut disc) else {
        println!("no {} in this disc's machine.res: there is no plane to fly", game_sim::phy::BIPLANE_PHY);
        return;
    };
    let Some(state) = biplane_state() else { return };
    let Some(mut wheels) = TheOriginalsGround::load() else { return };
    let Some(rays) = the_planes_rays(&mut disc) else {
        println!("no {} in this disc's machine.res: there is no plane to fly", game_sim::flight_search::BIPLANE_RAYS);
        return;
    };
    let full = cap;
    let first = inputs_from_capture(&full).unwrap_or_else(|e| panic!("{}: {e}", capture.display()))[0];
    let cap = past_the_first_row(full.clone());
    let f = biplane_fixture(&cap, &plane).unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
    let taken = over_free06(&full, &mut disc, |free06, named| {
        let mut start = biplane_start(&full, f.seed, &plane).unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
        let flight = start.plane.as_mut().expect("biplane_start puts a plane in the ride");
        *flight = std::mem::take(flight).with_rays(rays.clone());
        off_the_save_state(flight, &state).unwrap_or_else(|e| panic!("{BIPLANE_STATE}: {e}"));
        // Two frames to the second row, where the fixture's tick 0 is ([`past_the_first_row`]); both
        // on the first row's pad, which is idle (nothing held, the sticks centred) on the row and on
        // the second row alike.
        for _ in 0..2 {
            start.tick_in(first, &mut WithTheTrack { wheels: &mut wheels, track: &mut *free06 });
        }
        let row = biplane_start(&cap, f.seed, &plane).unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
        onto_the_row(start.plane.as_mut().expect("still flying"), row.plane.as_ref().expect("a plane"));
        let f = Fixture { world: Some(named.clone()), start: Some(start), ..f };
        println!("{}: {} frames, {} checks, seed 0x{:08x}", f.name, f.inputs.len(), f.checks(), f.seed);
        assert!(f.inputs.len() >= 30 * 20, "{} frames of flight is not an acceptance run", f.inputs.len());
        // The port's own ground, ungated: how far the run gets with nothing of the original's in it.
        match replay::verify_in(&f, free06) {
            Ok(_) => println!("over the port's own ground ({named}) it agrees on every tick"),
            Err(d) => println!("over the port's own ground ({named}) it parts at {d}"),
        }
        println!("over the original's ground query:");
        let mut ground = WithTheTrack { wheels: &mut wheels, track: free06 };
        acceptance_verdict_against(&f, &mut ground, BIPLANE_AGREES_THROUGH, "BIPLANE_AGREES_THROUGH");
    });
    if taken.is_none() {
        println!("— so there is no biplane verdict in this worktree, and the ratchet is unchecked, not met");
    }
}

/// **The plane's frame is the original's world step** (S41): from [`BIPLANE_STATE`], a second of the
/// game's `dynamics_world_step` run in the interpreter beside the port's [`Flight::frame`]
/// (through [`Sim::tick_in`], on the capture's first, idle pad) over [`TheOriginalsGround`], the
/// body, the three tires and the engine held to the game's word for word after every frame.
///
/// [`Flight::frame`]: game_sim::flight::Flight::frame
#[test]
fn the_plane_flies_frame_for_frame_like_the_original() {
    let Some((capture, cap)) = biplane_capture() else { return };
    let Some(mut disc) = Disc::open() else { return };
    let Some(plane) = the_plane_the_human_flew(&mut disc) else { return };
    let Some(mut ee) = biplane_state() else { return };
    let Some(mut ground) = TheOriginalsGround::load() else { return };
    let (Ok(gp), Ok(step), Ok(world)) =
        (ps2_oracle::addr("boot_gp"), ps2_oracle::addr("dynamics_world_step"), ps2_oracle::addr("dynamics_world"))
    else {
        println!("research/oracle.txt has no boot_gp, dynamics_world_step or dynamics_world");
        return;
    };
    ee.set(28, u64::from(gp));
    let pad = inputs_from_capture(&cap).unwrap_or_else(|e| panic!("{}: {e}", capture.display()))[0];
    let mut sim = biplane_start(&cap, 0, &plane).unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
    off_the_save_state(sim.plane.as_mut().expect("a plane"), &ee).unwrap_or_else(|e| panic!("{BIPLANE_STATE}: {e}"));
    let b = ee.u32_at(BIPLANE_PLANE + plane_at::BODY);
    let e = ee.u32_at(BIPLANE_PLANE + plane_at::ENGINE);
    let tires = plane_at::TIRES.map(|at| ee.u32_at(BIPLANE_PLANE + at));
    for frame in 1..=30 {
        ee.call(step, &[u64::from(world)], &[0.03333], 200_000_000);
        sim.tick_in(pad, &mut ground);
        let f = sim.plane.as_ref().expect("still a plane");
        let quad = |at: u32| Vf::new(ee.f32_at(at), ee.f32_at(at + 4), ee.f32_at(at + 8), ee.f32_at(at + 12));
        let word = |at: u32| F(ee.f32_at(at));
        let mut lanes = vec![
            ("body.pos", f.body.position, quad(b + 0x60)),
            ("body.vel", f.body.velocity, quad(b + 0x70)),
            ("body.quat", f.body.quat, quad(b + 0x170)),
            ("body.rate", f.body.rates, quad(b + 0x1b0)),
        ];
        let mut words = vec![];
        for (k, (tire, t)) in f.tires.iter().zip(tires).enumerate() {
            lanes.push((["tire 0 ground", "tire 1 ground", "tire 2 ground"][k], tire.ground, quad(t + 0xc0)));
            for (what, port, o) in [
                ("compression", tire.compression, 0x8c),
                ("last compression", tire.last_compression, 0x90),
                ("spring force", tire.spring_force, 0x94),
                ("spin", tire.spin, 0x124),
            ] {
                words.push((format!("tire {k} {what}"), port, word(t + o)));
            }
        }
        words.push(("engine thrust".to_string(), f.engine.thrust, word(e + 0x6c)));
        words.push(("engine rpm".to_string(), f.engine.rpm, word(e + 0x60)));
        words.push(("engine prop angle".to_string(), f.engine.prop_angle, word(e + plane_at::ENGINE_PROP_ANGLE)));
        words.push(("engine clock".to_string(), f.engine.clock, word(e + plane_at::ENGINE_CLOCK)));
        for (what, port, game) in lanes {
            assert_eq!(port.to_bits(), game.to_bits(), "frame {frame}: {what} is {port:?} where the game's is {game:?}");
        }
        for (what, port, game) in words {
            assert_eq!(port.to_bits(), game.to_bits(), "frame {frame}: {what} is {port:?} where the game's is {game:?}");
        }
    }
    println!("30 frames of the plane, word for word with dynamics_world_step");
}

// ===================================================================================================
// S64 — a replay that starts from the original's whole wheel state
// ===================================================================================================
//
// A capture's start is the chassis and the pads. Everything the spin step and the suspension carry from
// one tick to the next — each wheel's spin, angle, slip flags, contact frame, local velocity and force,
// compression — started from the port's defaults, so no replay could agree past its first step
// whatever the tire law did. The recording's save state holds every one of those words; these read
// them into the port's wheels, and the chassis' with them.

impl SaveState {
    fn vf(&self, a: u32) -> Option<Vf> {
        let [x, y, z, w] = self.quad(a)?;
        Some(Vf::new(f32::from_bits(x), f32::from_bits(y), f32::from_bits(z), f32::from_bits(w)))
    }

    fn flag(&self, a: u32) -> Option<bool> {
        Some(self.word(a)? != 0)
    }

    /// The human's front wheel (`0`) or rear (`1`).
    fn wheel_at(&self, which: usize) -> Option<u32> {
        self.rider_at([0x7b0, 0x7b4][which])
    }

    /// **One wheel's whole state as the original held it**, into the port's [`game_sim::ride::Wheel`]:
    /// every word the spin step (`tire::spin_integrate`'s, S58) and the suspension read back on the
    /// next substep, at the offsets `spin.rs` and `wheel.rs` prove them at. What the port builds from
    /// the `.phy` — the radius, the rates, the limits, the mount — is left as built:
    /// [`the_acceptance_bike_hangs_its_wheels_where_the_original_does`] and the oracles say those agree.
    fn seed_wheel(&self, which: usize, wh: &mut game_sim::ride::Wheel) -> Option<()> {
        let a = self.wheel_at(which)?;
        let s = &mut wh.w;
        s.mount_world = self.vf(a + 0x150)?;
        s.probe_end = self.vf(a + 0x100)?;
        s.probe_end_world = self.vf(a + 0x160)?;
        s.ground_normal = self.vf(a + 0x200)?;
        s.ray_hit = self.word(a + 0xd0)? as i32;
        s.swept_contact = self.word(a + 0xd4)? as i32;
        s.terrain_wins = self.word(a + 0xd8)? as i32;
        s.contact = self.flag(a + 0xdc)?;
        s.ray_steps = self.word(a + 0xec)? as i32;
        s.compression = self.float(a + 0xf0)?;
        s.last_compression = self.float(a + 0xf4)?;
        s.spring_force = self.float(a + 0xf8)?;
        s.contact_point = self.vf(a + 0x120)?;
        s.contact_normal = self.vf(a + 0x130)?;
        s.load_point = self.vf(a + 0x140)?;
        s.contact_velocity = self.vf(a + 0x170)?;
        s.chassis_velocity = self.vf(a + 0x180)?;
        s.balancing = self.flag(a + 0x3c)?;
        s.steer_torque = self.float(a + 0x6c)?;
        s.steer_angle = self.float(a + 0x8ac)?;
        s.steer_rate = self.float(a + 0x8b0)?;
        let m = &mut wh.mv;
        m.spin = self.float(a + 0x1d4)?;
        m.contact = s.contact;
        m.contact_normal = s.contact_normal;
        m.drive_torque = self.float(a + 0x210)?;
        m.balancing = s.balancing;
        // The seven words and three quads `wheel_clear` empties, in its order (`r#move::Fresh`).
        for (k, o) in [0xd0, 0xd4, 0x1f0, 0x1f4, 0xf0, 0xf8, 0x1a0].into_iter().enumerate() {
            m.fresh.scalars[k] = self.word(a + o)?;
        }
        for (k, o) in [0x170, 0x1b0, 0x1c0].into_iter().enumerate() {
            m.fresh.vectors[k] = self.vf(a + o)?;
        }
        wh.surface = self.word(a + 0x110)? as u8;
        wh.angle = self.float(a + 0x1d0)?;
        wh.chassis_local_velocity = self.vf(a + 0x190)?;
        wh.local_velocity = self.vf(a + 0x840)?;
        wh.local_force = self.vf(a + 0x850)?;
        for r in 0..4 {
            wh.frame[r] = self.vf(a + 0x8d0 + 16 * r as u32)?;
        }
        wh.side_per_load = self.float(a + 0x234)?;
        // The fork's axis, raked on the front (S58: left straight up, the turn's front parted
        // sideways at the first tick it steered).
        wh.steer_axis = self.vf(a + 0x8c0)?;
        wh.slip = game_sim::tire::Slip {
            spinning: self.flag(a + 0xe0)?,
            locked: self.flag(a + 0xe4)?,
            sliding: self.flag(a + 0xe8)?,
        };
        wh.mv.spin_from_torque = wh.slip.spinning;
        Some(())
    }

    /// The bike's wheel ray probe (`+0x7c8`) as the image holds it (S114): its root (`+0x160`), its
    /// nodes (`+0x16c`, `0x24` each: volume, radius, two child shorts, centre, half extent) and its
    /// segments (`+0x170`, `+0x168` of them, two quadwords each). The radius is the reach, which the
    /// port takes off the half extent ([`game_sim::flight_search::reach`]), so it is not kept.
    fn wheel_probe(&self) -> Option<game_sim::flight_search::Rays> {
        let p = self.rider_at(0x7c8)?;
        let count = self.word(p + 0x168)?;
        let (nodes, segs) = (self.word(p + 0x16c)?, self.word(p + 0x170)?);
        let three = |a: u32| -> Option<[F; 3]> { Some([self.float(a)?, self.float(a + 4)?, self.float(a + 8)?]) };
        let mut rays = game_sim::flight_search::Rays { root: self.word(p + 0x160)? as u16, ..Default::default() };
        for i in 0..count.saturating_sub(1).max(1) {
            let n = nodes + 0x24 * i;
            let ch = self.word(n + 8)?;
            rays.nodes.push(game_sim::track::Node {
                volume: self.float(n)?,
                centre: three(n + 0xc)?,
                half_extent: three(n + 0x18)?,
                children: [ch as u16, (ch >> 16) as u16],
            });
        }
        for i in 0..count {
            let s = segs + 0x20 * i;
            rays.segments.push(game_sim::ray_probe::Segment { from: self.vf(s)?, to: self.vf(s + 0x10)? });
        }
        Some(rays)
    }

    /// The wheel probe's record (its `+0x6c`) as the last search left it, each contact's point,
    /// normal and fraction (`+0x8c` of them, off the arrays at `+0x98`, `+0x94` and `+0xa8`); none
    /// when the image has no probe.
    fn wheel_record(&self) -> Vec<(Vf, Vf, F)> {
        let Some(r) = self.rider_at(0x7c8).and_then(|p| self.word(p + 0x6c)) else { return Vec::new() };
        let n = self.word(r + 0x8c).unwrap_or(0);
        let at = |o: u32| self.word(r + o).unwrap_or(0);
        (0..n)
            .filter_map(|i| Some((self.vf(at(0x98) + 16 * i)?, self.vf(at(0x94) + 16 * i)?, self.float(at(0xa8) + 4 * i)?)))
            .collect()
    }

    /// Both wheels, front then rear.
    fn seed_wheels(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        self.seed_wheel(0, &mut ride.front)?;
        self.seed_wheel(1, &mut ride.rear)
    }
}

impl SaveState {
    /// **The chassis as the original held it**: the body's whole integrator state off the rider's
    /// `+0x7a4`, at the offsets `chassis.rs` documents, and the chassis node's world transform
    /// (`+0x254`, its matrix at `+0x80`) as the ride's node and origin. The inverse inertia and the
    /// limits are the `.phy`'s and are left as the port built them.
    fn seed_chassis(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        let c = self.rider_at(0x7a4)?;
        let node = self.word(c + 0x254)?;
        let ch = &mut ride.chassis;
        ch.speed = self.float(c + 0x34)?;
        ch.cofg_world = self.vf(c + 0x60)?;
        ch.accel = self.vf(c + 0x70)?;
        ch.ang_accel = self.vf(c + 0x80)?;
        ch.euler = self.vf(c + 0x90)?;
        ch.force = self.vf(c + 0x130)?;
        ch.torque = self.vf(c + 0x140)?;
        ch.impulse = self.vf(c + 0x150)?;
        ch.correction = self.vf(c + 0x160)?;
        ch.cofg = self.vf(c + 0x190)?;
        ch.quat = self.vf(c + 0x1a0)?;
        ch.awake = self.word(c + 0x1b4)? as i32;
        ch.contacts = self.word(c + 0x1e0)? as i32;
        ch.vel = self.vf(c + 0x260)?;
        ch.local_vel = self.vf(c + 0x270)?;
        ch.world_ang_vel = self.vf(c + 0x280)?;
        ch.ang_vel = self.vf(c + 0x290)?;
        for k in 0..3 {
            ch.free[k] = self.word(c + 0x2a0 + 4 * k as u32)? as i32;
            ch.ang_free[k] = self.word(c + 0x2ac + 4 * k as u32)? as i32;
        }
        let three = |at: u32| -> Option<[F; 3]> {
            Some([self.float(at)?, self.float(at + 4)?, self.float(at + 8)?])
        };
        ch.pos_hist.dx = three(c + 0x2b8)?;
        ch.pos_hist.prev = three(c + 0x2c4)?;
        ch.vel_hist.dx = three(c + 0x2d0)?;
        ch.vel_hist.prev = three(c + 0x2dc)?;
        for k in 0..4 {
            ch.quat_hist.dx[k] = self.float(c + 0x2f0 + 4 * k as u32)?;
            ch.quat_hist.prev[k] = self.float(c + 0x300 + 4 * k as u32)?;
        }
        ch.ang_hist.dx = three(c + 0x310)?;
        ch.ang_hist.prev = three(c + 0x31c)?;
        ride.node = [self.vf(node + 0x80)?, self.vf(node + 0x90)?, self.vf(node + 0xa0)?];
        ride.origin = self.vf(node + 0xb0)?;
        Some(())
    }

    /// What the engine wrote last frame (`+0xa0` torque, `+0xa4` at the wheel, `+0xac` the road's
    /// revs): outputs the next frame overwrites, seeded so tick 0 is the image word for word.
    fn seed_engine_outputs(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        let e = self.rider_at(0x7b8)?;
        ride.engine.torque = self.float(e + 0xa0)?;
        ride.engine.wheel_torque = self.float(e + 0xa4)?;
        ride.engine.road_rpm = self.float(e + 0xac)?;
        Some(())
    }

    /// The engine's own state off the rider's `+0x7b8`, at the offsets `tests/move.rs` proves
    /// `engine_update` on: the gear `+0x68` and its ratios `+0x80`/`+0x84`, the clutch's launch
    /// scale `+0x88` and lock `+0x64`, the revs `+0xa8`, and the curve's cursor `+0x90`/`+0x94`. The
    /// acceptance start leaves the recording's; S106's teleported states were saved in another gear
    /// and with the clutch slipping, and the port's engine drove a braked rear into a spin at tick 1.
    fn seed_engine_state(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        let e = self.rider_at(0x7b8)?;
        let eng = &mut ride.engine;
        eng.gear = self.word(e + 0x68)? as i32;
        eng.ratio = self.float(e + 0x80)?;
        eng.inv_ratio = self.float(e + 0x84)?;
        eng.clutch_torque_scale = self.float(e + 0x88)?;
        eng.clutch_locked = self.flag(e + 0x64)?;
        eng.rpm = self.float(e + 0xa8)?;
        eng.curve.index = self.word(e + 0x90)?;
        eng.curve = game_sim::r#move::Curve::at(eng.curve.points, eng.curve.index, self.float(e + 0x94)?);
        Some(())
    }

    /// The balance and the pitch controller's memory off the rider (`r#move::Ride`'s offsets).
    fn seed_balance(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        let r = self.rider;
        let mv = &mut ride.mv;
        mv.g_peak = self.vf(r + 0x550)?;
        mv.balance_inertia = self.float(r + 0xa4c)?;
        mv.balance_command = self.float(r + 0xa60)?;
        mv.balance_point = self.float(r + 0xa64)?;
        mv.balance_noise_time = self.float(r + 0xa6c)?;
        mv.pitch_integral = self.float(r + 0x724)?;
        mv.stoppie = self.flag(r + 0x728)?;
        Some(())
    }

    /// The air state off the rider: which wheels touch (`+0x574`
    /// either, `+0x578` both), airborne (`+0x57c`) and last frame's (`+0x580`), the hang time
    /// (`+0x4f0`), the contact patch's velocity (`+0x430`) and the crossup angle (`+0x600`). Left
    /// at its default the port starts in the air, and its first ground step frees the roll axis
    /// (`ang_free.z`) for a tick the original keeps it locked.
    fn seed_air(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        let r = self.rider;
        let air = &mut ride.air;
        air.on_ground = self.flag(r + 0x574)?;
        air.both_wheels = self.flag(r + 0x578)?;
        air.airborne = self.flag(r + 0x57c)?;
        air.was_airborne = self.flag(r + 0x580)?;
        air.hang_time = self.float(r + 0x4f0)?;
        air.contact_vel = self.vf(r + 0x430)?;
        air.crossup = self.float(r + 0x600)?;
        Some(())
    }

    /// The levers after the rate limiter (`ride::Held`), off the rider: the brake `+0x650` and its
    /// rear copy `+0x654`, the gas `+0x658`, the clutch `+0x65c`, the squared stick `+0x628`/`+0x62c`
    /// and the lean `+0x670`/`+0x674` (the input limiter's, oracle S58). A state saved with a lever
    /// held (S106's braked bikes on a slope) has it at the top already; left at its default the
    /// port's brake would walk up from zero over the first ticks.
    fn seed_levers(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        let r = self.rider;
        let h = &mut ride.held;
        h.brake = self.float(r + 0x650)?;
        h.front_brake = h.brake;
        h.rear_brake = self.float(r + 0x654)?;
        h.gas = self.float(r + 0x658)?;
        h.clutch = self.float(r + 0x65c)?;
        h.stick = (self.float(r + 0x628)?, self.float(r + 0x62c)?);
        h.lean = (self.float(r + 0x670)?, self.float(r + 0x674)?);
        Some(())
    }
}

/// S68's captures (`watch/s68-backwards.toml`, the journal's `capture/`): the recording's save state
/// at a quarter of the speed, the rider backing up (gas and brake held at a stop) and turning at full
/// lock — on free09's flat, and over the edge of the steep slope after a turn at its top.
const S68_BACKWARDS: [&str; 2] = ["flat", "slope"];

/// How long each backwards ride agrees with the original's chassis (roll and pitch within
/// [`S68_ANGLE`], speed within [`S68_SPEED`]): the flat one until a pitch gap of 0.03 rad at tick 214,
/// backing up (S68 had it on every tick; main parted there before P72 too, with the captured levers,
/// and P72's levers off the pad are the captured ones on every tick), the slope one until a pitch gap
/// of 0.033 rad at tick 259, backing up towards the edge (S68 parted at 90, in the forward turn at the
/// top; with the levers off the pad it rides that turn within the gaps).
const S68_AGREES_THROUGH: [usize; 2] = [213, 258];
const S68_ANGLE: f32 = 0.03;
const S68_SPEED: f32 = 1.0;

/// The most either game's bike rolls while it backs up in these captures (the original's 0.21 rad on
/// the slope, after its jump off the edge), with the room the slope's parted path needs.
const S68_UPRIGHT: f32 = 0.35;

/// The original never wrecks a bike backing up and turning hard into the ground: nothing in its wreck
/// test reads the direction, its ground roll command is nought below speed 0, so the reversing bike
/// stays upright, and both captures run out with no wreck reason. The port rides the same pads from
/// the same save state to the same end, the levers and the reverse off the pad too (P72): the port's
/// reverse command is the captured `+0x648` on every row.
#[test]
fn the_captured_backwards_turns_never_wreck() {
    let Some((_, acc)) = acceptance_capture() else {
        println!("skipped: no acceptance capture");
        return;
    };
    let Some(st) = SaveState::open().filter(|s| s.leads(&acc)) else {
        println!("skipped: no save state for the acceptance run");
        return;
    };
    let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
    let seed = acceptance_fixture(&acc).map(|f| f.seed).unwrap_or(0);
    let dir = private_dir("capture").join("s68-backwards");
    let mut ran = 0;
    for (case, through) in S68_BACKWARDS.into_iter().zip(S68_AGREES_THROUGH) {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{case}.csv"))) else {
            println!("skipped: no s68-backwards/{case}.csv");
            continue;
        };
        let cap = Capture::parse(&text).unwrap();
        let ticks = quarter_ticks(&cap, saved).unwrap();
        let tape = PadTape::from_capture(&cap).unwrap();
        let f = |r: usize, c: &str| -> f32 { cell(&cap, r, column(&cap, c).unwrap()).parse().unwrap() };
        for r in 0..cap.rows.len() {
            assert_eq!(f(r, "wreck_reason"), 0.0, "{case}: the original wrecks on row {r}");
        }
        // A row at tick t carries the pad and the levers the port reads on tick t + 1 (S58's PAD_LEADS),
        // and the chassis the port leaves on tick t.
        let last = *ticks.last().unwrap() as usize;
        let mut inputs = vec![None; last + 2];
        let mut levers = vec![None; last + 2];
        let mut states = vec![None; last + 2];
        for (r, &t) in ticks.iter().enumerate() {
            if r > 0 {
                inputs[t as usize + 1] = Some(tape.frames[r - 1].input());
            }
            levers[t as usize + 1] = Some(r);
            states[t as usize] = Some(r);
        }
        let mut now = inputs.iter().flatten().next().copied().unwrap();
        let inputs: Vec<Input> = inputs.into_iter().map(|i| { now = i.unwrap_or(now); now }).collect();
        let Some(agreed) = in_the_world_of(&acc, |world, _named, bike| {
            let mut sim = acceptance_start(&acc, seed, bike).unwrap();
            let ride = &mut sim.rider.ride;
            st.seed_chassis(ride).expect("the chassis");
            st.seed_wheels(ride).expect("both wheels");
            st.seed_balance(ride).expect("the balance");
            st.seed_engine_outputs(ride).expect("the engine");
            st.seed_air(ride).expect("the air state");
            let dt = F(game_sim::tick::DeltaTime::CAP_30);
            let mut lever = levers.iter().flatten().next().copied().unwrap();
            let mut agreed = None;
            let mut rolled = (0.0f32, 0.0f32);
            let (mut reverse_off, mut reverse_rows, mut reversing) = (Vec::new(), 0, 0);
            for k in 0..last {
                let tick = k + 1;
                lever = levers[tick].unwrap_or(lever);
                // The levers and the reverse are the pad's (P72): the gas and the brake held below
                // speed 5 are the reverse, in the ride's own pad read. The kit's pad presses at full
                // pressure, so the strengths the port reads are the original's switches.
                let c = sim.poll(dt, &Sim::reports(inputs[k]));
                let frame = sim.step(dt, &c, world);
                assert!(frame.wreck.is_none(), "{case}: the port wrecks on tick {tick} ({:?})", frame.wreck);
                if levers[tick] == Some(lever) {
                    let (port, orig) = (sim.rider.ride.held.reverse.0, f(lever, "reverse"));
                    if port.to_bits() != orig.to_bits() {
                        reverse_off.push((tick, port, orig));
                    }
                    reverse_rows += 1;
                    reversing += usize::from(orig != 0.0);
                }
                // A tick the recorder missed has nothing to compare. A row is read after its tick or
                // before the next one's frame, so the tick's chassis is its own row's or the next's.
                let Some(row) = states[tick] else { continue };
                let r = &sim.rider.ride;
                let e = r.chassis.euler;
                let v = r.chassis.vel;
                let speed = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt();
                let port = [e.z(), e.x(), speed.copysign(r.chassis.local_vel.z())];
                // Speeds are the chassis' lengths (`ch_speed`), signed by the way the bike goes:
                // `bike_speed` is read part way through the tick and is only its sign here.
                let orig = |row: usize| {
                    [f(row, "ch_roll"), f(row, "ch_pitch"), f(row, "ch_speed").copysign(f(row, "bike_speed"))]
                };
                let off = |o: [f32; 3]| {
                    [(port[0] - o[0]).abs() / S68_ANGLE, (port[1] - o[1]).abs() / S68_ANGLE, (port[2] - o[2]).abs() / S68_SPEED]
                        .into_iter()
                        .fold(0.0, f32::max)
                };
                let near = [Some(row), states.get(tick + 1).copied().flatten()]
                    .into_iter()
                    .flatten()
                    .map(|r| off(orig(r)))
                    .fold(f32::INFINITY, f32::min);
                if f(lever, "reverse") != 0.0 {
                    rolled = (rolled.0.max(port[0].abs()), rolled.1.max(f(row, "ch_roll").abs()));
                }
                if agreed.is_none() && near > 1.0 {
                    let o = orig(row);
                    println!(
                        "{case}: parts on tick {tick}: roll {:.4} vs {:.4}, pitch {:.4} vs {:.4}, speed {:.3} vs {:.3}",
                        port[0], o[0], port[1], o[1], port[2], o[2]
                    );
                    agreed = Some(k);
                }
            }
            println!("{case}: the port's reverse is the original's on {} of {reverse_rows} rows ({reversing} reversing): off {reverse_off:?}",
                     reverse_rows - reverse_off.len());
            assert!(reverse_off.is_empty(), "{case}: the port's reverse is not the original's on (tick, port, original) {reverse_off:?}");
            assert!(reversing > 30, "{case}: the original reverses on only {reversing} rows");
            println!("{case}: backing up, the port rolls at most {:.3}, the original {:.3}", rolled.0, rolled.1);
            assert!(rolled.0 < S68_UPRIGHT, "{case}: the reversing bike rolls {} in the port", rolled.0);
            assert!(rolled.1 < S68_UPRIGHT, "{case}: the reversing bike rolls {} in the original", rolled.1);
            agreed.unwrap_or(last)
        }) else {
            println!("skipped: no disc for free09");
            continue;
        };
        println!("{case}: no wreck in {last} ticks, agrees through tick {agreed}");
        assert!(agreed >= through, "{case} agrees through tick {agreed}, S68_AGREES_THROUGH says {through}");
        ran += 1;
    }
    println!("{ran} of {} backwards captures ridden", S68_BACKWARDS.len());
}

/// S153's captures (`watch/s68-backwards.toml`'s lanes, the journal's `capture/`): the recording's save
/// state at a quarter of the speed, the gas for one ridden second and then nothing on the pad — the bike
/// runs down free09's slope ahead of the start, up the rise beyond it, stops and rolls back down it.
const S153_IDLE: [&str; 1] = ["rise"];

/// How long each idle ride agrees with the original's chassis (S68's gaps).
const S153_AGREES_THROUGH: [usize; 1] = [699];

/// The slowest the original's idle bike rolls backwards in each capture, in feet a second (bike_speed,
/// signed), so the test knows it rode the roll-back and not only the run down.
const S153_ROLLS_BACK: [f32; 1] = [-10.0];

/// An idle bike that stops on a rise and rolls back down it does not wreck in the original: nobody is
/// on the pad, so nothing but the slope moves it, and none of the wreck test's clauses reads the way
/// the bike is going. The port rides the same pad from the same save state and must not wreck either,
/// on any tick, rolling back included.
#[test]
fn the_captured_idle_roll_back_never_wrecks() {
    let Some((_, acc)) = acceptance_capture() else {
        println!("skipped: no acceptance capture");
        return;
    };
    let Some(st) = SaveState::open().filter(|s| s.leads(&acc)) else {
        println!("skipped: no save state for the acceptance run");
        return;
    };
    let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
    let seed = acceptance_fixture(&acc).map(|f| f.seed).unwrap_or(0);
    let dir = private_dir("capture").join("s153-idle");
    let mut ran = 0;
    for ((case, through), back) in S153_IDLE.into_iter().zip(S153_AGREES_THROUGH).zip(S153_ROLLS_BACK) {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{case}.csv"))) else {
            println!("skipped: no s153-idle/{case}.csv");
            continue;
        };
        let cap = Capture::parse(&text).unwrap();
        let ticks = quarter_ticks(&cap, saved).unwrap();
        let tape = PadTape::from_capture(&cap).unwrap();
        let f = |r: usize, c: &str| -> f32 { cell(&cap, r, column(&cap, c).unwrap()).parse().unwrap() };
        for r in 0..cap.rows.len() {
            assert_eq!(f(r, "wreck_reason"), 0.0, "{case}: the original wrecks on row {r}");
        }
        let slowest = (0..cap.rows.len()).map(|r| f(r, "bike_speed")).fold(f32::INFINITY, f32::min);
        assert!(slowest <= back, "{case}: the original rolls back at only {slowest} ft/s");
        // S68's alignment: a row at tick t carries the pad the port reads on tick t + 1, and the
        // chassis the port leaves on tick t.
        let last = *ticks.last().unwrap() as usize;
        let mut inputs = vec![None; last + 2];
        let mut states = vec![None; last + 2];
        for (r, &t) in ticks.iter().enumerate() {
            if r > 0 {
                inputs[t as usize + 1] = Some(tape.frames[r - 1].input());
            }
            states[t as usize] = Some(r);
        }
        let mut now = inputs.iter().flatten().next().copied().unwrap();
        let inputs: Vec<Input> = inputs.into_iter().map(|i| { now = i.unwrap_or(now); now }).collect();
        let Some((agreed, port_back)) = in_the_world_of(&acc, |world, _named, bike| {
            let mut sim = acceptance_start(&acc, seed, bike).unwrap();
            let ride = &mut sim.rider.ride;
            st.seed_chassis(ride).expect("the chassis");
            st.seed_wheels(ride).expect("both wheels");
            st.seed_balance(ride).expect("the balance");
            st.seed_engine_outputs(ride).expect("the engine");
            st.seed_air(ride).expect("the air state");
            let dt = F(game_sim::tick::DeltaTime::CAP_30);
            let (mut agreed, mut port_back) = (None, 0.0f32);
            for k in 0..last {
                let tick = k + 1;
                let c = sim.poll(dt, &Sim::reports(inputs[k]));
                let frame = sim.step(dt, &c, world);
                let r = &sim.rider.ride;
                let e = r.chassis.euler;
                let v = r.chassis.vel;
                let speed = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt().copysign(r.chassis.local_vel.z());
                port_back = port_back.min(speed);
                assert!(
                    frame.wreck.is_none(),
                    "{case}: the port wrecks on tick {tick} ({:?}) at speed {speed:.3}, pitch {:.4}, roll {:.4}",
                    frame.wreck, e.x(), e.z()
                );
                let Some(row) = states[tick] else { continue };
                let port = [e.z(), e.x(), speed];
                let orig = |row: usize| {
                    [f(row, "ch_roll"), f(row, "ch_pitch"), f(row, "ch_speed").copysign(f(row, "bike_speed"))]
                };
                let off = |o: [f32; 3]| {
                    [(port[0] - o[0]).abs() / S68_ANGLE, (port[1] - o[1]).abs() / S68_ANGLE, (port[2] - o[2]).abs() / S68_SPEED]
                        .into_iter()
                        .fold(0.0, f32::max)
                };
                let near = [Some(row), states.get(tick + 1).copied().flatten()]
                    .into_iter()
                    .flatten()
                    .map(|r| off(orig(r)))
                    .fold(f32::INFINITY, f32::min);
                if agreed.is_none() && near > 1.0 {
                    let o = orig(row);
                    println!(
                        "{case}: parts on tick {tick}: roll {:.4} vs {:.4}, pitch {:.4} vs {:.4}, speed {:.3} vs {:.3}",
                        port[0], o[0], port[1], o[1], port[2], o[2]
                    );
                    agreed = Some(k);
                }
            }
            (agreed.unwrap_or(last), port_back)
        }) else {
            println!("skipped: no disc for free09");
            continue;
        };
        println!("{case}: no wreck in {last} ticks, agrees through tick {agreed}; rolls back to {port_back:.2} ft/s, the original {slowest:.2}");
        assert!(port_back <= back, "{case}: the port rolls back at only {port_back} ft/s");
        assert!(agreed >= through, "{case} agrees through tick {agreed}, S153_AGREES_THROUGH says {through}");
        ran += 1;
    }
    println!("{ran} of {} idle roll-back captures ridden", S153_IDLE.len());
}

/// `watch/s8-start.toml`'s capture: the recording's own save state run on with an idle pad, which is
/// the acceptance run's first 28 ticks (the human first touches the pad at frame 2803), with the
/// chassis and the wheels beside the drawn bike.
const S8_START: &str = "s8-start";

/// Its first frame is read *before* the frame's tick: row 0 is the save state itself (its `f_spin`
/// is the image's word for word), and every row after it is read after its frame. So row 0 is tick 0,
/// and a later row's tick is its frame less the save state's.
fn save_state_ticks(cap: &Capture, saved: u64) -> Result<Vec<u64>, String> {
    let first: u64 = cell(cap, 0, column(cap, "frame_counter")?)
        .parse()
        .map_err(|_| "frame_counter is not a number".to_string())?;
    if first != saved + 1 {
        return Err(format!("the capture starts at frame {first}, not the one after the save state's {saved}"));
    }
    let ticks = ticks_of(cap)?;
    Ok(ticks.iter().enumerate().map(|(row, &t)| if row == 0 { 0 } else { t + 1 }).collect())
}

/// The lanes of `watch/s8-start.toml` (and S64's `watch/s64-wheels.toml`) as the port's walk names
/// them.
fn chassis_lanes() -> Vec<(String, String)> {
    let mut lanes = Vec::new();
    fn quad(lanes: &mut Vec<(String, String)>, col: &str, path: &str, n: usize) {
        for (i, x) in ["x", "y", "z", "w"].iter().enumerate().take(n) {
            lanes.push((format!("{col}.{x}"), format!("{path}[{i}]")));
        }
    }
    quad(&mut lanes, "ch_vel", "rider.ride.chassis.vel", 3);
    quad(&mut lanes, "ch_quat", "rider.ride.chassis.quat", 4);
    quad(&mut lanes, "ch_cofg", "rider.ride.chassis.cofg_world", 3);
    quad(&mut lanes, "ch_cofg_local", "rider.ride.chassis.cofg", 3);
    quad(&mut lanes, "ch_ang_vel", "rider.ride.chassis.world_ang_vel", 3);
    quad(&mut lanes, "ch_world_ang_vel", "rider.ride.chassis.ang_vel", 3);
    quad(&mut lanes, "ch_body_vel", "rider.ride.chassis.local_vel", 3);
    for (col, wheel) in [("f", "front"), ("r", "rear")] {
        quad(&mut lanes, &format!("{col}_load_point"), &format!("rider.ride.{wheel}.w.load_point"), 3);
        quad(&mut lanes, &format!("{col}_normal"), &format!("rider.ride.{wheel}.w.ground_normal"), 3);
        quad(&mut lanes, &format!("{col}_contact_vel"), &format!("rider.ride.{wheel}.w.contact_velocity"), 3);
    }
    for (i, x) in ["0", "1", "2"].iter().enumerate() {
        lanes.push((format!("ch_xform.m3{x}"), format!("rider.ride.origin[{i}]")));
    }
    for (col, wheel) in [("f", "front"), ("r", "rear")] {
        lanes.push((format!("{col}_comp"), format!("rider.ride.{wheel}.w.compression")));
        lanes.push((format!("{col}_last_comp"), format!("rider.ride.{wheel}.w.last_compression")));
        lanes.push((format!("{col}_spring_force"), format!("rider.ride.{wheel}.w.spring_force")));
        lanes.push((format!("{col}_spin"), format!("rider.ride.{wheel}.mv.spin")));
    }
    for (col, path) in [("eng_rpm", "rpm"), ("eng_road_rpm", "road_rpm"), ("eng_torque", "torque"), ("eng_wheel_torque", "wheel_torque")] {
        lanes.push((col.into(), format!("rider.ride.engine.{path}")));
    }
    // S64's tire half, which only `s64-wheels.csv` carries.
    for (col, wheel) in [("f", "front"), ("r", "rear")] {
        quad(&mut lanes, &format!("{col}_local_vel"), &format!("rider.ride.{wheel}.local_velocity"), 3);
        quad(&mut lanes, &format!("{col}_local_force"), &format!("rider.ride.{wheel}.local_force"), 3);
        for (c, path) in [("angle", "angle"), ("side_per_load", "side_per_load"), ("drive_torque", "mv.drive_torque")] {
            lanes.push((format!("{col}_{c}"), format!("rider.ride.{wheel}.{path}")));
        }
    }
    lanes.push(("f_steer".into(), "rider.ride.front.w.steer_angle".into()));
    lanes.push(("f_steer_rate".into(), "rider.ride.front.w.steer_rate".into()));
    lanes.push(("balance_command".into(), "rider.ride.mv.balance_command".into()));
    lanes.push(("balance_point".into(), "rider.ride.mv.balance_point".into()));
    // And the chassis integrator's histories, which the walk names by lane rather than by index.
    for (col, path, n) in [("pos", "pos_hist", 3), ("vel", "vel_hist", 3), ("quat", "quat_hist", 4), ("ang", "ang_hist", 3)] {
        for x in ["x", "y", "z", "w"].iter().take(n) {
            lanes.push((format!("ch_{col}_hist.{x}"), format!("rider.ride.chassis.{path}.{x}")));
            lanes.push((format!("ch_{col}_hist_prev.{x}"), format!("rider.ride.chassis.{path}.prev.{x}")));
        }
    }
    lanes
}

/// `watch/s64-wheels.toml`'s slip flags, as the port's walk names them.
fn slip_lanes() -> Vec<(String, String)> {
    let mut lanes = Vec::new();
    for (col, wheel) in [("f", "front"), ("r", "rear")] {
        for flag in ["spinning", "locked", "sliding"] {
            lanes.push((format!("{col}_{flag}"), format!("rider.ride.{wheel}.slip.{flag}")));
        }
    }
    lanes
}

/// S64's capture: `watch/s64-wheels.toml` — s8-start's fields and the tire half of each wheel — run
/// at a quarter speed (`NominalScalar` 0.25, and `settle_ms` 240 so the rider is there), in a
/// subfolder so the readers of `context/capture/` do not take it for theirs. At full speed the
/// recorder reads a frame after its tick and misses tick 1 outright (s8-start's first row is the save
/// state, its second already tick 2); at a quarter it reads each frame before its tick, so a row at
/// counter `c` is tick `c - saved - 1` and its first row, 2775, is tick 1.
const S64_WHEELS: &str = "s64-slow/s64-wheels-quarter";

/// The ticks of [`S64_WHEELS`]'s rows, one short: a row is the tick its counter says unless the recorder
/// was late for it — the state after its frame, not before — which shows as the row after it being
/// the same state over again (2818 and 2869 in the capture this was written against, a doubled step
/// into the row and none out of it). That row is a tick later. The last row has nothing after it to
/// tell by, and is left out.
fn quarter_ticks(cap: &Capture, saved: u64) -> Result<Vec<u64>, String> {
    let counter = column(cap, "frame_counter")?;
    let vsync = column(cap, "frame").ok();
    let n = cap.rows.len();
    // The pad's stamps are the IOP's, which counts its video fields whatever the game's tick did.
    let stamp = |c: usize| cap.columns[c].starts_with("stamp_");
    let same = |r: usize| {
        (0..cap.columns.len())
            .filter(|&c| c != counter && Some(c) != vsync && !stamp(c))
            .all(|c| cell(cap, r, c) == cell(cap, r + 1, c))
    };
    (0..n.saturating_sub(1))
        .map(|r| {
            let c: u64 = cell(cap, r, counter).parse().map_err(|_| "frame_counter is not a number".to_string())?;
            let t = c.checked_sub(saved + 1).ok_or_else(|| format!("row {r} is at frame {c}, before the save state's {saved}"))?;
            Ok(if same(r) { t + 1 } else { t })
        })
        .collect()
}

/// `w`'s samples on `ticks` (and `shift` later), cut to as many rows as there are ticks.
fn retick(mut w: Watch, ticks: &[u64], shift: u64) -> Watch {
    w.samples.truncate(ticks.len());
    for (s, &t) in w.samples.iter_mut().zip(ticks) {
        s.tick = t + shift;
    }
    w
}

/// Every lane of a save-state capture ([`chassis_lanes`], [`slip_lanes`] and the chassis' speed) that
/// the capture has a column for, on `ticks`.
fn save_state_watches(cap: &Capture, ticks: &[u64]) -> Vec<Watch> {
    let mut watches = Vec::new();
    for (col, path) in chassis_lanes() {
        let Ok(w) = cap.float(&col, &path) else { continue };
        watches.push(retick(w, ticks, 0));
    }
    // The chassis' speed is measured at the top of a frame and again by the attachment walk after
    // the physics (`chassis::measure`, S140): every row's `ch_speed` is the length of its own
    // `ch_vel`, the save state's too, and so is the port's on the same tick. Before S140 the port
    // took only the top-of-frame one, and a row's speed was the port's on the tick after it.
    if let Ok(w) = cap.float("ch_speed", "rider.ride.chassis.speed") {
        watches.push(retick(w, ticks, 0));
    }
    for (col, path) in slip_lanes() {
        let Ok(w) = cap.bool(&col, &path) else { continue };
        watches.push(retick(w, ticks, 0));
    }
    watches
}

/// The tick [`the_save_state_run_starts_from_the_originals_wheels`] agrees through on every lane
/// (`u64::MAX`: every tick of the capture). Tick 1 parted at S64 and, by the time S107 Ee-stepped it, on
/// one lane only: the angular history's `y`, by an ulp, off an angular acceleration one ulp out — the
/// torque agreed word for word at every contributor of both substeps, and the inverse inertia did not
/// (its `1/18.4` is the original's general inverse, [`game_sim::math::inverse`], not a reciprocal).
/// A ratchet: parting anywhere fails.
const SAVE_STATE_AGREES_THROUGH: u64 = u64::MAX;

/// The tick the same run's bike agrees through on its position (`rider.ride.origin`, the chassis
/// transform's translation): `x` and `z` hold on every tick of the capture, `y` parts by an ulp of
/// 208 at tick 22 at S64 (agreeing through 41 once main's later commits came in), and on every tick
/// (`u64::MAX`) since S107's inverse inertia. Before S64 it parted at tick 1, by 0.00086.
const SAVE_STATE_POSITION_AGREES_THROUGH: u64 = u64::MAX;

/// **The acceptance run's first ticks, started from the save state's whole bike** — the chassis'
/// integrator, both wheels ([`SaveState::seed_wheel`]), the air state, the drivetrain and the
/// balance — and compared lane by lane against the chassis and the wheels the original moved
/// ([`S64_WHEELS`], or `watch/s8-start.toml`'s capture where that is all there is), rather than against
/// the drawn bike the acceptance capture watches.
#[test]
fn the_save_state_run_starts_from_the_originals_wheels() {
    let Some((_, acc)) = acceptance_capture() else { return };
    let Some(st) = SaveState::open().filter(|s| s.leads(&acc)) else { return };
    let dir = private_dir("capture");
    let Some((name, text)) = [S64_WHEELS, S8_START].iter().find_map(|n| {
        std::fs::read_to_string(dir.join(format!("{n}.csv"))).ok().map(|t| (*n, t))
    }) else {
        println!("no {S64_WHEELS}.csv — `watch/s64-wheels.toml` says how it is recorded");
        return;
    };
    let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
    let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
    let ticks = if name == S64_WHEELS { quarter_ticks(&cap, saved) } else { save_state_ticks(&cap, saved) };
    let ticks = ticks.unwrap_or_else(|e| panic!("{name}.csv: {e}"));
    let seed = acceptance_fixture(&acc).map(|f| f.seed).unwrap_or(0);
    in_the_world_of(&acc, |world, named, bike| {
        let mut sim = acceptance_start(&acc, seed, bike).unwrap();
        let ride = &mut sim.rider.ride;
        st.seed_chassis(ride).expect("the chassis");
        st.seed_wheels(ride).expect("both wheels");
        st.seed_balance(ride).expect("the balance");
        st.seed_engine_outputs(ride).expect("the engine");
        st.seed_air(ride).expect("the air state");
        let watches = save_state_watches(&cap, &ticks);
        let last = *ticks.last().unwrap() as usize + 1;
        let f = Fixture {
            name: format!("{name} from the save state"),
            seed,
            start: Some(sim),
            inputs: vec![Input::default(); last],
            watches,
            world: Some(named.clone()),
            ..Default::default()
        };
        let parts = |f: &Fixture, world: &mut dyn game_sim::world::World| match replay::verify_in(f, world) {
            Ok(_) => {
                println!("{}: agrees on every tick", f.name);
                None
            }
            Err(d) => {
                println!("{}: {d}", f.name);
                Some(d.tick)
            }
        };
        let whole = parts(&f, world);
        let mut bike_at = None::<u64>;
        for w in &f.watches {
            let one = Fixture { name: format!("  {}", w.path), watches: vec![w.clone()], hashes: Vec::new(), ..f.clone() };
            let t = parts(&one, world);
            if w.path.starts_with("rider.ride.origin[") {
                bike_at = match (bike_at, t) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
        }
        let ratchet = |what: &str, at: Option<u64>, through: u64, name: &str| match at {
            Some(t) if t <= through => panic!(
                "{what} parts from the original at tick {t}, at or before {name} = {through}: a regression"
            ),
            Some(t) if t > through + 1 => println!("{what} now agrees through tick {}: raise {name}", t - 1),
            None if through != u64::MAX => println!("{what} agrees on every tick: raise {name} to u64::MAX"),
            _ => {}
        };
        // The ratchets are S64's capture's: s8-start's has no tick 1 and fewer lanes.
        if name != S64_WHEELS {
            return;
        }
        ratchet("the save-state run", whole, SAVE_STATE_AGREES_THROUGH, "SAVE_STATE_AGREES_THROUGH");
        ratchet("its bike's position", bike_at, SAVE_STATE_POSITION_AGREES_THROUGH, "SAVE_STATE_POSITION_AGREES_THROUGH");
    });
}

// ===================================================================================================
// S89 — rides on free06's railway bed, from S89's own save states
// ===================================================================================================
//
// The railway is a raised ballast bed of 62 collision meshes of surface 1 (S89's first entry). These
// are two rides on it in the original, recorded with `watch/s89-rails.toml` from save states S89 made
// out of the human's biplane state on free06 (the pause menu's "Switch To Machine: Bike", then a
// teleport onto the bed): along its x≈4591 straight from standing (gas, the brake, gas, a turn), and
// straight across its z≈4607 straight at speed. Each replays here from the state's whole bike, S64's
// way, on the same bike (`race250p.phy` on `250_motoxxx`'s wheel mounts) and the same pads.

/// **The chassis' inverse inertia is the original's, word for word** (S107): the rows a ride builds
/// off its `.phy` against the three the human's save state holds at the chassis' `+0xf0` (the tensor
/// is at `+0xb0`, and nothing in a frame writes either). The old per-axis reciprocal was an ulp low on
/// `race125p`'s `1/18.4` and `+0` where these are `−0`, which was all of tick 1's parting.
#[test]
fn the_chassis_inverse_inertia_is_the_save_states() {
    let Some((_, acc)) = acceptance_capture() else { return };
    let Some(st) = SaveState::open().filter(|s| s.leads(&acc)) else { return };
    let c = st.rider_at(0x7a4).expect("the chassis");
    let ran = in_the_world_of(&acc, |_, _, bike| {
        let sim = acceptance_start(&acc, 0, bike).unwrap();
        let ours = sim.rider.ride.chassis.inv_inertia.rows.map(|r| r.0.map(f32::to_bits));
        for (r, row) in ours.iter().enumerate() {
            let theirs: [u32; 4] = core::array::from_fn(|k| st.word(c + 0xf0 + 16 * r as u32 + 4 * k as u32).unwrap());
            assert_eq!(*row, theirs, "inverse inertia row {r}: ours {row:08x?}, the state's {theirs:08x?}");
        }
        let tensor: [u32; 3] = core::array::from_fn(|k| st.word(c + 0xb0 + 20 * k as u32).unwrap());
        println!("the tensor's diagonal {tensor:08x?}; the inverse agrees word for word");
    });
    assert!(ran.is_some() || Disc::open().is_none(), "the disc is here but the world was not built");
}

/// S89's rides: `context/capture/s89/<name>.csv` and the RAM image of the state it was recorded from,
/// `context/states/s89-<name>.eeMemory.bin` (`./kit state ram` of the slot it was saved to).
const S89_RIDES: [&str; 2] = ["along", "across"];

/// The human's rider in S89's states: the fourth entry of the game manager's actor list, where the
/// acceptance recording's is the third.
const S89_RIDER: [u32; 6] = [0x34, 0x34, 0x8, 0x8, 0x8, 0x0];

/// The tick each of [`S89_RIDES`] agrees through on every lane, in that order. A ratchet like
/// [`SAVE_STATE_AGREES_THROUGH`]: parting at or before it fails, agreeing further asks to raise it.
/// S107 had both parting at their first compared tick (2). S123 Ee-stepped the along image's ground
/// step: the original's swept contacts on the bed are the wheel probe record's points as the port's
/// search finds them, word for word, in both substeps of tick 1, and so are the load points and
/// compressions. Both now part in the air, on the angular history's z (the first lane to go): along at
/// 237, four ticks after it leaves the bed's shoulder (233), by an ulp; across at 142, 24 ticks into its
/// flight off the bed (118). Up to there every lane is the original's on every tick (S124: the pads go on
/// the ticks, [`save_state_inputs`], and no longer on the rows).
const S89_AGREES_THROUGH: [u64; 2] = [236, 141];

/// The tick each wheel's slip and contact lanes agree through on each of [`S89_RIDES`] (`u64::MAX`:
/// every tick), though the chassis parts in the air ([`S89_AGREES_THROUGH`]). A ratchet: parting
/// earlier fails. Before S124 the pads went on the rows, so the gas came a tick late and the bed's
/// edge (along 106/108, across 50/53) parted on a ride already off the original's.
const S89_LANES_AGREE_THROUGH: [(&str, [u64; 2]); 12] = [
    ("rider.ride.front.slip.spinning", [u64::MAX, u64::MAX]),
    ("rider.ride.front.slip.locked", [571, u64::MAX]),
    ("rider.ride.front.slip.sliding", [u64::MAX, u64::MAX]),
    ("rider.ride.rear.slip.spinning", [u64::MAX, u64::MAX]),
    ("rider.ride.rear.slip.locked", [571, u64::MAX]),
    ("rider.ride.rear.slip.sliding", [u64::MAX, u64::MAX]),
    // Along, the bed is rows 0-107: the contact holds through the wheels dropping off it.
    ("rider.ride.front.w.ray_hit", [517, u64::MAX]),
    ("rider.ride.front.w.swept_contact", [u64::MAX, u64::MAX]),
    ("rider.ride.front.surface", [u64::MAX, u64::MAX]),
    ("rider.ride.rear.w.ray_hit", [518, u64::MAX]),
    ("rider.ride.rear.w.swept_contact", [u64::MAX, u64::MAX]),
    ("rider.ride.rear.surface", [u64::MAX, u64::MAX]),
];

/// A tick of a ride, as the original's row and the port's run each had it: the bike's speed across
/// its own heading, and per wheel (front, rear) whether the ray hit, the sweep made a contact, and
/// the surface byte the wheel holds.
#[derive(Clone, Copy, Debug)]
struct RailTick {
    side: f32,
    ray: [bool; 2],
    swept: [bool; 2],
    surface: [u8; 2],
}

impl RailTick {
    /// Both wheels on a prop and neither on the terrain: the bed, on these rides.
    fn on_the_bed(&self) -> bool {
        self.swept[0] && self.swept[1] && !self.ray[0] && !self.ray[1]
    }
}

/// The original's [`RailTick`]s, one per row.
fn rail_ticks_of(cap: &Capture) -> Vec<RailTick> {
    let col = |name: &str| column(cap, name).unwrap_or_else(|e| panic!("{e}"));
    let num = |r: usize, name: &str| -> f32 { cell(cap, r, col(name)).parse().unwrap_or(f32::NAN) };
    let flag = |r: usize, name: &str| cell(cap, r, col(name)) != "0";
    (0..cap.rows.len())
        .map(|r| {
            let v = [num(r, "ch_vel.x"), num(r, "ch_vel.y"), num(r, "ch_vel.z")];
            let x = [num(r, "bike_xform.m00"), num(r, "bike_xform.m01"), num(r, "bike_xform.m02")];
            RailTick {
                side: v[0] * x[0] + v[1] * x[1] + v[2] * x[2],
                ray: [flag(r, "f_ray_hit"), flag(r, "r_ray_hit")],
                swept: [flag(r, "f_swept"), flag(r, "r_swept")],
                surface: [num(r, "f_surface") as u8, num(r, "r_surface") as u8],
            }
        })
        .collect()
}

/// The port's [`RailTick`] after a tick.
fn rail_tick_of(sim: &Sim) -> RailTick {
    let ride = &sim.rider.ride;
    let (v, m) = (ride.chassis.vel, ride.transform());
    let x = m.0[0];
    RailTick {
        side: v.x() * x.x() + v.y() * x.y() + v.z() * x.z(),
        ray: [ride.front.w.ray_hit != 0, ride.rear.w.ray_hit != 0],
        swept: [ride.front.w.swept_contact != 0, ride.rear.w.swept_contact != 0],
        surface: [ride.front.surface, ride.rear.surface],
    }
}

/// A ray probe's every word, so that two compare bit for bit (a `-0.0` is not a `0.0`).
fn bits_of_rays(r: &game_sim::flight_search::Rays) -> (u16, Vec<(u32, [u32; 3], [u32; 3], [u16; 2])>, Vec<[[u32; 4]; 2]>) {
    let b3 = |v: [F; 3]| v.map(|x| x.0.to_bits());
    let b4 = |v: Vf| [v.x(), v.y(), v.z(), v.w()].map(f32::to_bits);
    (
        r.root,
        r.nodes.iter().map(|n| (n.volume.0.to_bits(), b3(n.centre), b3(n.half_extent), n.children)).collect(),
        r.segments.iter().map(|s| [b4(s.from), b4(s.to)]).collect(),
    )
}

/// **S89's rides on the Dallas railway bed, in the original and replayed.** What the original does on
/// the bed is asserted off its own rows: the wheels stand on it through the sweep alone, never the
/// ray, and hold the terrain's surface byte (0 or 12), never the bed's 1; and the bike does not
/// slide (under 2 ft/s across its heading, which only the rock onto the shoulder comes near). The
/// port's run from the same state and pads is then compared lane by lane, S64's way, with each
/// lane's first divergence printed and the whole run's held by [`S89_AGREES_THROUGH`].
#[test]
fn the_dallas_rail_rides_replay_from_their_save_states() {
    for (k, name) in S89_RIDES.iter().enumerate() {
        let path = private_dir("capture").join("s89").join(format!("{name}.csv"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("no {} — `watch/s89-rails.toml` says how it is recorded", path.display());
            continue;
        };
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let image = private_dir("states").join(format!("s89-{name}.eeMemory.bin"));
        let Some(st) = SaveState::open_image(&image, &S89_RIDER) else { continue };
        // Recorded at a quarter speed, so a row is read before its tick and the first is a tick or two
        // after the image's (the recorder's settle runs on): [`quarter_ticks`] numbers them.
        let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
        let first: u64 = cell(&cap, 0, column(&cap, "frame_counter").unwrap()).parse().unwrap();
        assert!(first > saved, "{}: frame {saved}, after {}'s first row {first}", image.display(), path.display());

        // The original, off its rows.
        let orig = rail_ticks_of(&cap);
        let bed: Vec<usize> = (0..orig.len()).filter(|&r| orig[r].on_the_bed()).collect();
        // Along stays on the top for 108 rows before it drifts off; across crosses it in 4 (55-58).
        let least = [100, 4][k];
        assert!(bed.len() >= least, "{name}: the original was on the bed {} rows", bed.len());
        for &r in &bed {
            for w in 0..2 {
                assert!(
                    [0, 12].contains(&orig[r].surface[w]),
                    "{name} row {r}: wheel {w} held surface {} on the bed",
                    orig[r].surface[w]
                );
            }
        }
        let slid = bed.iter().map(|&r| orig[r].side.abs()).fold(0.0, f32::max);
        println!("{name}: the original on the bed {} of {} rows, at most {slid} across its heading", bed.len(), orig.len());
        // Under 0.04 on the bed's flat top; the most (1.24, along, at 67 ft/s) is the bike rocking over
        // onto the shoulder as it drifts off the top.
        assert!(slid < 2.0, "{name}: the original slid at {slid} on the bed");

        let ticks = quarter_ticks(&cap, saved).unwrap_or_else(|e| panic!("{name}: {e}"));
        // On the ticks, not the rows: a row's pad is the one PAD_LEADS on, and the first tick rides on
        // the controls the state was saved holding (S124; S117's rule).
        let inputs = save_state_inputs(&cap, &ticks, st.held_controls()).unwrap_or_else(|e| panic!("{name}: {e}"));
        in_the_world_on(&cap, "race250p.phy", "250_motoxxx", |world, named, bike| {
            if let Some(b) = &bike {
                for (w, mount) in [b.mounts.0, b.mounts.1].into_iter().enumerate() {
                    let at = st.wheel_at(w).and_then(|a| st.vf(a + 0xa0));
                    println!("{name}: wheel {w} hangs at {mount:?}, the original's at {at:?}");
                }
                // The states ride `race250p`, not `race250a` (S107): the two differ only in their tires'
                // SideForce and SlippingSideForce curves, and the front wheel's surface-0 table in the
                // image is `race250p`'s, word for word.
                let a = st.wheel_at(0).expect("the front wheel");
                let s = &b.phy.front.surfaces[0];
                for (k, want) in [s.side_force, s.slipping_side_force, s.max_drive_force, s.max_side_force,
                                  s.max_slipping_side_force, s.skid_drive_force, s.skid_side_force,
                                  s.skid_slipping_side_force, s.slipping_side_force_round_off,
                                  s.slipping_side_force_drop_off, s.slipping_side_force_scale,
                                  s.slipping_side_force_speed_scale, s.braking_side_force_round_off,
                                  s.braking_side_force_drop_off, s.braking_side_force_scale,
                                  s.braking_side_force_speed_scale, s.side_force_round_off,
                                  s.side_force_drop_off, s.side_force_scale, s.side_force_speed_scale]
                    .into_iter()
                    .enumerate()
                {
                    let got = st.word(a + 0x238 + 0x40 * k as u32).expect("the grip table");
                    assert_eq!(got, want.0.to_bits(), "{name}: the front's surface-0 grip word {k}");
                }
            }
            let mut sim = acceptance_start(&cap, 0, bike).unwrap();
            let ride = &mut sim.rider.ride;
            st.seed_chassis(ride).expect("the chassis");
            st.seed_wheels(ride).expect("both wheels");
            st.seed_balance(ride).expect("the balance");
            st.seed_engine_outputs(ride).expect("the engine");
            st.seed_air(ride).expect("the air state");
            if let Some(d) = st.drivetrain() {
                d.seed(ride);
            }
            st.seed_levers(ride).expect("the levers");
            st.seed_engine_state(ride).expect("the engine's state");
            // The wheel probe the ride searches with is the image's, bit for bit (S114).
            let probe = st.wheel_probe().expect("the wheel probe");
            let built = game_sim::ride::wheel_rays(&ride.front.w, &ride.rear.w);
            assert_eq!(bits_of_rays(&built), bits_of_rays(&probe), "{name}: the wheel probe");
            // And what it finds where the image left the bike is what the original's last search found:
            // the same faces, the normals to the bit. The record is a substep older than the pose
            // (the search runs before the substep's integrate), so the points and fractions are near.
            let ram = st.wheel_record();
            let found = {
                let node = game_sim::rider_contact::node(&ride.transform());
                let (field, props, trees) = world.wooded();
                let bounds = field.as_ref().map(|f| {
                    let (lo, hi) = (f.min(), f.max());
                    ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
                });
                let mut search = game_sim::broad_phase::Search { least: F(30.0), extra: F(50.0), capacity: 256, ..Default::default() };
                let mut scene = game_sim::flight_search::Scene::wooded(props, trees, None, bounds, &built, &node, &mut search);
                let _ = game_sim::contact_search::Search::rays(&mut scene, true);
                scene.ray_record().contacts.clone()
            };
            assert_eq!(found.len(), ram.len(), "{name}: the wheel probe's contacts");
            for (c, (point, normal, fraction)) in found.iter().zip(&ram) {
                println!("{name}: wheel contact {:#06x} at {:?} ({:?}), the original's at {point:?} ({fraction:?})", c.segment, c.point, c.fraction);
                assert_eq!(c.normal.0.map(f32::to_bits), normal.0.map(f32::to_bits), "{name}: a wheel contact's normal");
                assert!((c.point.0[1] - point.0[1]).abs() < 0.01 && (c.fraction.0 - fraction.0).abs() < 0.001, "{name}: a wheel contact");
            }
            let mut watches = Vec::new();
            for (col, path) in chassis_lanes() {
                let Ok(w) = cap.float(&col, &path) else { continue };
                watches.push(retick(w, &ticks, 0));
            }
            for (col, path) in slip_lanes() {
                let Ok(w) = cap.bool(&col, &path) else { continue };
                watches.push(retick(w, &ticks, 0));
            }
            for (col, wheel) in [("f", "front"), ("r", "rear")] {
                if let Ok(w) = cap.int(&format!("{col}_ray_hit"), &format!("rider.ride.{wheel}.w.ray_hit")) {
                    watches.push(retick(w, &ticks, 0));
                }
                if let Ok(w) = cap.int(&format!("{col}_swept"), &format!("rider.ride.{wheel}.w.swept_contact")) {
                    watches.push(retick(w, &ticks, 0));
                }
                if let Ok(w) = cap.uint(&format!("{col}_surface"), &format!("rider.ride.{wheel}.surface")) {
                    watches.push(retick(w, &ticks, 0));
                }
            }
            let last = *ticks.last().unwrap() as usize + 1;
            let mut pads = inputs.clone();
            pads.resize(last.max(pads.len()), Input::default());
            let f = Fixture {
                name: format!("s89 {name} from its save state"),
                seed: 0,
                start: Some(sim.clone()),
                inputs: pads.clone(),
                watches,
                world: Some(named.clone()),
                ..Default::default()
            };
            let parts = |f: &Fixture, world: &mut dyn World| match replay::verify_in(f, world) {
                Ok(_) => {
                    println!("{}: agrees on every tick", f.name);
                    None
                }
                Err(d) => {
                    println!("{}: {d}", f.name);
                    Some(d.tick)
                }
            };
            let whole = parts(&f, world);
            for w in &f.watches {
                let one = Fixture { name: format!("  {}", w.path), watches: vec![w.clone()], hashes: Vec::new(), ..f.clone() };
                let through = parts(&one, world).map_or(u64::MAX, |t| t - 1);
                if let Some((_, want)) = S89_LANES_AGREE_THROUGH.iter().find(|(p, _)| *p == w.path) {
                    assert!(through >= want[k], "{name}: {} agrees through tick {through}, was {}", w.path, want[k]);
                    if through > want[k] {
                        println!("{name}: {} now agrees through tick {through}: raise S89_LANES_AGREE_THROUGH", w.path);
                    }
                }
            }
            for (path, _) in S89_LANES_AGREE_THROUGH {
                assert!(f.watches.iter().any(|w| w.path == path), "{name}: no {path} lane in the capture");
            }

            // The port's own run beside the original's rows: where each stood on the bed, and how
            // fast each went across its heading there.
            let mut port = Vec::with_capacity(last);
            for t in 1..last {
                sim.tick_in(pads[t - 1], world);
                port.push((t as u64, rail_tick_of(&sim)));
            }
            let on = port.iter().filter(|(_, p)| p.on_the_bed()).count();
            let port_slid = port.iter().filter(|(_, p)| p.swept[0] || p.swept[1]).map(|(_, p)| p.side.abs()).fold(0.0, f32::max);
            println!("{name}: the port on the bed {on} of {} ticks, at most {port_slid} across its heading on a prop", port.len());
            // Not bit-exact the whole way (it parts in the air, [`S89_AGREES_THROUGH`]), but the port rides the bed the
            // original's way: as long, through the sweep, on the terrain's surface byte, and no slide.
            assert!(on >= least, "{name}: the port was on the bed {on} ticks, the original {}", bed.len());
            for (t, p) in port.iter().filter(|(_, p)| p.on_the_bed()) {
                assert!(p.surface.iter().all(|s| [0, 12].contains(s)), "{name} tick {t}: the port held {:?} on the bed", p.surface);
            }
            assert!(port_slid < 2.0, "{name}: the port slid at {port_slid} on the bed");
            for (r, &t) in ticks.iter().enumerate().step_by(10) {
                if let Some((_, p)) = port.iter().find(|(pt, _)| *pt == t) {
                    let o = orig[r];
                    println!(
                        "{name} tick {t:4}: original side {:7.3} ray {:?} swept {:?} surface {:?} | port side {:7.3} ray {:?} swept {:?} surface {:?}",
                        o.side, o.ray, o.swept, o.surface, p.side, p.ray, p.swept, p.surface
                    );
                }
            }
            match whole {
                Some(t) if t <= S89_AGREES_THROUGH[k] => panic!(
                    "{name} parts from the original at tick {t}, at or before S89_AGREES_THROUGH[{k}] = {}: a regression",
                    S89_AGREES_THROUGH[k]
                ),
                Some(t) if t > S89_AGREES_THROUGH[k] + 1 => println!("{name} now agrees through tick {}: raise S89_AGREES_THROUGH[{k}]", t - 1),
                None => println!("{name} agrees on every tick: raise S89_AGREES_THROUGH[{k}]"),
                _ => {}
            }
        });
    }
}

// --- S67: animandpump's landings, each from a save state taken just before it ------------------------

/// S67's capture (`research/journal/2026-10-24-s67-landing-snap-2/capture/landings.toml`): the user's
/// animandpump recording played back on the original, the whole bike every tick (S64's lanes on this
/// state's chains) over frames 1939..3539, and a save state a few ticks before each landing, kept as
/// `landings-<frame>.p2s` with its RAM image beside it (`./kit state ram … --out landings-<frame>.ee.bin`).
const S67_LANDINGS: &str = "s67/landings";

/// The save states the capture took, by the frame they were asked for, and the touchdown each one is
/// before (S109's frame ranges): rear first off a 15-tick hop down a slope, the 211-tick big drop,
/// an 11-tick hop landed rolled 48°, the 58-tick nose-first drop, and a 7-tick hop.
const S67_STATES: [(u64, u64); 5] = [(2143, 2149), (2745, 2751), (3386, 3393), (3452, 3459), (3492, 3499)];

/// A second playback of the same recording over frames 1939..2798, with the save states taken on the
/// big drop's last airborne tick and on its touchdown tick: the first ground tick is then run from
/// the original's own touchdown, not from six ticks of the port's flight to it.
const S67_RUN2: &str = "s67/run2";

/// [`S67_RUN2`]'s save states, as [`S67_STATES`].
const S67_RUN2_STATES: [(u64, u64); 2] = [(2750, 2751), (2751, 2751)];

/// S115's playback of the same recording over frames 1939..3485, with the save states taken on the
/// nose-first drop's last two airborne ticks: its touchdown tick is run from the original's own flight.
const S115_NOSE: &str = "s115/landings";

/// [`S115_NOSE`]'s save states, as [`S67_STATES`].
const S115_NOSE_STATES: [(u64, u64); 2] = [(3457, 3459), (3458, 3459)];

/// Ticks past the touchdown each run is compared for.
const S67_AFTER: u64 = 12;

/// How far from the original a landing may ride from its touchdown on: degrees off any of the three
/// angles, distance between the centres of mass and between the velocities, compression, and the
/// farthest apart of the drawn node's rows and of the chase camera's two axes — the shell's two
/// readings of the ride, held to the original's own node (`ch_xform`) rather than to the port's. Not zero:
/// two playbacks of the one recording already part in the last bits, the capture's rows are read at
/// three phases of a frame, and the state is a few ticks of the port's own flight from the ground.
/// Before S67's two fixes the big drop bounced back up by a foot and more; after them the worst
/// landing here is 0.33°, 0.023 and 0.18. The nose-first drop (3452) had a bound of its own until S115,
/// at 2.69° and 2.36 in the velocity; with the fixes since it rides within 0.04°, 0.003 and 0.043.
const S67_BOUND: [f32; 6] = [0.5, 0.05, 0.3, 0.01, 0.05, 0.01];

impl SaveState {
    /// A RAM image at `at`, its rider found down whichever of the two list shapes the states in this
    /// project have (`first_recording`'s two `next` hops, animandpump's one: S109).
    fn open_either(at: &std::path::Path) -> Option<SaveState> {
        let ram = std::fs::read(at).ok()?;
        let mut state = SaveState { ram, rider: 0 };
        for hops in [&[0x34, 0x34, 0x8, 0x0][..], &[0x34, 0x34, 0x8, 0x8, 0x0][..]] {
            let mut a = state.word(Self::GAME_MANAGER)?;
            for &hop in hops {
                a = state.word(a + hop)?;
            }
            if state.word(a) == Some(2) && state.word(a + 0x20).and_then(|c| state.word(c + 0x34)) == Some(1) {
                state.rider = a;
                return Some(state);
            }
        }
        None
    }

    /// Everything [`the_save_state_run_starts_from_the_originals_wheels`] seeds, and the drivetrain.
    fn seed_ride(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        self.seed_chassis(ride)?;
        self.seed_wheels(ride)?;
        self.seed_balance(ride)?;
        self.seed_engine_outputs(ride)?;
        self.seed_air(ride)?;
        self.drivetrain()?.seed(ride);
        self.seed_held(ride)
    }

    /// What the input layer had rate-limited by this frame (`ride::Held`): the stick (`+0x628`/`+0x62c`),
    /// its fore-aft flick speed (`+0x630`), the two brakes (`+0x650`/`+0x654`) and the lean (`+0x670`/
    /// `+0x674`). The flick's sample ring and which axes the d-pad held are not read; animandpump rides
    /// the stick.
    fn seed_held(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        let r = self.rider;
        let h = &mut ride.held;
        h.stick = (self.float(r + 0x628)?, self.float(r + 0x62c)?);
        h.flick = self.float(r + 0x630)?;
        h.front_brake = self.float(r + 0x650)?;
        h.rear_brake = self.float(r + 0x654)?;
        h.lean = (self.float(r + 0x670)?, self.float(r + 0x674)?);
        Some(())
    }
}

/// Pitch, roll and yaw in degrees off a chassis quaternion, as `research/s55_landing.py` reads them off
/// a transform: row 2 forward, row 1 up, row 0 right.
fn attitude(q: Vf) -> (f32, f32, f32) {
    let m = math::quat_to_mat4(q, Vf::new(0.0, 0.0, 0.0, 1.0)).0;
    let (right, up, fwd) = (m[0], m[1], m[2]);
    let yaw = fwd.x().atan2(fwd.z()).to_degrees();
    let pitch = fwd.y().clamp(-1.0, 1.0).asin().to_degrees();
    let roll = right.y().atan2(up.y()).to_degrees();
    (pitch, roll, yaw)
}

/// One tick of a landing, the port's or the original's.
#[derive(Clone, Copy, Debug)]
struct Touch {
    quat: Vf,
    cofg: Vf,
    vel: Vf,
    spin: Vf,
    comp: (f32, f32),
    airborne: bool,
    /// Each wheel's spring force, its tire force in the wheel's frame, and whether its ray hit.
    wheels: [(f32, Vf, f32); 2],
    /// The gear, the revs, the engine's torque and the wheel's, and the rate-limited throttle.
    engine: [f32; 5],
    /// The chassis' node, rows and origin: what the shell draws the bike with (`Ride::transform`).
    drawn: [Vf; 4],
    /// The forward and up the chase camera's target block is given (`Ride::axes`): the original's
    /// writer reads them off the node's rows 2 and 1.
    aim: (Vf, Vf),
}

impl Touch {
    fn of(ride: &game_sim::ride::Ride) -> Touch {
        Touch {
            quat: ride.chassis.quat,
            cofg: ride.chassis.cofg_world,
            vel: ride.chassis.vel,
            spin: ride.chassis.world_ang_vel,
            comp: (ride.front.w.compression.0, ride.rear.w.compression.0),
            airborne: ride.air.airborne,
            wheels: [&ride.front, &ride.rear]
                .map(|wh| (wh.w.spring_force.0, wh.local_force, wh.w.ray_hit as f32)),
            engine: [
                ride.engine.gear as f32,
                ride.engine.rpm.0,
                ride.engine.torque.0,
                ride.engine.wheel_torque.0,
                ride.held.gas.0,
            ],
            drawn: ride.transform().0,
            aim: ride.axes(),
        }
    }

    fn row(cap: &Capture, row: usize) -> Result<Touch, String> {
        let f = |name: &str| -> Result<f32, String> {
            cell(cap, row, column(cap, name)?).parse::<f32>().map_err(|_| format!("row {row}: {name} is not a number"))
        };
        let v = |name: &str| -> Result<Vf, String> {
            Ok(Vf::new(f(&format!("{name}.x"))?, f(&format!("{name}.y"))?, f(&format!("{name}.z"))?, f(&format!("{name}.w"))?))
        };
        let row = |i: usize| -> Result<Vf, String> {
            let m = |j: usize| f(&format!("ch_xform.m{i}{j}"));
            Ok(Vf::new(m(0)?, m(1)?, m(2)?, m(3)?))
        };
        let drawn = [row(0)?, row(1)?, row(2)?, row(3)?];
        Ok(Touch {
            quat: v("ch_quat")?,
            cofg: v("ch_cofg")?,
            vel: v("ch_vel")?,
            spin: v("ch_ang_vel")?,
            comp: (f("f_comp")?, f("r_comp")?),
            airborne: f("airborne")? != 0.0,
            wheels: [
                (f("f_spring_force")?, v("f_local_force")?, f("f_ray_hit")?),
                (f("r_spring_force")?, v("r_local_force")?, f("r_ray_hit")?),
            ],
            engine: [f("eng_gear")?, f("eng_rpm")?, f("eng_torque")?, f("eng_wheel_torque")?, f("gas_limited")?],
            drawn,
            aim: (drawn[2], drawn[1]),
        })
    }

    /// The lanes that are not the same bits, by name.
    fn parted(&self, o: &Touch) -> Vec<&'static str> {
        let same = |a: Vf, b: Vf| (0..3).all(|i| a.0[i].to_bits() == b.0[i].to_bits());
        let mut out = Vec::new();
        if (0..4).any(|i| self.quat.0[i].to_bits() != o.quat.0[i].to_bits()) {
            out.push("quat");
        }
        for (name, a, b) in [("cofg", self.cofg, o.cofg), ("vel", self.vel, o.vel), ("spin", self.spin, o.spin)] {
            if !same(a, b) {
                out.push(name);
            }
        }
        if self.comp.0.to_bits() != o.comp.0.to_bits() || self.comp.1.to_bits() != o.comp.1.to_bits() {
            out.push("comp");
        }
        if self.airborne != o.airborne {
            out.push("airborne");
        }
        out
    }

    /// How far apart two ticks are: the largest of the three angles in degrees, the distance between
    /// the centres of mass, between the velocities, the larger compression difference, the farthest
    /// apart of the drawn node's four rows, and of the camera's two axes.
    fn gap(&self, o: &Touch) -> [f32; 6] {
        let (a, b) = (attitude(self.quat), attitude(o.quat));
        let turn = |d: f32| (d + 540.0).rem_euclid(360.0) - 180.0;
        let angle = [a.0 - b.0, a.1 - b.1, a.2 - b.2].map(|d| turn(d).abs()).into_iter().fold(0.0, f32::max);
        let apart = |u: Vf, v: Vf| (0..3).map(|i| (u.0[i] - v.0[i]).powi(2)).sum::<f32>().sqrt();
        let comp = (self.comp.0 - o.comp.0).abs().max((self.comp.1 - o.comp.1).abs());
        let drawn = (0..4).map(|i| apart(self.drawn[i], o.drawn[i])).fold(0.0, f32::max);
        let aim = apart(self.aim.0, o.aim.0).max(apart(self.aim.1, o.aim.1));
        [angle, apart(self.cofg, o.cofg), apart(self.vel, o.vel), comp, drawn, aim]
    }

    fn line(&self) -> String {
        let (p, r, y) = attitude(self.quat);
        format!(
            "{p:+6.1} {r:+6.1} {y:+7.1} | {:+6.2} {:+6.2} {:+6.2} | {:.3} {:.3} | {:+6.1} {}",
            self.spin.x(), self.spin.y(), self.spin.z(), self.comp.0, self.comp.1, self.vel.y(),
            if self.airborne { "air" } else { "gnd" },
        )
    }

    /// The wheels: spring force, tire force x/y/z, ray hit, front then rear.
    fn wheel_line(&self) -> String {
        let one = |(spring, force, hit): (f32, Vf, f32)| {
            format!("{spring:8.1} {:+8.1} {:+8.1} {:+8.1} {hit}", force.x(), force.y(), force.z())
        };
        let [gear, rpm, torque, wheel, gas] = self.engine;
        let (c, v) = (self.cofg, self.vel);
        format!("{} | {} | gear {gear} rpm {rpm:.0} torque {torque:.1} wheel {wheel:.1} gas {gas:.3} \
                 | at {:.3} {:.3} {:.3} vel {:.2} {:.2} {:.2}",
                one(self.wheels[0]), one(self.wheels[1]), c.x(), c.y(), c.z(), v.x(), v.y(), v.z())
    }
}

/// The rows of `cap` around frame `at` whose counters step one a row, as a capture of their own.
fn dense_run(cap: &Capture, at: u64) -> Capture {
    let mut start = cap.frames.iter().position(|&f| f >= at).unwrap_or(0);
    while start > 0 && cap.frames[start - 1] + 1 == cap.frames[start] {
        start -= 1;
    }
    let mut end = start + 1;
    while end < cap.frames.len() && cap.frames[end - 1] + 1 == cap.frames[end] {
        end += 1;
    }
    Capture { columns: cap.columns.clone(), frames: cap.frames[start..end].to_vec(), rows: cap.rows[start..end].to_vec() }
}

/// The capture's rows by the game's frame counter.
fn rows_by_counter(cap: &Capture) -> std::collections::BTreeMap<u64, usize> {
    let counter = column(cap, "frame_counter").unwrap();
    let mut out = std::collections::BTreeMap::new();
    for row in 0..cap.rows.len() {
        if let Ok(c) = cell(cap, row, counter).parse::<u64>() {
            out.insert(c, row);
        }
    }
    out
}

/// The class animandpump is ridden on. Not the freeride's `race125p.phy`: the state's engine holds
/// `race500p.phy`'s power curve to the bit, and that curve is the only thing the two files differ in.
/// The other 500 classes (`race500a`, `pro500p`, `pro500a`) differ from this one in their tires and
/// air control, which is why the test does not stop at the curve to tell them apart from it.
const S67_PHY: &str = "race500p.phy";

/// A bike class by its `.phy` name, out of the first bike archive that has it.
fn the_class_named(disc: &mut Disc, tables: &RiderTables, name: &str) -> Option<BikePhy> {
    for archive in disc.bike_archives() {
        let Some(bytes) = disc.member(&archive) else { continue };
        let Ok(res) = Res::parse(&bytes) else { continue };
        if let Some(phy) = by_suffix(&res, name).and_then(|e| res.read(&bytes, e).ok()) {
            return Some(BikePhy::read_bytes(&phy, &tables.ride.curve));
        }
    }
    None
}

impl SaveState {
    /// The engine's power curve as the state holds it: each point's revs and torque, as bits
    /// (`+0x6c` the point count, `+0x70`/`+0x74` the two tables).
    fn power_curve(&self) -> Option<Vec<(u32, u32)>> {
        let e = self.rider_at(0x7b8)?;
        let (n, rpm, torque) = (self.word(e + 0x6c)?, self.word(e + 0x70)?, self.word(e + 0x74)?);
        (0..n).map(|k| Some((self.word(rpm + 4 * k)?, self.word(torque + 4 * k)?))).collect()
    }
}

/// **Each of animandpump's landings, ridden by the port from the original's own save state with the
/// original's own pad**, and compared tick by tick through the touchdown with the chassis the original
/// moved: its orientation, the body's spin, both suspensions' compression and the airborne flag. Prints
/// both sides of every tick and each lane's first parting, and holds the worst gap from the touchdown
/// on to [`S67_BOUND`]; see S67's journal for what the numbers say. Skipped with a line where the
/// capture or a state is not in this worktree.
#[test]
fn each_captured_landing_rides_like_the_original() {
    let dir = private_dir("capture");
    let mut compared = 0;
    for (name, states) in [(S67_LANDINGS, &S67_STATES[..]), (S67_RUN2, &S67_RUN2_STATES[..]), (S115_NOSE, &S115_NOSE_STATES[..])] {
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.csv"))) else {
        println!("no {name}.csv — S67's journal says how it is recorded");
        continue;
    };
    let whole = Capture::parse(&text).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
    in_the_world_of(&whole, |world, _, bike| {
        let Some(Ridden { tables, mounts, .. }) = bike else {
            println!("no Race 125 on this disc");
            return;
        };
        let Some(phy) = Disc::open().and_then(|mut disc| the_class_named(&mut disc, &tables, S67_PHY)) else {
            println!("no {S67_PHY} on this disc");
            return;
        };
        for &(asked, touchdown) in states {
            let at = dir.join(format!("{name}-{asked}.ee.bin"));
            let Some(st) = SaveState::open_either(&at) else {
                println!("no {} with the human's rider in it", at.display());
                continue;
            };
            let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
            // The capture has a torn row (a counter skipped); the tape is built from the run of rows
            // around this state that steps one tick a row.
            let cap = dense_run(&whole, saved);
            let tape = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
            let rows = rows_by_counter(&cap);
            // The pad of the tick that produced row `k` is `tape.frames[k - 2]`, not `PadTape::from_capture`'s
            // `k - 1`: this recorder polls the pad buffer the game stamped two frames back, as recording2's
            // does (S67: at `k - 1` the 2751 touchdown parts by 0.3° a tick, at `k - 2` by 1e-6).
            let pad_of = |counter: u64| rows.get(&counter).filter(|&&r| r >= 2).map(|&r| tape.frames[r - 2]);
            let held = st.wheels().unwrap();
            let bits = |v: Vf| [v.0[0].to_bits(), v.0[1].to_bits(), v.0[2].to_bits(), v.0[3].to_bits()];
            assert_eq!([bits(mounts.0), bits(mounts.1)], held, "the state's wheels hang where the 125_faisst's do");
            let curve = st.power_curve().unwrap();
            let ours = phy.engine.curve_rpm.iter().zip(&phy.engine.curve_torque).map(|(r, t)| (r.to_bits(), t.to_bits()));
            assert_eq!(curve, ours.take(curve.len()).collect::<Vec<_>>(), "the state's bike makes {S67_PHY}'s power");
            let mut rider = Rider::new(&phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
            rider.sense.wrecks_enabled = false;
            let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider);
            st.seed_ride(&mut sim.rider.ride).expect("the state's whole bike");
            println!("landing at {touchdown}, from the state at frame {saved}:");
            println!("   tick  side   pitch   roll     yaw | spin x/y/z (world)   | f_comp r_comp |   vy   air");
            let mut first: Option<(u64, Vec<&str>)> = None;
            let (mut ports, mut origs) = (Vec::new(), Vec::new());
            for counter in saved + 1..=touchdown + S67_AFTER {
                let Some(pad) = pad_of(counter) else {
                    println!("  {counter}: no row in the capture");
                    break;
                };
                sim.frame(pad.dt(), &pad.reports, world);
                let port = Touch::of(&sim.rider.ride);
                let Some(orig) = rows.get(&counter).map(|&r| Touch::row(&cap, r).unwrap()) else { break };
                let parted = port.parted(&orig);
                ports.push(port);
                origs.push((counter, orig));
                let mark = if counter == touchdown { " <- touchdown" } else { "" };
                println!("  {counter} orig {}{mark}", orig.line());
                println!("  {counter} port {}  {}", port.line(), parted.join(" "));
                if counter + 3 >= touchdown && counter <= touchdown + 6 {
                    println!("  {counter} orig   wheels {}", orig.wheel_line());
                    println!("  {counter} port   wheels {}", port.wheel_line());
                }
                if first.is_none() && !parted.is_empty() {
                    first = Some((counter, parted));
                }
            }
            match first {
                Some((c, lanes)) => println!("  first parts at {c} ({} after the state): {}", c - saved, lanes.join(" ")),
                None => println!("  agrees on every tick through {}", touchdown + S67_AFTER),
            }
            let worst = worst_from(&ports, &origs, touchdown);
            println!("  {name} {saved}: worst from the touchdown on: {:.2} deg, cofg {:.3}, vel {:.3}, comp {:.3}, \
                      drawn {:.4}, aim {:.4}", worst[0], worst[1], worst[2], worst[3], worst[4], worst[5]);
            assert!(
                (0..6).all(|i| worst[i] <= S67_BOUND[i]),
                "{name} {saved}: the landing rides {worst:?} from the original's (deg, cofg, vel, comp, drawn, aim), \
                 past {S67_BOUND:?}",
            );
            assert!(origs.last().is_some_and(|&(c, _)| c == touchdown + S67_AFTER), "{name} {saved}: rows to the end");
            compared += 1;
        }
    });
    }
    println!("{compared} landings compared");
}

/// The largest gaps between the port's ticks and the original's rows from `touchdown` on, each row
/// set against whichever of the port's ticks before, at and after its counter is nearest it in the
/// centre of mass: the capture reads its rows at three phases of the frame (a stale row repeats the
/// tick before, and the row after one can hold the tick it missed), so a row's own counter is not
/// always the tick it shows.
fn worst_from(ports: &[Touch], origs: &[(u64, Touch)], touchdown: u64) -> [f32; 6] {
    let mut worst = [0.0f32; 6];
    for (k, (counter, orig)) in origs.iter().enumerate() {
        if *counter < touchdown {
            continue;
        }
        let near = (k.saturating_sub(1)..(k + 2).min(ports.len()))
            .map(|j| orig.gap(&ports[j]))
            .min_by(|a, b| a[1].total_cmp(&b[1]))
            .unwrap();
        for i in 0..6 {
            worst[i] = worst[i].max(near[i]);
        }
    }
    worst
}

/// **The original ends every vehicle update with the chassis turned to its node's matrix** (S67):
/// the stunt state pack snapshots the node's world rows as a quaternion and the stunt clocks copy it
/// back into the chassis, so the quaternion the next frame integrates from is the matrix's, whatever
/// the no-hander's roll or the bottom-out's shove did to the node after the integrator. Runs the
/// whole update on each S67 state and holds the chassis quaternion it leaves to the port's
/// `mat4_to_quat` of the node's rows, to the bit — the step `ride.rs` ends its frame with.
#[test]
fn the_vehicle_update_leaves_the_chassis_its_nodes_quaternion() {
    use ps2_oracle::{Ee, addr};
    let dir = private_dir("capture");
    let mut ran = 0;
    let states = S67_STATES.iter().map(|&(asked, _)| (S67_LANDINGS, asked))
        .chain(S67_RUN2_STATES.iter().map(|&(asked, _)| (S67_RUN2, asked)));
    for (name, asked) in states {
        let at = dir.join(format!("{name}-{asked}.ee.bin"));
        let (Some(st), Ok(mut ee)) = (SaveState::open_either(&at), Ee::from_ram(&at)) else {
            println!("no {}", at.display());
            continue;
        };
        ee.set(28, u64::from(addr("boot_gp").unwrap()));
        let ch = st.rider_at(0x7a4).unwrap();
        ee.call(addr("bike_vehicle_update").unwrap(), &[u64::from(st.rider)], &[1.0 / 60.0], 500_000_000);
        let tf = ee.u32_at(ch + 0x254);
        let row = |o: u32| Vf::new(ee.f32_at(o), ee.f32_at(o + 4), ee.f32_at(o + 8), ee.f32_at(o + 12));
        let node = math::Mat4([row(tf + 0x80), row(tf + 0x90), row(tf + 0xa0), row(tf + 0xb0)]);
        let quat = row(ch + 0x1a0);
        assert_eq!(
            math::mat4_to_quat(&node).0.map(f32::to_bits),
            quat.0.map(f32::to_bits),
            "{name}-{asked}: the chassis quaternion after the update is its node's",
        );
        ran += 1;
    }
    println!("{ran} states updated");
}

/// **The nose-first drop's touchdown tick is the original's to the bit** (S115): from the RAM images
/// on its last two airborne ticks, the original's whole vehicle update runs one tick in the oracle,
/// and the port, seeded from the same image as [`each_captured_landing_rides_like_the_original`] does
/// and given the pad that tick read, runs its one tick beside it. The front touches down at 48 a
/// second straight down on the 3458 image's tick, the rear still in the air: S67's touchdown that
/// parted by 0.3 in the velocity. Chassis attitude, centre, velocity and spin, the node's rows and
/// origin, and each wheel's compression and spring force, held bit for bit (before S115's
/// `orientation_point` the centre and the origin were an ulp off in z on both ticks). Skipped with a
/// line where the images are not here.
#[test]
fn the_nose_first_touchdown_tick_is_the_originals() {
    use ps2_oracle::{Ee, addr};
    let dir = private_dir("capture");
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{S115_NOSE}.csv"))) else {
        println!("no {S115_NOSE}.csv — S115's journal says how it is recorded");
        return;
    };
    let whole = Capture::parse(&text).unwrap_or_else(|e| panic!("{S115_NOSE}.csv: {e}"));
    let mut ran = 0;
    in_the_world_of(&whole, |world, _, bike| {
        let Some(Ridden { tables, mounts, .. }) = bike else {
            println!("no Race 125 on this disc");
            return;
        };
        let Some(phy) = Disc::open().and_then(|mut disc| the_class_named(&mut disc, &tables, S67_PHY)) else {
            println!("no {S67_PHY} on this disc");
            return;
        };
        for &(asked, _) in &S115_NOSE_STATES {
            let at = dir.join(format!("{S115_NOSE}-{asked}.ee.bin"));
            let (Some(st), Ok(mut ee)) = (SaveState::open_either(&at), Ee::from_ram(&at)) else {
                println!("no {}", at.display());
                continue;
            };
            let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
            let cap = dense_run(&whole, saved);
            let tape = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{S115_NOSE}.csv: {e}"));
            let rows = rows_by_counter(&cap);
            // The pad of the tick after the state, as the landing test finds it (S67's f−2).
            let pad = tape.frames[rows[&(saved + 1)] - 2];
            let mut rider = Rider::new(&phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
            rider.sense.wrecks_enabled = false;
            let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider);
            st.seed_ride(&mut sim.rider.ride).expect("the state's whole bike");
            sim.frame(pad.dt(), &pad.reports, world);
            let ride = &sim.rider.ride;

            ee.set(28, u64::from(addr("boot_gp").unwrap()));
            let (ch, front, rear) = (st.rider_at(0x7a4).unwrap(), st.rider_at(0x7b0).unwrap(), st.rider_at(0x7b4).unwrap());
            ee.call(addr("bike_vehicle_update").unwrap(), &[u64::from(st.rider)], &[pad.dt().0], 500_000_000);
            let v = |o: u32| [0, 4, 8, 12].map(|k| ee.u32_at(o + k));
            let bits = |v: Vf| v.0.map(f32::to_bits);
            let tf = ee.u32_at(ch + 0x254);
            let lanes = [
                ("quat", v(ch + 0x1a0), bits(ride.chassis.quat)),
                ("cofg", v(ch + 0x60), bits(ride.chassis.cofg_world)),
                ("vel", v(ch + 0x260), bits(ride.chassis.vel)),
                ("spin", v(ch + 0x280), bits(ride.chassis.world_ang_vel)),
                ("node right", v(tf + 0x80), bits(ride.transform().0[0])),
                ("node up", v(tf + 0x90), bits(ride.transform().0[1])),
                ("node forward", v(tf + 0xa0), bits(ride.transform().0[2])),
                ("node origin", v(tf + 0xb0), bits(ride.transform().0[3])),
            ];
            let wheels = [
                ("front comp", ee.u32_at(front + 0xf0), ride.front.w.compression.0.to_bits()),
                ("front spring force", ee.u32_at(front + 0xf8), ride.front.w.spring_force.0.to_bits()),
                ("rear comp", ee.u32_at(rear + 0xf0), ride.rear.w.compression.0.to_bits()),
                ("rear spring force", ee.u32_at(rear + 0xf8), ride.rear.w.spring_force.0.to_bits()),
                // The speed the attachment walk's measure leaves after the physics (S140).
                ("chassis speed", ee.u32_at(ch + 0x34), ride.chassis.speed.0.to_bits()),
            ];
            let f = |b: [u32; 4]| b.map(f32::from_bits);
            println!("{S115_NOSE} {saved} -> {}:", saved + 1);
            for (name, orig, port) in lanes {
                println!("  {name:>18} orig {:?} port {:?}", f(orig), f(port));
            }
            for (name, orig, port) in wheels {
                println!("  {name:>18} orig {} port {}", f32::from_bits(orig), f32::from_bits(port));
            }
            let ulps = |a: u32, b: u32| (a as i32).wrapping_sub(b as i32).unsigned_abs();
            let parted: Vec<String> = lanes
                .iter()
                .filter(|(_, o, p)| o != p)
                .map(|(name, o, p)| format!("{name} by {:?} ulp", [0, 1, 2, 3].map(|i| ulps(o[i], p[i]))))
                .chain(wheels.iter().filter(|(_, o, p)| o != p).map(|(name, o, p)| format!("{name} by {} ulp", ulps(*o, *p))))
                .collect();
            assert!(parted.is_empty(), "{S115_NOSE} {saved}: the port's tick parts from the original's: {parted:?}");
            ran += 1;
        }
    });
    println!("{ran} touchdown ticks compared");
}

/// **The chassis' speed is measured again after the physics** (S140): the riding branch of the bike's
/// vehicle update walks the bike's attachments after the air state, and the chassis' frame update there
/// is its measure, which writes `+0x34` as well as the euler (S138 ported only the euler). On every
/// S67 and S115 RAM image the original's whole vehicle update runs one tick in the oracle, and the
/// speed it leaves is held, to the bit, to the port's [`game_sim::chassis::measure`] of the velocity
/// the same update left — not of the velocity the frame began with, which the top-of-frame measure
/// takes and which differs from it on every image here. The saved image's own speed is the measure
/// of its own velocity too: the state was saved after a frame's walk. The port's side of the same
/// tick, from the original's state and pad, is [`the_nose_first_touchdown_tick_is_the_originals`]'s
/// `chassis speed` lane. Skipped with a line where the images are not here.
#[test]
fn the_attachment_walk_measures_the_speed_after_the_physics() {
    use ps2_oracle::{Ee, addr};
    let dir = private_dir("capture");
    let measured = |vel: Vf| {
        let mut ch = game_sim::chassis::Chassis { vel, ..Default::default() };
        game_sim::chassis::measure(&mut ch, F::ZERO, Vf::ZERO);
        ch.speed.0.to_bits()
    };
    let mut ran = 0;
    let states = S67_STATES.iter().map(|&(asked, _)| (S67_LANDINGS, asked))
        .chain(S67_RUN2_STATES.iter().map(|&(asked, _)| (S67_RUN2, asked)))
        .chain(S115_NOSE_STATES.iter().map(|&(asked, _)| (S115_NOSE, asked)));
    for (name, asked) in states {
        let at = dir.join(format!("{name}-{asked}.ee.bin"));
        let (Some(st), Ok(mut ee)) = (SaveState::open_either(&at), Ee::from_ram(&at)) else {
            println!("no {}", at.display());
            continue;
        };
        ee.set(28, u64::from(addr("boot_gp").unwrap()));
        let ch = st.rider_at(0x7a4).unwrap();
        let vel = |ee: &Ee| Vf::new(ee.f32_at(ch + 0x260), ee.f32_at(ch + 0x264), ee.f32_at(ch + 0x268), ee.f32_at(ch + 0x26c));
        let (vel_before, speed_before) = (vel(&ee), ee.u32_at(ch + 0x34));
        assert_eq!(speed_before, measured(vel_before), "{name}-{asked}: the saved speed is its own velocity's");
        let dt = ee.f32_at(addr("frame_dt").unwrap());
        ee.call(addr("bike_vehicle_update").unwrap(), &[u64::from(st.rider)], &[dt], 500_000_000);
        let (vel_after, speed_after) = (vel(&ee), ee.u32_at(ch + 0x34));
        println!(
            "{name}-{asked}: speed {} -> {} (|vel| before {}, after {})",
            f32::from_bits(speed_before), f32::from_bits(speed_after),
            f32::from_bits(measured(vel_before)), f32::from_bits(measured(vel_after)),
        );
        assert_eq!(speed_after, measured(vel_after), "{name}-{asked}: the update leaves the measure of its own velocity");
        assert_ne!(speed_after, measured(vel_before), "{name}-{asked}: and not the frame's first velocity's");
        ran += 1;
    }
    println!("{ran} states updated");
}

// --- S58: the tyres' slip, case by case ------------------------------------------------------------

/// S58's captures of `watch/s58-slip.toml`, each the recording's own save state ridden on with the
/// kit's pad at a quarter speed (`research/journal/2026-10-21-s58-tyre-slip/capture/` has the pad
/// scripts and the job), in `context/capture/s58-slip/`.
///
/// With each, the last tick the bike's path (`origin`) is bit-exact and the last tick all four slip
/// flags agree (`u64::MAX`: every tick of the capture): a ratchet, to raise. S58 left them at 82/146,
/// 96/126, 105/130 and 82/123, parted by S107's tick-1 drift, the landing off the jump face at 124 and
/// the turn's quaternion sign flip at 130 (journal 2026-10-23-s58-slip-cases). The flip is the stunt
/// clocks' copy-back (S67): the frame ends on `mat4_to_quat` of the node's matrix, whose largest
/// candidate goes from `y` to `w` at 130, so the sign does; the port has done it since S67 and S116
/// raised the pins. The turn's path now parts at 178 by an ulp, after its angular history at 173; the
/// launch's and the brake's at the landing (124, 106). S117's two held traverses of a steep face agree
/// to the bit while both wheels are on it (74 and 97 ticks, side loads up to 1.8 times the wheel's
/// load) and part on the first tick both are off, in the chassis' angular acceleration: steep's slip
/// flags then agree to its last tick, steep2's part at 134, after its landing.
const S58_CASES: [(&str, &str, u64, u64); 6] = [
    ("turn", "a hard turn at speed on flat dirt", 177, u64::MAX),
    ("launch", "a full-gas launch from standing", 123, u64::MAX),
    ("brake", "a locked brake at speed", 105, u64::MAX),
    ("traverse", "a part-steered ride up the jump face and across a side slope", u64::MAX, u64::MAX),
    ("steep", "full gas from standing across a 46° face side on, held up it", 90, 250),
    ("steep2", "the same from 20 ft further back along the face, 48° there", 106, 133),
];

/// S116: the tick the turn's chassis quaternion changes sign in the original — the same rotation,
/// written by the copy-back after the integrate, so the quaternion's history keeps the old sign for a
/// tick. The turn's replay must cross it.
const S116_FLIP: u64 = 130;

/// The S58 cases ridden from a save state of their own rather than first_recording's, by the RAM
/// image in `context/states/` it is replayed from. S117's `steep` is S106's `side` spot (S106's tele.py,
/// `95 -2.7 -10`: the 46° face to the right of the start, the bike side on) and `steep2` the same face
/// 20 ft further back (`95 -2.7 -30`), each ridden on with the gas
/// (`research/journal/2026-10-27-s117-steep-traverse/`). A teleported state's levers and engine are
/// not first_recording's, so these go through [`replay_a_save_state_case`], which seeds them too, with
/// the ticks whose row the recorder read out of step (its `late`), none here.
const S58_OWN_STATES: [(&str, &str, &[u64]); 2] = [("steep", "s117-steep", &[]), ("steep2", "s117-steep2", &[])];

/// How many ticks ahead of a quarter-speed row's ride state its pad is. A row at tick `t` (the ride
/// as it is after tick `t`, [`quarter_ticks`]) carries the control state tick `t + 2` rides on, which
/// is the port's `inputs[t + 1]` (`inputs[k]` drives tick `k + 1`). Measured on S58's launch: with
/// the pad a tick later (`inputs[t]`) the port's rear wheel took the gas's first torque (379.33) and
/// its spin (1.19) at tick 31, the original at 32, every lane one tick ahead; with it here they come
/// on the same tick.
const PAD_LEADS: usize = 1;

/// Each tick's input in a save-state capture ridden with a pad: from a row at tick `t`, the report
/// its pad buffers say was read ([`PadTape::from_capture`]'s half), `PAD_LEADS` on; on an index no
/// row gives (the ones up to the second row's, and the one a late row skips), the nearest one before
/// it, or after it before the first. Checked against the control state the same row carries, which
/// is what the game itself believed was held.
///
/// Except the first tick's buttons, when `own` gives them: the controls the state itself was saved
/// holding ([`SaveState::held_controls`]). The game's first tick after a load rides on the control
/// state it was saved with, and only the next one on the pad the kit sends after the load (S117:
/// steep's state was saved with the brake held, the pad released before the load; with the brake on
/// tick 1 alone, ticks 2 to 14 are the original's to the bit, with it on ticks 1 and 2 or on neither
/// they part at the first compared tick).
fn save_state_inputs(cap: &Capture, ticks: &[u64], own: Option<u16>) -> Result<Vec<Input>, String> {
    let tape = PadTape::from_capture(cap).map_err(|e| e.to_string())?;
    let last = *ticks.last().ok_or("no rows")? as usize;
    let mut on = vec![None; last + 1 + PAD_LEADS];
    // The tape drops row 0 (a full-speed row is the tick before it), so row r is its frame r - 1.
    for (r, &t) in ticks.iter().enumerate().skip(1) {
        let input = tape.frames[r - 1].input();
        let mut held = 0u16;
        for (i, c) in Control::ALL.iter().enumerate() {
            if cell(cap, r, column(cap, &format!("ctl{i}.held"))?) == "1" {
                held |= c.mask();
            }
        }
        if held != input.buttons {
            return Err(format!(
                "row {r}: the pad read {:#06x} but the game held {held:#06x}",
                input.buttons
            ));
        }
        on[t as usize + PAD_LEADS] = Some(input);
    }
    let mut now = on.iter().flatten().next().copied().ok_or("no input")?;
    Ok(on
        .into_iter()
        .map(|i| {
            if let Some(i) = i {
                now = i;
            }
            now
        })
        .enumerate()
        .map(|(k, i)| match own {
            Some(buttons) if k == 0 => Input { buttons, ..i },
            _ => i,
        })
        .collect())
}

/// **Each of S58's slip cases, replayed in the port from the save state's whole bike** on the pad the
/// original read, and every lane's first divergence printed, earliest first: the slip flags, each
/// wheel's spin, local velocity and force, the surface under it, and the chassis' path.
#[test]
fn the_slip_cases_replay_from_the_save_state() {
    let Some((_, acc)) = acceptance_capture() else { return };
    let Some(st) = SaveState::open().filter(|s| s.leads(&acc)) else { return };
    let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
    let seed = acceptance_fixture(&acc).map(|f| f.seed).unwrap_or(0);
    let dir = private_dir("capture").join("s58-slip");
    for (case, what, path_through, slip_through) in S58_CASES {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{case}.csv"))) else {
            println!("{case} ({what}): no capture — `watch/s58-slip.toml` says how it is recorded");
            continue;
        };
        if let Some((_, image, late)) = S58_OWN_STATES.iter().find(|(c, _, _)| *c == case) {
            let Some(own) = SaveState::open_at(private_dir("states").join(format!("{image}.eeMemory.bin"))) else {
                println!("{case} ({what}): no RAM image of its save state ({image})");
                continue;
            };
            let parted = replay_a_save_state_case(&acc, &own, seed, &text, case, what, late);
            let first = |lane: &str| parted.iter().find(|(_, d)| d.contains(lane)).map_or(u64::MAX, |(t, _)| *t);
            assert!(first(".origin[") > path_through, "{case}: the path parts at {}, not after {path_through}", first(".origin["));
            assert!(first(".slip.") > slip_through, "{case}: a slip flag parts at {}, not after {slip_through}", first(".slip."));
            continue;
        }
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{case}.csv: {e}"));
        let ticks = quarter_ticks(&cap, saved).unwrap_or_else(|e| panic!("{case}.csv: {e}"));
        let inputs = save_state_inputs(&cap, &ticks, None).unwrap_or_else(|e| panic!("{case}.csv: {e}"));
        in_the_world_of(&acc, |world, named, bike| {
            let mut sim = acceptance_start(&acc, seed, bike).unwrap();
            let ride = &mut sim.rider.ride;
            st.seed_chassis(ride).expect("the chassis");
            st.seed_wheels(ride).expect("both wheels");
            st.seed_balance(ride).expect("the balance");
            st.seed_engine_outputs(ride).expect("the engine");
            st.seed_air(ride).expect("the air state");
            let mut watches = save_state_watches(&cap, &ticks);
            for (col, wheel) in [("f_surface", "front"), ("r_surface", "rear")] {
                if let Ok(w) = cap.uint(col, &format!("rider.ride.{wheel}.surface")) {
                    watches.push(retick(w, &ticks, 0));
                }
            }
            let f = Fixture {
                name: format!("{case} from the save state"),
                seed,
                start: Some(sim),
                inputs: inputs.clone(),
                watches,
                world: Some(named.clone()),
                ..Default::default()
            };
            // S58_TRACE="case:from:to:lane,lane": the original's and the port's values side by side.
            if let Ok(t) = std::env::var("S58_TRACE") {
                let parts: Vec<&str> = t.split(':').collect();
                if parts[0] == case {
                    let (from, to): (u64, u64) = (parts[1].parse().unwrap(), parts[2].parse().unwrap());
                    let mut sim = f.initial();
                    for (k, &input) in f.inputs.iter().enumerate().take(to as usize) {
                        sim.tick_in(input, world);
                        let tick = k as u64 + 1;
                        if tick < from {
                            continue;
                        }
                        let dump = game_sim::hash::Dump::of(&sim);
                        let mut line = format!("{tick:4} in {:#06x} {:?}", input.buttons, input.stick);
                        for lane in parts[3].split(',') {
                            let w = f.watches.iter().find(|w| w.path.ends_with(lane));
                            let want = w.and_then(|w| w.samples.iter().find(|s| s.tick == tick)).map(|s| format!("{}", s.value));
                            let path = w.map(|w| w.path.clone()).unwrap_or(lane.to_string());
                            let got = dump.get(&path).map(|l| format!("{l}"));
                            line += &format!(" | {lane} {} / {}", want.unwrap_or("-".into()), got.unwrap_or("?".into()));
                        }
                        println!("{line}");
                    }
                }
            }
            let mut parted = Vec::new();
            for w in &f.watches {
                let one = Fixture { name: w.path.clone(), watches: vec![w.clone()], hashes: Vec::new(), ..f.clone() };
                if let Err(d) = replay::verify_in(&one, world) {
                    parted.push((d.tick, d.to_string()));
                }
            }
            parted.sort();
            println!(
                "{case} ({what}): {} ticks, {} of {} lanes part",
                inputs.len(),
                parted.len(),
                f.watches.len()
            );
            for (_, d) in parted.iter().take(6) {
                println!("  {d}");
            }
            // And the slip's own lanes, wherever they come in the order.
            let slip = ["slip.", ".surface", "mv.spin", ".origin[", "local_force", "drive_torque"];
            for (_, d) in parted.iter().skip(6).filter(|(_, d)| slip.iter().any(|s| d.contains(s))) {
                println!("  {d}");
            }
            let agree: Vec<&str> = f
                .watches
                .iter()
                .map(|w| w.path.as_str())
                .filter(|p| !parted.iter().any(|(_, d)| d.contains(&format!("{p} "))))
                .collect();
            println!("  agree on every tick: {}", agree.join(" "));
            let first = |lane: &str| parted.iter().find(|(_, d)| d.contains(lane)).map_or(u64::MAX, |(t, _)| *t);
            // `u64::MAX` asks for every tick, which `>` alone cannot say.
            let past = |at: u64, through: u64| at > through || at == u64::MAX;
            assert!(past(first(".origin["), path_through), "{case}: the path parts at {}, not after {path_through}", first(".origin["));
            assert!(past(first(".slip."), slip_through), "{case}: a slip flag parts at {}, not after {slip_through}", first(".slip."));
            if case == "turn" {
                // The capture really has the flip: `w` and the history's `w` change sign a tick apart.
                let w = |col: &str, t: u64| {
                    let r = ticks.iter().position(|&k| k == t).unwrap_or_else(|| panic!("no row for tick {t}"));
                    cell(&cap, r, column(&cap, col).unwrap()).parse::<f32>().unwrap()
                };
                let (q, h) = ("ch_quat.w", "ch_quat_hist.w");
                assert!(w(q, S116_FLIP - 1) < 0.0 && w(q, S116_FLIP) > 0.0, "turn: no quaternion flip at {S116_FLIP}");
                assert!(w(h, S116_FLIP) < 0.0 && w(h, S116_FLIP + 1) > 0.0, "turn: the history flips with it");
                assert!(first("chassis.quat") > S116_FLIP + 1, "turn: the quaternion parts at {}", first("chassis.quat"));
            }
        });
    }
}

// --- S106: the brake held on a steep face ------------------------------------------------------------

/// **One save-state capture ridden with a pad, replayed in the port from the state's whole bike** on
/// the pad the original read, and every lane's first divergence printed, earliest first: the slip
/// flags, each wheel's spin, local velocity and force, the surface under it, and the chassis' path.
/// S58's test's body, for S106's braked slopes, with the levers and the engine seeded too. Returns
/// every lane that parts, at its first tick. `late` names ticks whose row the recorder read out of step with the ride (S106's nose: the row
/// after the load is the next tick's), whose samples are not compared.
fn replay_a_save_state_case(acc: &Capture, st: &SaveState, seed: u32, text: &str, case: &str, what: &str,
                            late: &[u64]) -> Vec<(u64, String)> {
    let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
    let cap = Capture::parse(text).unwrap_or_else(|e| panic!("{case}.csv: {e}"));
    let ticks = quarter_ticks(&cap, saved).unwrap_or_else(|e| panic!("{case}.csv: {e}"));
    let inputs = save_state_inputs(&cap, &ticks, st.held_controls()).unwrap_or_else(|e| panic!("{case}.csv: {e}"));
    in_the_world_of(acc, |world, named, bike| {
        let mut sim = acceptance_start(acc, seed, bike).unwrap();
        let ride = &mut sim.rider.ride;
        st.seed_chassis(ride).expect("the chassis");
        st.seed_wheels(ride).expect("both wheels");
        st.seed_balance(ride).expect("the balance");
        st.seed_engine_outputs(ride).expect("the engine");
        st.seed_air(ride).expect("the air state");
        st.seed_levers(ride).expect("the levers");
        st.seed_engine_state(ride).expect("the engine's state");
        let mut watches = save_state_watches(&cap, &ticks);
        for (col, wheel) in [("f_surface", "front"), ("r_surface", "rear")] {
            if let Ok(w) = cap.uint(col, &format!("rider.ride.{wheel}.surface")) {
                watches.push(retick(w, &ticks, 0));
            }
        }
        for w in &mut watches {
            w.samples.retain(|s| !late.contains(&s.tick));
        }
        let f = Fixture {
            name: format!("{case} from the save state"),
            seed,
            start: Some(sim),
            inputs: inputs.clone(),
            watches,
            world: Some(named.clone()),
            ..Default::default()
        };
        // S58_TRACE="case:from:to:lane,lane": the original's and the port's values side by side.
        if let Ok(t) = std::env::var("S58_TRACE") {
            let parts: Vec<&str> = t.split(':').collect();
            if parts[0] == case {
                let (from, to): (u64, u64) = (parts[1].parse().unwrap(), parts[2].parse().unwrap());
                let mut sim = f.initial();
                for (k, &input) in f.inputs.iter().enumerate().take(to as usize) {
                    sim.tick_in(input, world);
                    let tick = k as u64 + 1;
                    if tick < from {
                        continue;
                    }
                    let dump = game_sim::hash::Dump::of(&sim);
                    let mut line = format!("{tick:4} in {:#06x} {:?}", input.buttons, input.stick);
                    for lane in parts[3].split(',') {
                        let w = f.watches.iter().find(|w| w.path.ends_with(lane));
                        let want = w.and_then(|w| w.samples.iter().find(|s| s.tick == tick)).map(|s| format!("{}", s.value));
                        let path = w.map(|w| w.path.clone()).unwrap_or(lane.to_string());
                        let got = dump.get(&path).map(|l| format!("{l}"));
                        line += &format!(" | {lane} {} / {}", want.unwrap_or("-".into()), got.unwrap_or("?".into()));
                    }
                    println!("{line}");
                }
            }
        }
        let mut parted = Vec::new();
        for w in &f.watches {
            let one = Fixture { name: w.path.clone(), watches: vec![w.clone()], hashes: Vec::new(), ..f.clone() };
            if let Err(d) = replay::verify_in(&one, world) {
                parted.push((d.tick, d.to_string()));
            }
        }
        parted.sort();
        println!(
            "{case} ({what}): {} ticks, {} of {} lanes part",
            inputs.len(),
            parted.len(),
            f.watches.len()
        );
        for (_, d) in parted.iter().take(6) {
            println!("  {d}");
        }
        // And the slip's own lanes, wherever they come in the order.
        let slip = ["slip.", ".surface", "mv.spin", ".origin[", "local_force", "drive_torque"];
        for (_, d) in parted.iter().skip(6).filter(|(_, d)| slip.iter().any(|s| d.contains(s))) {
            println!("  {d}");
        }
        let agree: Vec<&str> = f
            .watches
            .iter()
            .map(|w| w.path.as_str())
            .filter(|p| !parted.iter().any(|(_, d)| d.contains(&format!("{p} "))))
            .collect();
        println!("  agree on every tick: {}", agree.join(" "));
        parted
    })
    .unwrap_or_default()
}

/// S106's captures of `watch/s58-slip.toml` (`research/journal/2026-10-24-s106-brake-slope-slide/
/// capture/` has the scripts), in `context/capture/s106-slope/`, each with the save state it was
/// ridden from: the brake held from the first frame, the bike stood by `tele.py` on the 53° face
/// ahead of first_recording's start (nose down) and on the 46° face to its right (side on), and from
/// the start itself, rolled over the lip and braked down the face (`roll`: 37 ft/s off the lip, a
/// 45-tick flight and a landing at 64 ft/s; `roll2`: coasted on at 15 ft/s, braked to a stop on the
/// face, held there 150 ticks and let go). The states' RAM images are `./kit state ram`'s of the
/// slots `tele.py` saved, in `context/states/`. Then the ticks not compared (`late`: roll's row 70
/// was read a tick late, the same words as its row 71), and the last tick the path and the slip
/// flags agree through. The slip flags agree on every tick of all four, roll's past its landing
/// (S118); side and roll2 agree on every lane of every tick. Nose's path parts by an ulp at 180, and
/// roll's flight by its roll rate at 67 (S118's journal: not the Euler angles, not the wheels), which
/// reaches its path at 132.
const S106_CASES: [(&str, &str, &str, &[u64], u64, u64); 4] = [
    ("nose", "s106-nose", "the brake held from standing on a 53° face, nose down", &[1], 179, 200),
    ("side", "s106-side", "the brake held from standing on a 46° face, side on", &[], 200, 200),
    ("roll", "", "rolled over the lip and braked down the face", &[70], 131, 240),
    ("roll2", "", "coasted over the lip at 15 ft/s, braked to a stop on the face, then let go", &[], 421, 421),
];

/// **Each of S106's braked slopes, replayed in the port from its own save state**
/// ([`replay_a_save_state_case`]): the path (the chassis' origin) and the wheels' slip flags agree
/// with the original's past the ticks in [`S106_CASES`], which ratchet up.
#[test]
fn the_braked_slopes_replay_from_their_save_states() {
    let Some((_, acc)) = acceptance_capture() else { return };
    let seed = acceptance_fixture(&acc).map(|f| f.seed).unwrap_or(0);
    let dir = private_dir("capture").join("s106-slope");
    for (case, image, what, late, path_through, slip_through) in S106_CASES {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{case}.csv"))) else {
            println!("{case} ({what}): no capture — S106's journal says how it is recorded");
            continue;
        };
        let st = if image.is_empty() {
            SaveState::open().filter(|s| s.leads(&acc))
        } else {
            SaveState::open_at(private_dir("states").join(format!("{image}.eeMemory.bin")))
        };
        let Some(st) = st else {
            println!("{case} ({what}): no RAM image of its save state");
            continue;
        };
        let parted = replay_a_save_state_case(&acc, &st, seed, &text, case, what, late);
        let first = |lane: &str| parted.iter().find(|(_, d)| d.contains(lane)).map_or(u64::MAX, |(t, _)| *t);
        assert!(first(".origin[") > path_through, "{case}: the path parts at {}, not after {path_through}", first(".origin["));
        assert!(first(".slip.") > slip_through, "{case}: a slip flag parts at {}, not after {slip_through}", first(".slip."));
    }
}

// --- S138: a wall and a front wheel, each from a save state taken just before it ---------------------

/// S138's captures (`research/s138_hits.py`, `research/s138/hits.toml`: S67's whole-bike spec on
/// recording2's rider, the wreck reason included): recording2's bike teleported about free09's farmyard
/// and ridden into the garage, or thrown up and pitched forward by hand onto its front wheel, each with
/// save states a few ticks before the hit (`<name>-<frame>.p2s`, the RAM image beside it as
/// `<name>-<frame>.ee.bin`). The capture, the state the port starts from, the ticks it is ridden past
/// the state, and what the case is; whether the original wrecks, and on which tick, is the capture's
/// own `wreck_reason` lane.
const S138_HITS: [(&str, u64, u64, &str); 8] = [
    ("s138/wall", 2849, 12, "head-on into the garage's back wall at 92 a second"),
    ("s138/slow", 2764, 14, "head-on into the back wall at 29 a second"),
    ("s138/gprobe", 2797, 24, "drifting left into the back wall's west corner at 50 a second"),
    ("s138/rub2", 2785, 30, "clipping the shed's south-east corner post at 46 a second, 2 ft in"),
    ("s138/nprobe3", 2791, 24, "a 62 degree nose landing at 30 a second down"),
    ("s138/nprobe4", 2819, 24, "a 66 degree nose landing at 44 a second down"),
    ("s138/nprobe6", 2790, 24, "a 69.9 degree nose landing, a tenth of a degree inside the limit"),
    ("s138/nprobe7", 2787, 24, "a 75 degree nose landing"),
];

/// The bike class recording2 is ridden on, and the files its body rays and frame load from (S86): out
/// of whichever of free09's archives carries them.
const S138_TRACKS: [&str; 3] = ["Tracks\\free09.res", "Tracks\\free09_2.res", "Tracks\\free09_ss_2.res"];

/// The riding bike's two collision files, off the disc.
fn the_bike_col(disc: &mut Disc) -> Option<game_sim::bike_col::BikeCol> {
    use game_sim::bike_col;
    let (rays, shape) = (bike_col::rays_file(bike_col::MODEL), bike_col::shape_file(bike_col::MODEL));
    for track in S138_TRACKS {
        let Some(bytes) = disc.member(track) else { continue };
        let Ok(res) = Res::parse(&bytes) else { continue };
        let read = |name: &str| by_suffix(&res, name).and_then(|e| res.read(&bytes, e).ok());
        if let (Some(r), Some(s)) = (read(&rays), read(&shape)) {
            return Some(bike_col::BikeCol::load(&r, &s).expect("the bike's collision files"));
        }
    }
    None
}

/// The tick a capture's wreck reason first reads set from `from` on, and the reason.
fn first_wreck(cap: &Capture, from: u64) -> Option<(u64, u8)> {
    let (fc, wr) = (column(cap, "frame_counter").ok()?, column(cap, "wreck_reason").ok()?);
    (0..cap.rows.len()).find_map(|r| {
        let counter: u64 = cell(cap, r, fc).parse().ok()?;
        let reason: f64 = cell(cap, r, wr).parse().ok()?;
        (counter > from && reason != 0.0).then_some((counter, reason as u8))
    })
}

/// **A bike into a wall, or onto its front wheel, wrecks or rides it out as the original's does**
/// (S138): each of [`S138_HITS`] ridden by the port from the original's own save state with the
/// original's own pad, on free09 with its props and the bike's body rays, and the wreck verdict — the
/// tick and the reason, or none — held to the capture's. Rows are read at one of three phases of a
/// frame, so the port's tick may be the row's or the one either side of it. Prints both sides' centre
/// of mass, velocity and g peak every tick. Skipped with a line where a capture or state is missing.
#[test]
fn walls_and_nose_landings_wreck_as_the_original_does() {
    let dir = private_dir("capture");
    let (mut compared, mut parted) = (0, Vec::new());
    for (name, asked, ticks, what) in S138_HITS {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.csv"))) else {
            println!("no {name}.csv — S138's journal says how it is recorded");
            continue;
        };
        let whole = Capture::parse(&text).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
        let at = dir.join(format!("{name}-{asked}.ee.bin"));
        let Some(st) = SaveState::open_either(&at) else {
            println!("no {} with the human's rider in it", at.display());
            continue;
        };
        // Free09, by recording2's own capture: a few hundred rows in one yard fit more than one area.
        let Some((_, acc)) = acceptance_capture() else { return };
        in_the_world_of(&acc, |world, _, bike| {
            let Some(Ridden { tables, phy, mounts }) = bike else {
                println!("no Race 125 on this disc");
                return;
            };
            let Some(col) = Disc::open().and_then(|mut disc| the_bike_col(&mut disc)) else {
                println!("no bike collision files on this disc");
                return;
            };
            let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
            let cap = dense_run(&whole, saved);
            let tape = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
            let rows = rows_by_counter(&cap);
            // The recorder polls the pad buffer stamped two frames back, as S67's does.
            let pad_of = |counter: u64| rows.get(&counter).filter(|&&r| r >= 2).map(|&r| tape.frames[r - 2]);
            let curve = st.power_curve().unwrap();
            let ours = phy.engine.curve_rpm.iter().zip(&phy.engine.curve_torque).map(|(r, t)| (r.to_bits(), t.to_bits()));
            assert_eq!(curve, ours.take(curve.len()).collect::<Vec<_>>(), "{name}: the state's bike makes {FREERIDE_PHY}'s power");
            let mut rider = Rider::new(&phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
            rider.carries_bike_col(Arc::new(col));
            let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider);
            st.seed_ride(&mut sim.rider.ride).expect("the state's whole bike");
            let want = first_wreck(&cap, saved).filter(|&(c, _)| c <= saved + ticks);
            println!("{name} ({what}), from the state at frame {saved}: the original {}",
                     want.map_or("rides it out".to_string(), |(c, r)| format!("wrecks at {c}, reason {r}")));
            let g = |r: usize, l: &str| cell(&cap, r, column(&cap, &format!("g_peak.{l}")).unwrap()).parse::<f32>().unwrap();
            let lane = |r: usize, n: &str| -> [f32; 3] {
                ["x", "y", "z"].map(|l| cell(&cap, r, column(&cap, &format!("{n}.{l}")).unwrap()).parse::<f32>().unwrap())
            };
            let mut got = None;
            for counter in saved + 1..=saved + ticks {
                let Some(pad) = pad_of(counter) else {
                    println!("  {counter}: no row in the capture");
                    break;
                };
                sim.frame(pad.dt(), &pad.reports, world);
                let ride = &sim.rider.ride;
                let (c, v, gp) = (ride.chassis.cofg_world, ride.chassis.vel, ride.mv.g_peak);
                if let Some(&r) = rows.get(&counter) {
                    let (oc, ov) = (lane(r, "ch_cofg"), lane(r, "ch_vel"));
                    let q = lane(r, "ch_quat");
                    let w = cell(&cap, r, column(&cap, "ch_quat.w").unwrap()).parse::<f32>().unwrap();
                    let (pitch, ..) = attitude(Vf::new(q[0], q[1], q[2], w));
                    println!("  {counter} orig cofg {:9.3} {:8.3} {:9.3} vel {:7.2} {:7.2} {:7.2} g {:6.2} {:6.2} {:6.2} pitch {pitch:7.3}",
                             oc[0], oc[1], oc[2], ov[0], ov[1], ov[2], g(r, "x"), g(r, "y"), g(r, "z"));
                }
                let (pitch, ..) = attitude(ride.chassis.quat);
                println!("  {counter} port cofg {:9.3} {:8.3} {:9.3} vel {:7.2} {:7.2} {:7.2} g {:6.2} {:6.2} {:6.2} pitch {pitch:7.3} \
                          ground {}{}",
                         c.x(), c.y(), c.z(), v.x(), v.y(), v.z(), gp.x(), gp.y(), gp.z(), ride.air.on_ground,
                         sim.rider.state.wreck.map_or(String::new(), |w| format!("  wreck {}", w.0)));
                if got.is_none() && let Some(w) = sim.rider.state.wreck {
                    got = Some((counter, w.0));
                }
            }
            println!("  the port {}", got.map_or("rides it out".to_string(), |(c, r)| format!("wrecks at {c}, reason {r}")));
            let agrees = match (want, got) {
                (None, None) => true,
                (Some((wc, wr)), Some((gc, gr))) => gr == wr && gc.abs_diff(wc) <= 1,
                _ => false,
            };
            if !agrees {
                parted.push(format!("{name} ({what}): the original {want:?}, the port {got:?} (tick, reason)"));
            }
            compared += 1;
        });
    }
    println!("{compared} hits compared");
    assert!(parted.is_empty(), "{}", parted.join("\n"));
}

// --- S139: the chassis' sweep back, a bike thrown sideways through a wall and a post ------------------

/// S139's captures (`research/s139/shove.sh`, `research/s139/sweep.toml`: S138's whole bike plus the
/// riding bike's sweep back — the record's swept segment, its sweep probe's flags and contact), in
/// `context/capture/s138/` beside S138's: recording2's bike teleported about free09's farmyard and
/// thrown by hand at 200 ft/s (clamped to the riding 135 on its first integrate), 8 up, or ridden in
/// on the gas. The body rays reach ±1.3 ft sideways and 3.6 ft ahead of the hub, so a bike moving 4.4
/// ft a tick sideways passes a wall between two ticks' rays, and only the sweep back meets it; head on
/// the rays meet a post first and the sweep never strikes. The capture, the state the port starts from
/// (the third saved, as the recorder's first two rows have no pad), the ticks ridden, and what it is.
const S139_SWEEPS: [(&str, u64, u64, &str); 3] = [
    ("s138/side1", 2469, 20, "thrown west through the shed's east wall, mid-wall"),
    ("s138/sidepost", 2470, 16, "thrown west through the shed's south-east corner post"),
    ("s138/post", 2564, 10, "ridden head-on into the south-east corner post at 82 a second, the gas held"),
];

/// The sweep probe's flag bit for "its last search struck something" (`+0x116` 0x20).
const SWEEP_STRUCK: u32 = 0x20;

/// How far the port may put the bike back from where the original does on a tick both strike, and
/// its chassis stray from the original's while both ride, in feet, per lane.
const SWEPT_TO_THE_ORIGINALS: f32 = 0.05;

/// **A bike too fast for its body rays is put back out of a thin prop as the original's is** (S139):
/// each of [`S139_SWEEPS`] ridden by the port from the original's save state with the original's pad,
/// as [`walls_and_nose_landings_wreck_as_the_original_does`] rides S138's. Every tick, before the
/// frame, the chassis' sweep back ([`game_sim::ride::Ride::sweep_back`], the search the rider's tick
/// opens with) is run on a copy of the port's bike: it must strike on the ticks the original's sweep
/// probe reads struck, and put the node where the original's record says it put it
/// ([`SWEPT_TO_THE_ORIGINALS`]); the chassis the rider's own sweep puts back keeps to the
/// original's path while both ride; and the first wreck is the original's (reason, tick ±1).
#[test]
fn the_sweep_back_puts_a_fast_bike_back_out_of_a_post_as_the_original_does() {
    let dir = private_dir("capture");
    let (mut compared, mut swept, mut parted) = (0, 0, Vec::new());
    for (name, asked, ticks, what) in S139_SWEEPS {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.csv"))) else {
            println!("no {name}.csv — S139's journal says how it is recorded");
            continue;
        };
        let whole = Capture::parse(&text).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
        let at = dir.join(format!("{name}-{asked}.ee.bin"));
        let Some(st) = SaveState::open_either(&at) else {
            println!("no {} with the human's rider in it", at.display());
            continue;
        };
        let Some((_, acc)) = acceptance_capture() else { return };
        in_the_world_of(&acc, |world, _, bike| {
            let Some(Ridden { tables, phy, mounts }) = bike else {
                println!("no Race 125 on this disc");
                return;
            };
            let Some(col) = Disc::open().and_then(|mut disc| the_bike_col(&mut disc)) else {
                println!("no bike collision files on this disc");
                return;
            };
            let col = Arc::new(col);
            let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
            let cap = dense_run(&whole, saved);
            let tape = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
            let rows = rows_by_counter(&cap);
            let pad_of = |counter: u64| rows.get(&counter).filter(|&&r| r >= 2).map(|&r| tape.frames[r - 2]);
            let mut rider = Rider::new(&phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
            rider.carries_bike_col(Arc::clone(&col));
            let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider);
            st.seed_ride(&mut sim.rider.ride).expect("the state's whole bike");
            let want = first_wreck(&cap, saved).filter(|&(c, _)| c <= saved + ticks);
            println!("{name} ({what}), from the state at frame {saved}: the original {}",
                     want.map_or("rides it out".to_string(), |(c, r)| format!("wrecks at {c}, reason {r}")));
            let num = |r: usize, n: &str| cell(&cap, r, column(&cap, n).unwrap()).parse::<f32>().unwrap();
            let lane = |r: usize, n: &str| [num(r, &format!("{n}.x")), num(r, &format!("{n}.y")), num(r, &format!("{n}.z"))];
            // The port's tick N is the original's row N + 1 (its chassis lanes read the tick's end a
            // frame late); the sweep lanes are read at the tick's top, so a struck row may instead hold
            // the next tick's sweep (sidepost's rows 2474..2476): a row is met by the port's sweep of
            // the tick before it or of its own tick, and a sweep a read-ahead row hides is held only to
            // fall among the original's own strikes.
            let (mut got, mut ours_all, mut theirs_all) = (None, Vec::new(), Vec::new());
            for counter in saved + 1..=saved + ticks {
                let Some(pad) = pad_of(counter) else {
                    println!("  {counter}: no row in the capture");
                    break;
                };
                // The port's sweep back on this tick, on a copy: the rider's tick runs the same one
                // first, from the same bike, while it is riding.
                let ours = (sim.rider.state.wreck.is_none()).then(|| {
                    let mut probe = sim.rider.ride.clone();
                    probe.sweep_back(&phy, Some(&col.rays), world, pad.dt()).then_some(probe.origin)
                }).flatten();
                let ours = ours.map(|o| [o.x(), o.y(), o.z()]);
                sim.frame(pad.dt(), &pad.reports, world);
                let ride = &sim.rider.ride;
                let c = ride.chassis.cofg_world;
                let show = |s: Option<[f32; 3]>| s.map_or("-".to_string(), |s| format!("{:9.3} {:8.3} {:9.3}", s[0], s[1], s[2]));
                if let Some(o) = ours {
                    ours_all.push((counter, o));
                }
                if let Some(&r) = rows.get(&(counter + 1)) {
                    let theirs = (num(r, "sw_flags") as u32 & SWEEP_STRUCK != 0 && num(r, "wreck_reason") == 0.0).then(|| lane(r, "sw_to"));
                    let oc = lane(r, "ch_cofg");
                    println!("  {} orig cofg {:9.3} {:8.3} {:9.3} swept to {}", counter + 1, oc[0], oc[1], oc[2], show(theirs));
                    // The bike the rider's own sweep put back rides the original's path until either
                    // wrecks; a row read a tick ahead (post's 2567 repeats 2568) holds the next tick's,
                    // and the port's tick before has no row.
                    let riding = sim.rider.state.wreck.is_none() && num(r, "wreck_reason") == 0.0;
                    let ahead = rows.get(&(counter + 2)).is_some_and(|&n| lane(n, "ch_cofg") == oc);
                    if riding && !ahead && (0..3).any(|k| ([c.x(), c.y(), c.z()][k] - oc[k]).abs() > SWEPT_TO_THE_ORIGINALS) {
                        parted.push(format!("{name} at {counter}: the port's chassis at {c:?}, the original's at {oc:?}"));
                    }
                    if let Some(t) = theirs.filter(|_| counter < saved + ticks) {
                        theirs_all.push((counter + 1, t));
                    }
                }
                println!("  {counter} port cofg {:9.3} {:8.3} {:9.3} swept to {}{}", c.x(), c.y(), c.z(), show(ours),
                         sim.rider.state.wreck.map_or(String::new(), |w| format!("  wreck {}", w.0)));
                if got.is_none() && let Some(w) = sim.rider.state.wreck {
                    got = Some((counter + 1, w.0));
                }
            }
            let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() <= SWEPT_TO_THE_ORIGINALS);
            // Only rows before the port's wreck (a wrecked port bike sweeps no more) are held.
            let theirs_all: Vec<_> = theirs_all.into_iter().filter(|&(r, _)| got.is_none_or(|(g, _)| r < g)).collect();
            for &(r, t) in &theirs_all {
                if !ours_all.iter().any(|&(c, o)| (r - 1..=r).contains(&c) && near(o, t)) {
                    parted.push(format!("{name} at row {r}: the original's sweep puts the bike back at {t:?}, no port sweep of tick {} or {r} does", r - 1));
                }
            }
            for &(c, o) in &ours_all {
                let seen = theirs_all.iter().any(|&(r, t)| (c + 1 == r || c == r) && near(o, t));
                let among = theirs_all.iter().any(|&(r, _)| r + 1 >= c && r <= c + 3);
                if !seen && !among && want.is_none_or(|(w, _)| c + 2 < w) {
                    parted.push(format!("{name} at {c}: the port's sweep puts the bike back at {o:?}, the original's sweep strikes on no row near it"));
                }
            }
            swept += theirs_all.len();
            println!("  the port {}; the original's sweep struck on {} rows, the port's on {} ticks",
                     got.map_or("rides it out".to_string(), |(c, r)| format!("wrecks at {c}, reason {r}")), theirs_all.len(), ours_all.len());
            let agrees = match (want, got) {
                (None, None) => true,
                (Some((wc, wr)), Some((gc, gr))) => gr == wr && gc.abs_diff(wc) <= 1,
                _ => false,
            };
            if !agrees {
                parted.push(format!("{name} ({what}): the original {want:?}, the port {got:?} (tick, reason)"));
            }
            compared += 1;
        });
    }
    println!("{compared} sweeps compared, {swept} of the original's put-backs met");
    assert!(compared == 0 || swept > 0, "no capture's sweep struck: the comparison proved nothing");
    assert!(parted.is_empty(), "{}", parted.join("\n"));
}

// --- S150: crooked landings, each from a save state taken in the air ------------------------------

/// One tick of a crooked landing, both sides: the frame, the original's heading and whether its row
/// reads the wheels down (where the capture has a row), and the port's heading and wheels.
#[derive(Clone, Copy, Debug)]
struct Turned {
    counter: u64,
    original: Option<(f32, bool)>,
    port: (f32, bool),
}

/// Ride `name`'s capture (`research/s150/spin.sh`) in the port from its state at `asked` for `ticks`
/// ticks, as [`walls_and_nose_landings_wreck_as_the_original_does`] rides S138's: free09, the props,
/// the bike's body rays, the original's pad. Answers each tick's [`Turned`], the original's first
/// wreck and the port's, or `None` and a line where a capture, state or disc file is missing.
#[allow(clippy::type_complexity)]
fn ride_a_crooked_landing(name: &str, asked: u64, ticks: u64)
    -> Option<(Vec<Turned>, Option<(u64, u8)>, Option<(u64, u8)>)> {
    let dir = private_dir("capture");
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.csv"))) else {
        println!("no {name}.csv — S150's journal says how it is recorded");
        return None;
    };
    let whole = Capture::parse(&text).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
    let at = dir.join(format!("{name}-{asked}.ee.bin"));
    let Some(st) = SaveState::open_either(&at) else {
        println!("no {} with the human's rider in it", at.display());
        return None;
    };
    let (_, acc) = acceptance_capture()?;
    let mut out = None;
    in_the_world_of(&acc, |world, _, bike| {
        let Some(Ridden { tables, phy, mounts }) = bike else {
            println!("no Race 125 on this disc");
            return;
        };
        let Some(col) = Disc::open().and_then(|mut disc| the_bike_col(&mut disc)) else {
            println!("no bike collision files on this disc");
            return;
        };
        let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
        let cap = dense_run(&whole, saved);
        let tape = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
        let rows = rows_by_counter(&cap);
        let pad_of = |counter: u64| rows.get(&counter).filter(|&&r| r >= 2).map(|&r| tape.frames[r - 2]);
        let mut rider = Rider::new(&phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
        rider.carries_bike_col(Arc::new(col));
        let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider);
        st.seed_ride(&mut sim.rider.ride).expect("the state's whole bike");
        let want = first_wreck(&cap, saved).filter(|&(c, _)| c <= saved + ticks);
        let num = |r: usize, n: &str| cell(&cap, r, column(&cap, n).unwrap()).parse::<f32>().unwrap();
        let (mut seen, mut got) = (Vec::new(), None);
        for counter in saved + 1..=saved + ticks {
            let Some(pad) = pad_of(counter) else { break };
            sim.frame(pad.dt(), &pad.reports, world);
            let ride = &sim.rider.ride;
            let original = rows.get(&counter).map(|&r| {
                let q = Vf::new(num(r, "ch_quat.x"), num(r, "ch_quat.y"), num(r, "ch_quat.z"), num(r, "ch_quat.w"));
                (attitude(q).2, num(r, "on_ground") != 0.0)
            });
            seen.push(Turned { counter, original, port: (attitude(ride.chassis.quat).2, ride.air.on_ground) });
            if got.is_none() && let Some(w) = sim.rider.state.wreck {
                got = Some((counter, w.0));
            }
        }
        out = Some((seen, want, got));
    });
    out
}

/// S150's landings, each a capture of recording2's rider thrown up off free09's farmyard with the
/// stick held right through the air (`research/s150/spin.sh NAME UP 0 40 True research/s150/right.pad`,
/// 200 frames), and the frame of the save state it is ridden from: the third, as the recorder's first
/// two rows have no pad. The crossup at the touchdown is the bike's heading against its velocity's.
const S150_LANDINGS: [(&str, u64, u64, &str); 3] = [
    ("s138/s150-r50", 2747, 100, "thrown up 50 ft/s, lands about 118 degrees crossed up"),
    ("s138/s150-r60", 2709, 195, "thrown up 60 ft/s, lands about 148 degrees crossed up"),
    ("s138/s150-r70", 2750, 195, "thrown up 70 ft/s, lands about 169 degrees crossed up, backwards"),
];

/// How far the port's heading may stray from the original's on any tick of a landing both ride out,
/// from the touchdown to the end of the capture, in degrees: the worst seen is 5.06, six ticks after
/// r60's touchdown, where the capture rows' phase moves the most (memory: three phases).
const HEADING_FROM_THE_ORIGINAL: f32 = 6.0;

/// The angle from `b` to `a` in degrees, wrapped into -180..180.
fn turned_by(a: f32, b: f32) -> f32 {
    (a - b + 540.0).rem_euclid(360.0) - 180.0
}

/// **S150.** A trick landed crooked: the original wrecks a landing crossed up 118 degrees (the
/// landing crossup clause, reason 3), and rides out ones crossed up 148 and 169 degrees, the ground
/// then swinging the bike round by 135 and 167 degrees over the next three seconds. The port must do the
/// same from the same states with the same pad: wreck where the original does (reason, tick ±1), and
/// where it rides out, ride out too, touching down within a tick of it and keeping its heading
/// within [`HEADING_FROM_THE_ORIGINAL`] of the original's on every tick of the swing. A landing mid
/// trick is the trick clause's, proved in `tests/trick.rs` (`a_scripted_pad_lands_one_trick_and_wrecks_another`)
/// and `tests/air.rs`.
#[test]
fn crooked_landings_wreck_or_swing_round_as_the_original_does() {
    let (mut compared, mut parted) = (0, Vec::new());
    for (name, asked, ticks, what) in S150_LANDINGS {
        let Some((seen, want, got)) = ride_a_crooked_landing(name, asked, ticks) else { continue };
        compared += 1;
        println!("{name} ({what}): the original {want:?}, the port {got:?}");
        match (want, got) {
            (Some((c, r)), Some((gc, gr))) if r == gr && gc.abs_diff(c) <= 1 => {}
            (None, None) => {
                let down = |side: fn(&Turned) -> bool| seen.iter().position(side);
                let Some(o) = down(|t| t.original.is_some_and(|(_, g)| g)) else {
                    parted.push(format!("{name}: the original never touches down in the capture"));
                    continue;
                };
                match down(|t| t.port.1) {
                    Some(p) if p.abs_diff(o) <= 1 => {}
                    p => parted.push(format!("{name}: the port touches down at {:?}, the original at {}",
                                             p.map(|p| seen[p].counter), seen[o].counter)),
                }
                let rows: Vec<_> = seen[o..].iter().filter_map(|t| t.original.map(|(h, _)| (t.counter, h, t.port.0))).collect();
                let swing = turned_by(rows.last().unwrap().1, rows[0].1);
                let worst = rows.iter().map(|&(c, h, p)| (c, turned_by(p, h).abs())).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
                println!("  touchdown at {}: the original swings {swing:.1} degrees by {}, the port strays {:.2} at most (at {})",
                         seen[o].counter, rows.last().unwrap().0, worst.1, worst.0);
                if worst.1 > HEADING_FROM_THE_ORIGINAL {
                    parted.push(format!("{name}: the port's heading strays {:.2} degrees from the original's at {}", worst.1, worst.0));
                }
            }
            _ => parted.push(format!("{name}: the original {want:?}, the port {got:?}")),
        }
    }
    println!("{compared} landings compared");
    assert!(parted.is_empty(), "{}", parted.join("\n"));
}

// --- P147: a backflip cut short onto the roof, under Air Wrecks on and off ---------------------------

/// P147's captures (`research/p147`: `make_tape.py` on recording2, `play.sh`, `roof.toml`, which is
/// S138's whole-bike spec plus the flip flags and the riding collision record): flip 1 with the stick
/// pushed against it from field 298, which bleeds the torque off near upside down. The capture, the
/// state the port starts from, the ticks ridden past it, and what the case is.
const P147_LANDINGS: [(&str, u64, u64, &str); 2] = [
    ("p147/roof", 2580, 20, "the stick centred at field 320: still flipping, onto the roof at up.y -0.93"),
    ("p147/nose", 2580, 20, "the stick held forward to field 420: turned back past vertical, onto the nose"),
];

impl SaveState {
    /// The backflip off the rider, which [`SaveState::seed_ride`] leaves at its defaults: the four
    /// flags (`+0x4f4` the torque applied this substep, `+0x4f8` flipping, `+0x4fc` triggered, `+0x500`
    /// armed), the torque (`+0x520`) and the control-authority ramp a finished flip starts (`+0x4e8`). A flip still turning keeps the airborne attitude clause from running, so a state
    /// taken mid-flip and seeded without them is judged as a bike that is not flipping.
    fn seed_flip(&self, ride: &mut game_sim::ride::Ride) -> Option<()> {
        let r = self.rider;
        let air = &mut ride.air;
        air.flip_torque_applied = self.flag(r + 0x4f4)?;
        air.flipping = self.flag(r + 0x4f8)?;
        air.flip_active = self.flag(r + 0x4fc)?;
        air.flip_armed = self.flag(r + 0x500)?;
        air.flip_torque = self.vf(r + 0x520)?;
        air.authority = self.float(r + 0x4e8)?;
        Some(())
    }
}

/// **A bike landed on its roof wrecks on the original's tick and reason, Air Wrecks on and off**
/// (P147): each of [`P147_LANDINGS`] ridden by the port from the original's save state with the
/// original's pad, the bike's body rays and free09's ground, once with [`game_sim::state::Sense::air_wrecks`] on and
/// once off, and the wreck verdict held to the capture's (the tick ±1, as in S138, and the reason).
/// The roof landing is the original's reason 6: the body rays search the ground once the bike's up
/// is under a quarter, and the head ray finds it. The airborne clause never runs on it (the flip is
/// still on), so the switch cannot change it; the nose landing is a landing clause's, which the
/// switch leaves alone too. Skipped with a line where a capture or state is missing.
#[test]
fn roof_landings_wreck_as_the_original_does_air_wrecks_on_and_off() {
    let dir = private_dir("capture");
    let (mut compared, mut parted) = (0, Vec::new());
    for (name, asked, ticks, what) in P147_LANDINGS {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.csv"))) else {
            println!("no {name}.csv — P147's journal says how it is recorded");
            continue;
        };
        let whole = Capture::parse(&text).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
        let at = dir.join(format!("{name}-{asked}.ee.bin"));
        let Some(st) = SaveState::open_either(&at) else {
            println!("no {} with the human's rider in it", at.display());
            continue;
        };
        let Some((_, acc)) = acceptance_capture() else { return };
        in_the_world_of(&acc, |world, _, bike| {
            let Some(Ridden { tables, phy, mounts }) = bike else {
                println!("no Race 125 on this disc");
                return;
            };
            let Some(col) = Disc::open().and_then(|mut disc| the_bike_col(&mut disc)) else {
                println!("no bike collision files on this disc");
                return;
            };
            let col = Arc::new(col);
            let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
            let cap = dense_run(&whole, saved);
            let tape = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
            let rows = rows_by_counter(&cap);
            let pad_of = |counter: u64| rows.get(&counter).filter(|&&r| r >= 2).map(|&r| tape.frames[r - 2]);
            let want = first_wreck(&cap, saved).filter(|&(c, _)| c <= saved + ticks);
            println!("{name} ({what}), from the state at frame {saved}: the original {}",
                     want.map_or("rides it out".to_string(), |(c, r)| format!("wrecks at {c}, reason {r}")));
            assert!(want.is_some(), "{name}: the capture's original wrecks within {ticks} ticks");
            for air_wrecks in [true, false] {
                let mut rider = Rider::new(&phy, mounts.clone(), Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
                rider.carries_bike_col(Arc::clone(&col));
                let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider);
                st.seed_ride(&mut sim.rider.ride).expect("the state's whole bike");
                st.seed_flip(&mut sim.rider.ride).expect("the state's flip flags");
                sim.rider.sense.air_wrecks = air_wrecks;
                let mut got = None;
                for counter in saved + 1..=saved + ticks {
                    let Some(pad) = pad_of(counter) else {
                        println!("  {counter}: no row in the capture");
                        break;
                    };
                    sim.frame(pad.dt(), &pad.reports, world);
                    let ride = &sim.rider.ride;
                    if let Some(&r) = rows.get(&counter) {
                        let up = cell(&cap, r, column(&cap, "up_y").unwrap());
                        let ow = cell(&cap, r, column(&cap, "wreck_reason").unwrap());
                        println!("  {counter} air wrecks {air_wrecks}: orig up.y {up:>10} wreck {ow}  port up.y {:10.6} flipping {} ground {}{}",
                                 ride.transform().0[1].y(), ride.air.flipping, ride.air.on_ground,
                                 sim.rider.state.wreck.map_or(String::new(), |w| format!("  wreck {}", w.0)));
                    }
                    if got.is_none() && let Some(w) = sim.rider.state.wreck {
                        got = Some((counter, w.0));
                        break;
                    }
                }
                println!("  the port, air wrecks {air_wrecks}: {}", got.map_or("rides it out".to_string(), |(c, r)| format!("wrecks at {c}, reason {r}")));
                let agrees = match (want, got) {
                    (Some((wc, wr)), Some((gc, gr))) => gr == wr && gc.abs_diff(wc) <= 1,
                    _ => false,
                };
                if !agrees {
                    parted.push(format!("{name} ({what}), air wrecks {air_wrecks}: the original {want:?}, the port {got:?} (tick, reason)"));
                }
                compared += 1;
            }
        });
    }
    println!("{compared} landings compared");
    assert!(parted.is_empty(), "{}", parted.join("\n"));
}

/// The tail landings S144 captured in the original (context/capture/s144, research/s144/tail.sh):
/// recording2's bike thrown up in free09's farmyard with a tail-down pitch rate (and, for the crossed-up
/// one, a yaw rate), from a state saved in the air, onto the yard's flat ground with the pad idle. Name,
/// the frame its state was asked at, how many ticks to ride, and what it is.
const S144_TAILS: [(&str, u64, u64, &str); 12] = [
    ("s144/probe1", 2718, 60, "a 8 degree tail-down landing at 26 a second down, rolled on"),
    ("s144/probe2", 2710, 70, "a 15 degree tail-down landing at 32 a second down"),
    ("s144/probe3", 2740, 100, "a flat landing at 47 a second down"),
    ("s144/mid", 2742, 100, "a 52 degree tail-down landing at 49 a second down, the rear shock bottomed"),
    ("s144/probe4", 2740, 100, "a 70 degree tail-down slam at 52 a second down, the rear shock bottomed"),
    ("s144/tall", 2742, 145, "a 52 degree tail-down slam at 73 a second down, past the vertical g limit"),
    ("s144/cross5", 2735, 100, "a 34 degree tail-down landing at 41 a second down, crossed up 13 degrees"),
    ("s144/cross7", 2755, 90, "a 25 degree tail-down landing at 40 a second down, crossed up 35 and leaned 43"),
    ("s144/cross8", 2752, 130, "a 35 degree tail-down landing at 64 a second down, crossed up 20 degrees"),
    ("s144/cross6", 2742, 90, "crossed up 49 degrees and leaned 50, landing front wheel first"),
    ("s144/midgas", 2751, 120, "a 55 degree tail-down landing at 53 a second down on the gas"),
    ("s144/midback", 2734, 60, "on the gas with the stick pulled back: looped over in the air"),
];

/// **A tail-first landing wrecks or rides out as the original's does** (S144): each of
/// [`S144_TAILS`] ridden by the port from the original's state with the original's pad, the same way
/// and to the same tolerance as S138's walls and nose landings
/// ([`walls_and_nose_landings_wreck_as_the_original_does`]): the first wreck's reason and tick ±1, or
/// none on both sides. Skipped with a line where a capture or state is missing.
#[test]
fn tail_landings_wreck_as_the_original_does() {
    let parted = wrecks_as_the_original_does(&S144_TAILS);
    assert!(parted.is_empty(), "{}", parted.join("\n"));
}

/// Rides each case from its state in free09's world and answers the ones whose wreck verdict parts
/// from the capture's. Prints both sides' centre of mass, velocity, g peak and pitch every tick, and
/// the original's rear shock compression and the port's wheels on the ground.
fn wrecks_as_the_original_does(cases: &[(&str, u64, u64, &str)]) -> Vec<String> {
    let dir = private_dir("capture");
    let (mut compared, mut parted) = (0, Vec::new());
    for &(name, asked, ticks, what) in cases {
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{name}.csv"))) else {
            println!("no {name}.csv — S144's journal says how it is recorded");
            continue;
        };
        let whole = Capture::parse(&text).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
        let at = dir.join(format!("{name}-{asked}.ee.bin"));
        let Some(st) = SaveState::open_either(&at) else {
            println!("no {} with the human's rider in it", at.display());
            continue;
        };
        let Some((_, acc)) = acceptance_capture() else { break };
        in_the_world_of(&acc, |world, _, bike| {
            let Some(Ridden { tables, phy, mounts }) = bike else {
                println!("no Race 125 on this disc");
                return;
            };
            let Some(col) = Disc::open().and_then(|mut disc| the_bike_col(&mut disc)) else {
                println!("no bike collision files on this disc");
                return;
            };
            let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
            let cap = dense_run(&whole, saved);
            let tape = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{name}.csv: {e}"));
            let rows = rows_by_counter(&cap);
            // The pad two frames back, as S138's; these states are saved on the capture's first row and
            // the pad is idle throughout (research/s138/idle.pad), so the first two ticks take row 0's.
            let pad_of = |counter: u64| rows.get(&counter).map(|&r| tape.frames[r.saturating_sub(2)]);
            let mut rider = Rider::new(&phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
            rider.carries_bike_col(Arc::new(col));
            let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider);
            st.seed_ride(&mut sim.rider.ride).expect("the state's whole bike");
            let want = first_wreck(&cap, saved).filter(|&(c, _)| c <= saved + ticks);
            println!("{name} ({what}), from the state at frame {saved}: the original {}",
                     want.map_or("rides it out".to_string(), |(c, r)| format!("wrecks at {c}, reason {r}")));
            let num = |r: usize, n: &str| cell(&cap, r, column(&cap, n).unwrap()).parse::<f32>().unwrap();
            let lane = |r: usize, n: &str| ["x", "y", "z"].map(|l| num(r, &format!("{n}.{l}")));
            let mut got = None;
            for counter in saved + 1..=saved + ticks {
                let Some(pad) = pad_of(counter) else {
                    println!("  {counter}: no row in the capture");
                    break;
                };
                sim.frame(pad.dt(), &pad.reports, world);
                let ride = &sim.rider.ride;
                let (c, v, gp) = (ride.chassis.cofg_world, ride.chassis.vel, ride.mv.g_peak);
                if let Some(&r) = rows.get(&counter) {
                    let (oc, ov, og, q) = (lane(r, "ch_cofg"), lane(r, "ch_vel"), lane(r, "g_peak"), lane(r, "ch_quat"));
                    let (pitch, ..) = attitude(Vf::new(q[0], q[1], q[2], num(r, "ch_quat.w")));
                    println!("  {counter} orig cofg {:9.3} {:8.3} {:9.3} vel {:7.2} {:7.2} {:7.2} g {:6.2} {:6.2} {:6.2} pitch {pitch:7.3} \
                              comp {:.2} {:.2}",
                             oc[0], oc[1], oc[2], ov[0], ov[1], ov[2], og[0], og[1], og[2], num(r, "f_comp"), num(r, "r_comp"));
                }
                let (pitch, ..) = attitude(ride.chassis.quat);
                println!("  {counter} port cofg {:9.3} {:8.3} {:9.3} vel {:7.2} {:7.2} {:7.2} g {:6.2} {:6.2} {:6.2} pitch {pitch:7.3} \
                          ground {}{}",
                         c.x(), c.y(), c.z(), v.x(), v.y(), v.z(), gp.x(), gp.y(), gp.z(), ride.air.on_ground,
                         sim.rider.state.wreck.map_or(String::new(), |w| format!("  wreck {}", w.0)));
                if got.is_none() && let Some(w) = sim.rider.state.wreck {
                    got = Some((counter, w.0));
                }
            }
            println!("  the port {}", got.map_or("rides it out".to_string(), |(c, r)| format!("wrecks at {c}, reason {r}")));
            let agrees = match (want, got) {
                (None, None) => true,
                (Some((wc, wr)), Some((gc, gr))) => gr == wr && gc.abs_diff(wc) <= 1,
                _ => false,
            };
            if !agrees {
                parted.push(format!("{name} ({what}): the original {want:?}, the port {got:?} (tick, reason)"));
            }
            compared += 1;
        });
    }
    println!("{compared} landings compared");
    parted
}

// --- S126: a free09 trunk, ridden into from S100's save state -------------------------------------

/// S100's two rides of recording2's 125 flat out into free09's plant 347 (a TreeB), each with the
/// state it starts from (`context/capture/s100/<name>.ee.bin`, the `.p2s`'s `eeMemory.bin`): `tree`
/// as the state is, `nowreck` with the original's wrecks switched off.
const S100_TREE: &str = "s100/tree";

/// Ticks of [`S100_TREE`] ridden: the run-up (61 ticks) and the hit, with room either side.
const S100_TICKS: u64 = 70;

/// Plant 347's axis on the ground plane (S100): the trunk the ride is aimed at.
const S100_TRUNK: [f32; 2] = [4968.84, 3087.89];

/// How many ticks before the original's the port's wreck comes. All of it is the launch (P154): from
/// the same state and pad the port's 125 is 3.3 ft/s quicker by 2920 and reaches the trunk 4 ticks
/// early, at 57.4 ft/s against the original's 58.2. Zero once the launch is the original's.
const S100_EARLY: u64 = 4;

/// **A free09 trunk wrecks the rider as the original's does** (S126): S100's ride replayed in the
/// port from its own state — the whole bike seeded, free09's ground, props and every trunk of its
/// ecosystem standing, the bike's body rays carried, the original's pad — wrecks on the tick the
/// original does, over the bars (reason 1), on the impact load the resolve of the rays' trunk
/// contacts leaves in g_peak.
#[test]
fn a_free09_trunk_wrecks_as_the_original_does() {
    use game_sim::camera::CurveTable;
    use game_sim::eco::{Trees, Wooded};
    let dir = private_dir("capture");
    let Ok(text) = std::fs::read_to_string(dir.join(format!("{S100_TREE}.csv"))) else {
        println!("no {S100_TREE}.csv — S100's journal says how it is recorded");
        return;
    };
    let whole = Capture::parse(&text).unwrap_or_else(|e| panic!("{S100_TREE}.csv: {e}"));
    let at = dir.join(format!("{S100_TREE}.ee.bin"));
    let Some(st) = SaveState::open_either(&at) else {
        println!("no {} — the eeMemory.bin of S100's tree.p2m2_SaveState.p2s", at.display());
        return;
    };
    let Some(mut disc) = Disc::open() else { return };
    let track = "Tracks\\free09.res";
    let Some(area) = Area::all_of(&mut disc, track).into_iter().next() else {
        println!("no {track} on this disc");
        return;
    };
    let (props, _) = area.props(&mut disc);
    let terrain = Terrain::parse(&area.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &area.scene.terrain);
    let bytes = disc.member(track).expect("free09's archive");
    let res = Res::parse(&bytes).expect("free09's archive");
    let esb = by_suffix(&res, "free09.esb").and_then(|e| res.read(&bytes, e).ok()).expect("free09's ecosystem");
    let (trees, missed) = Trees::on_ground(&esb, &ground, &CurveTable::build()).expect("free09's ecosystem");
    assert!(missed.is_empty(), "free09's trunks the port lacks: {missed:?}");
    let mut world = Wooded { world: Place { ground, props }, trees };
    let tables = Arc::new(RiderTables::build());
    let phy = the_bike_named(&mut disc, &tables, FREERIDE_PHY).expect("race125p.phy");
    let mounts = the_wheels_of(&mut disc, FREERIDE_MODEL).expect("the 125's wheels");
    let col = the_bike_col(&mut disc).expect("the bike's collision files");

    let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
    let cap = dense_run(&whole, saved);
    let tape = PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{S100_TREE}.csv: {e}"));
    let rows = rows_by_counter(&cap);
    // The recorder polls the pad two frames back; the first two rows' are the tape's first, idle
    // frame (the tape holds Cross from its second row, S100).
    let pad_of = |counter: u64| rows.get(&counter).map(|&r| tape.frames[r.saturating_sub(2)]);
    let mut rider = Rider::new(&phy, mounts, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    rider.carries_bike_col(Arc::new(col));
    let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy.clone(), rider);
    st.seed_ride(&mut sim.rider.ride).expect("the state's whole bike");
    let want = first_wreck(&cap, saved).expect("the original wrecks");
    let lane = |r: usize, n: &str| -> [f32; 3] {
        ["x", "y", "z"].map(|l| cell(&cap, r, column(&cap, &format!("{n}.{l}")).unwrap()).parse::<f32>().unwrap())
    };
    println!("from the state at frame {saved}: the original wrecks at {}, reason {}", want.0, want.1);
    let mut got = None;
    for counter in saved + 1..=saved + S100_TICKS {
        let Some(pad) = pad_of(counter) else {
            println!("  {counter}: no row in the capture");
            break;
        };
        sim.frame(pad.dt(), &pad.reports, &mut world);
        let ride = &sim.rider.ride;
        let (c, v, g) = (ride.chassis.cofg_world, ride.chassis.vel, ride.mv.g_peak);
        if counter + 8 >= want.0 && counter <= want.0 + 2 {
            if let Some(&r) = rows.get(&counter) {
                let (oc, ov, og) = (lane(r, "ch_cofg"), lane(r, "ch_vel"), lane(r, "g_peak"));
                println!("  {counter} orig cofg {:9.3} {:8.3} {:9.3} vel {:7.2} {:7.2} {:7.2} g {:6.2} {:6.2} {:6.2}",
                         oc[0], oc[1], oc[2], ov[0], ov[1], ov[2], og[0], og[1], og[2]);
            }
            println!("  {counter} port cofg {:9.3} {:8.3} {:9.3} vel {:7.2} {:7.2} {:7.2} g {:6.2} {:6.2} {:6.2}{}",
                     c.x(), c.y(), c.z(), v.x(), v.y(), v.z(), g.x(), g.y(), g.z(),
                     sim.rider.state.wreck.map_or(String::new(), |w| format!("  wreck {}", w.0)));
        } else if counter % 10 == 0 {
            if let Some(&r) = rows.get(&counter) {
                println!("  {counter} speed: original {:.2}, port {:.2}", lane(r, "ch_vel")[2], v.z());
            }
        }
        if got.is_none() && let Some(w) = sim.rider.state.wreck {
            got = Some((counter, w.0, g, c));
        }
    }
    println!("the port {}", got.map_or("rides it out".to_string(), |(c, r, g, _)| format!("wrecks at {c}, reason {r}, g_peak {g:?}")));
    let (gc, gr, g, c) = got.expect("the port wrecks on the trunk");
    let off = |x: f32, z: f32| ((x - S100_TRUNK[0]).powi(2) + (z - S100_TRUNK[1]).powi(2)).sqrt();
    let theirs = lane(rows[&want.0], "ch_cofg");
    println!("at the wreck the centre of mass is {:.2} ft off the trunk's axis, the original's {:.2}", off(c.x(), c.z()), off(theirs[0], theirs[2]));
    assert_eq!(gr, want.1, "over the bars, as the original");
    assert!(g.z() < -phy.air.front_wreck_speed_limit.0, "on the fore-aft impact load, past the 125's front limit");
    assert!((2.5..5.0).contains(&off(c.x(), c.z())), "against the trunk, nose first");
    assert_eq!(want.0.checked_sub(gc), Some(S100_EARLY), "the port's wreck, the original's at {}, comes S100_EARLY ticks before it (P154)", want.0);
}

// --- S129: the bike meeting free06's train, from S129's own save states -------------------------------
//
// Two states made from S89's along state with `research/s129/tele.py` (the bike moved, braked, and
// saved when the train's engine had its path clock at a set value), recorded at a quarter speed with
// `watch/s129-train.toml`:
//
// - **braked**: the bike braked on the z≈2319 rail at x≈3498, across it (facing +z), the engine four
//   seconds off. The original's train runs straight through the bike: every car passes over the spot,
//   and the bike does not move, is not thrown and does not wreck (`context/s129/shots` has the wagon
//   round the rider).
// - **across**: the bike 70 ft short of the rail, facing it, the train going by; the gas, into the
//   wagons' side.
//
// The cars are S81's, seeded from the same image: every path car (found by its vtable), its path, its
// numbers and its state, its collision mesh off the disc, attached to the world's props after the
// scenery's as the shell does.

/// S129's rides: `context/capture/s129/<name>.csv` and `context/states/s129-<name>.eeMemory.bin`.
const S129_RIDES: [&str; 2] = ["braked", "across"];

/// The tick each of [`S129_RIDES`] agrees through on every lane, in that order: a ratchet like
/// [`S89_AGREES_THROUGH`].
const S129_AGREES_THROUGH: [u64; 2] = [1, 147];

/// The path car's fields (`research/oracle.txt` S81; `tests/traffic.rs` has the rest).
mod s129_car {
    pub const VTABLE: u32 = 0x8;
    pub const NAME: u32 = 0x320;
    pub const NODE: u32 = 0x254;
    pub const WORLD: u32 = 0x80;
    pub const MESH: u32 = 0x428;
    pub const PATH: u32 = 0x430;
    pub const LAST: u32 = 0x450;
    pub const HEAD: u32 = 0x480;
    pub const UP: u32 = 0x4a0;
    pub const FIRST: u32 = 0x4e4;
    pub const CURSOR: u32 = 0x4f4;
    pub const TANGENTS: u32 = 0x510;
    pub const TIME_SCALE: u32 = 0x534;
    pub const LAG: u32 = 0x558;
    pub const HEADING_FILTER: u32 = 0x55c;
    pub const NORMAL_FILTER: u32 = 0x560;
    pub const KEYS: u32 = 0x8;
    pub const CLOCK: u32 = 0xc;
    pub const KEY: u32 = 0x30;
}

impl SaveState {
    /// Every path car in the image, in address order: an object whose vtable is the path car's
    /// (`path_car_vtable`) with a path and a mesh.
    fn path_cars(&self) -> Vec<u32> {
        let vtable = ps2_oracle::addr("path_car_vtable").expect("path_car_vtable in research/oracle.txt");
        let ok = |a: u32| (0x10_0000..0x200_0000).contains(&a);
        (0x10_0000u32..0x200_0000 - 0x600)
            .step_by(16)
            .filter(|&a| self.word(a + s129_car::VTABLE) == Some(vtable))
            .filter(|&a| self.word(a + s129_car::PATH).is_some_and(ok) && self.word(a + s129_car::MESH).is_some_and(ok))
            .collect()
    }

    fn mat(&self, a: u32) -> Option<Mat4> {
        Some(Mat4([self.vf(a)?, self.vf(a + 16)?, self.vf(a + 32)?, self.vf(a + 48)?]))
    }

    fn c_string(&self, a: u32) -> String {
        let bytes: Vec<u8> = (0..64)
            .map_while(|k| {
                let w = self.word((a + k) & !3)?;
                let b = (w >> (8 * ((a + k) & 3))) as u8;
                (b != 0).then_some(b)
            })
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// The image's path cars as the port's [`game_sim::traffic::Traffic`]: each car's path and numbers
    /// out of RAM, its collision mesh out of `meshes` by the name the car carries.
    fn traffic(&self, meshes: impl Fn(&str) -> Option<Vec<u8>>) -> game_sim::traffic::Traffic {
        use game_sim::traffic::{Car, Spec, Traffic};
        use s129_car as at;
        let cars = self.path_cars();
        let specs = cars
            .iter()
            .map(|&car| {
                let path = self.word(car + at::PATH).unwrap();
                let (n, keys) = (self.word(path).unwrap(), self.word(path + at::KEYS).unwrap());
                let points: Vec<(F, Vf)> =
                    (0..n).map(|k| keys + k * at::KEY).map(|k| (self.float(k).unwrap(), self.vf(k + 0x10).unwrap())).collect();
                let f = |o| self.float(car + o).unwrap();
                let col = self.c_string(car + at::NAME).to_ascii_lowercase().replace(".psg", ".col");
                let mesh = meshes(&col).map(|b| game_sim::track::Mesh::parse(&b).unwrap_or_else(|e| panic!("{col}: {e}")));
                assert!(mesh.is_some(), "no {col} in the track's archives");
                Spec::new(&points, f(at::TIME_SCALE), f(at::HEADING_FILTER), f(at::NORMAL_FILTER), f(at::LAG), mesh).expect("a path")
            })
            .collect();
        let mut traffic = Traffic::new(specs);
        let measured: Vec<[Vf; 2]> = traffic.specs.iter().map(|s| s.measured).collect();
        traffic.cars = cars
            .iter()
            .zip(measured)
            .map(|(&car, measured)| Car {
                clock: self.float(self.word(car + at::PATH).unwrap() + at::CLOCK).unwrap(),
                seg: self.word(car + at::CURSOR).unwrap(),
                point: self.vf(car + at::LAST).unwrap(),
                head: self.vf(car + at::HEAD).unwrap(),
                normal: self.vf(car + at::UP).unwrap(),
                first: self.word(car + at::FIRST).unwrap() != 0,
                pose: self.mat(self.word(car + at::NODE).unwrap() + at::WORLD).unwrap(),
                // Still on the measuring's tangents only while the cached ones are those, bit for
                // bit: the dirty word is clear on a car that has moved since as well (S146,
                // tests/traffic.rs `the_cars_drive_on_from_a_rides_state`).
                stale: [at::TANGENTS, at::TANGENTS + 0x10]
                    .iter()
                    .zip(measured)
                    .all(|(&o, m)| self.vf(car + o).unwrap().to_bits()[..3] == m.to_bits()[..3]),
                level: Default::default(),
            })
            .collect();
        traffic
    }
}

impl SaveState {
    /// Whether the bike's nearby search (`rider +0x7bc`: the list at `+0`, its count at `+4`, eight
    /// bytes a candidate) holds any of the path cars' meshes. Its wheel probe and body rays collide
    /// only against that list, and the list is gathered again only once the bike has moved out of its
    /// bound (`broad_phase::Search`), so a bike that last gathered as a ghost — whose categories leave
    /// out `0x40`, the cars', toys' and bikes' bit — and has stood since never meets a car.
    fn bike_search_holds_cars(&self) -> bool {
        const BIKE_SEARCH: u32 = 0x7bc;
        let meshes: Vec<u32> = self.path_cars().iter().map(|&c| self.word(c + s129_car::MESH).unwrap()).collect();
        let search = self.rider_at(BIKE_SEARCH).expect("the bike's search");
        let (list, count) = (self.word(search).unwrap(), self.word(search + 4).unwrap());
        (0..count).any(|k| meshes.contains(&self.word(list + 8 * k).unwrap()))
    }
}

/// [`in_the_world_on`] on free06 with the state's path cars in it: the scenery's props, then the cars'
/// meshes ([`game_sim::traffic::Traffic::attach`]) when the state's bike search holds them
/// ([`SaveState::bike_search_holds_cars`]; the cars run either way), and the traffic to put in the
/// sim. Named rather than found from the rows ([`recorded_on`]): a bike that stands still fits every
/// area's ground.
fn in_the_world_with_cars<R>(
    st: &SaveState,
    phy: &str,
    model: &str,
    then: impl FnOnce(&mut dyn World, &Where, Option<Ridden>, game_sim::traffic::Traffic) -> R,
) -> Option<R> {
    let mut disc = Disc::open()?;
    let area = Area::all_of(&mut disc, "Tracks\\free06.res").into_iter().next()?;
    let (mut props, _) = area.props(&mut disc);
    let mut parts = Vec::new();
    for name in [area.named.track.clone(), area.named.track.replace(".res", "_2.res")] {
        if let Some(bytes) = disc.member(&name) {
            if let Ok(res) = Res::parse(&bytes) {
                parts.push((bytes, res));
            }
        }
    }
    let mut traffic = st.traffic(|want| parts.iter().find_map(|(bytes, res)| by_suffix(res, want).and_then(|e| res.read(bytes, e).ok())));
    if st.bike_search_holds_cars() {
        traffic.attach(&mut props);
    }
    let terrain = Terrain::parse(&area.terrain).ok()?;
    let ground = Ground::new(&terrain, &area.scene.terrain);
    let tables = Arc::new(RiderTables::build());
    let bike = the_bike_named(&mut disc, &tables, phy).map(|phy| {
        let mounts = the_wheels_of(&mut disc, model).expect("the bike model's tire nodes");
        Ridden { tables: Arc::clone(&tables), phy, mounts }
    });
    let mut place = Place { ground, props };
    Some(then(&mut place, &area.named, bike, traffic))
}

/// **S129: the bike meeting the Dallas train, in the original and replayed.**
#[test]
fn the_bike_meets_the_dallas_train_as_the_original_does() {
    for (k, name) in S129_RIDES.iter().enumerate() {
        let path = private_dir("capture").join("s129").join(format!("{name}.csv"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("no {} — `watch/s129-train.toml` says how it is recorded", path.display());
            continue;
        };
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let image = private_dir("states").join(format!("s129-{name}.eeMemory.bin"));
        let Some(st) = SaveState::open_image(&image, &S89_RIDER) else { continue };
        let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
        let ticks = quarter_ticks(&cap, saved).unwrap_or_else(|e| panic!("{name}: {e}"));
        let inputs = save_state_inputs(&cap, &ticks, st.held_controls()).unwrap_or_else(|e| panic!("{name}: {e}"));
        in_the_world_with_cars(&st, "race250p.phy", "250_motoxxx", |world, named, bike, traffic| {
            assert_eq!(traffic.cars.len(), 21, "{name}: free06's path cars");
            let mut sim = acceptance_start(&cap, 0, bike).unwrap();
            sim.traffic = traffic;
            let ride = &mut sim.rider.ride;
            st.seed_chassis(ride).expect("the chassis");
            st.seed_wheels(ride).expect("both wheels");
            st.seed_balance(ride).expect("the balance");
            st.seed_engine_outputs(ride).expect("the engine");
            st.seed_air(ride).expect("the air state");
            if let Some(d) = st.drivetrain() {
                d.seed(ride);
            }
            st.seed_levers(ride).expect("the levers");
            st.seed_engine_state(ride).expect("the engine's state");

            let mut watches = Vec::new();
            for (col, path) in chassis_lanes() {
                let Ok(w) = cap.float(&col, &path) else { continue };
                watches.push(retick(w, &ticks, 0));
            }
            for (col, path) in slip_lanes() {
                let Ok(w) = cap.bool(&col, &path) else { continue };
                watches.push(retick(w, &ticks, 0));
            }
            for (col, wheel) in [("f", "front"), ("r", "rear")] {
                for (c, p) in [("ray_hit", "w.ray_hit"), ("swept", "w.swept_contact")] {
                    if let Ok(w) = cap.int(&format!("{col}_{c}"), &format!("rider.ride.{wheel}.{p}")) {
                        watches.push(retick(w, &ticks, 0));
                    }
                }
            }
            // A row's train is a tick ahead of its bike: the recorder reads after the next frame has
            // run its path cars and before it runs the bike, the port's order (cars, then rider).
            if let Ok(w) = cap.float("train_clock", "cars[0].clock") {
                watches.push(retick(w, &ticks, 1));
            }
            let last = *ticks.last().unwrap() as usize + 1;
            let mut pads = inputs.clone();
            pads.resize(last.max(pads.len()), Input::default());
            let f = Fixture {
                name: format!("s129 {name} from its save state"),
                seed: 0,
                start: Some(sim.clone()),
                inputs: pads.clone(),
                watches,
                world: Some(named.clone()),
                ..Default::default()
            };
            let parts = |f: &Fixture, world: &mut dyn World| match replay::verify_in(f, world) {
                Ok(_) => {
                    println!("{}: agrees on every tick", f.name);
                    None
                }
                Err(d) => {
                    println!("{}: {d}", f.name);
                    Some(d.tick)
                }
            };
            let whole = parts(&f, world);
            let mut apart = Vec::new();
            for w in &f.watches {
                let one = Fixture { name: format!("  {}", w.path), watches: vec![w.clone()], hashes: Vec::new(), ..f.clone() };
                if parts(&one, world).is_some() {
                    apart.push(w.path.clone());
                }
            }
            // The train runs as the original's in both, every tick. Braked, the bike's search holds no
            // car (the_parked_bike_never_gathers_the_train) and the port, with none in its scene, keeps
            // the original's place, wheel contacts and ground under the whole train: only the creep the
            // original has at tick 2 (a few thousandths of a foot a second, not the port's) parts it.
            let must_agree: &[&str] = match *name {
                "braked" => &[
                    "cars[0].clock",
                    "rider.ride.chassis.cofg[0]",
                    "rider.ride.chassis.cofg[1]",
                    "rider.ride.chassis.cofg[2]",
                    "rider.ride.front.w.ground_normal[1]",
                    "rider.ride.rear.w.ground_normal[1]",
                    "rider.ride.front.w.ray_hit",
                    "rider.ride.front.w.swept_contact",
                    "rider.ride.rear.w.ray_hit",
                    "rider.ride.rear.w.swept_contact",
                ],
                _ => &["cars[0].clock"],
            };
            for path in must_agree {
                assert!(f.watches.iter().any(|w| w.path == *path), "{name}: no {path} lane");
                assert!(!apart.iter().any(|p| p == path), "{name}: {path} parts from the original");
            }

            // Where each was, every ten ticks.
            let col = |c: &str| column(&cap, c).unwrap_or_else(|e| panic!("{e}"));
            let num = |r: usize, c: &str| -> f32 { cell(&cap, r, col(c)).parse().unwrap_or(f32::NAN) };
            if *name == "braked" {
                // The original's bike under the train: never shoved (it creeps 0.18 ft down the bed over
                // the twelve seconds, braked, the port's cofg with it bit for bit), never down.
                for r in 0..cap.rows.len() {
                    for c in ["ch_cofg.x", "ch_cofg.y", "ch_cofg.z"] {
                        assert!((num(r, c) - num(0, c)).abs() < 0.25, "braked row {r}: the original's bike moved on {c}");
                    }
                    assert_eq!(num(r, "wreck_reason"), 0.0, "braked row {r}: the original's bike went down");
                }
            }
            let mut port = sim.clone();
            let mut at = Vec::with_capacity(last);
            let mut wrecked = None;
            for t in 1..last {
                let frame = port.tick_in(pads[t - 1], world);
                if wrecked.is_none() {
                    wrecked = frame.wreck.map(|w| (t, w));
                }
                let c = port.rider.ride.chassis.cofg_world;
                at.push((t as u64, [c.x(), c.y(), c.z()], wrecked.is_some(), port.traffic.cars[0].pose.0[3].x()));
            }
            println!("{name}: the port's first wreck {wrecked:?}");
            for (r, &t) in ticks.iter().enumerate().step_by(10) {
                if let Some((_, p, wrecked, engine)) = at.iter().find(|(pt, ..)| *pt == t) {
                    println!(
                        "{name} tick {t:4}: original cofg ({:.2}, {:.2}, {:.2}) wreck {} engine x {:.2} | port ({:.2}, {:.2}, {:.2}) wrecked {wrecked} engine x {engine:.2}",
                        num(r, "ch_cofg.x"), num(r, "ch_cofg.y"), num(r, "ch_cofg.z"), num(r, "wreck_reason"), num(r, "train_pos.x"), p[0], p[1], p[2]
                    );
                }
            }
            match whole {
                Some(t) if t <= S129_AGREES_THROUGH[k] => panic!(
                    "{name} parts from the original at tick {t}, at or before S129_AGREES_THROUGH[{k}] = {}: a regression",
                    S129_AGREES_THROUGH[k]
                ),
                Some(t) if t > S129_AGREES_THROUGH[k] + 1 => println!("{name} now agrees through tick {}: raise S129_AGREES_THROUGH[{k}]", t - 1),
                None => println!("{name} agrees on every tick: raise S129_AGREES_THROUGH[{k}]"),
                _ => {}
            }
        });
    }
}

/// **S129: why the braked bike never meets the train.** The bike's wheel probe and body rays collide
/// only against its nearby search's list ([`SaveState::bike_search_holds_cars`]), and in the braked
/// state that list was last gathered while the bike was a ghost: it holds the scenery, a placed group
/// and the race gates, and nothing whose category has the ghost's `0x40` (the cars, toys and bikes).
/// The bike stands, so the list is never gathered again and the train runs through it. In the across
/// state the list holds every path car. Then, from the braked image saved at frame 16344 (the front
/// wheel's ray inside the second wagon's side): the port's wheel probe, built from the image's own
/// wheels over the scenery alone, records what the original's did — the rail under both rays, its
/// face's normal bit for bit (with the wagons in the scene it records the wagon's side instead).
#[test]
fn the_parked_bike_never_gathers_the_train() {
    const GHOST: u32 = 0x40;
    for (name, holds) in [("braked", false), ("braked-12", false), ("across", true)] {
        let image = private_dir("states").join(format!("s129-{name}.eeMemory.bin"));
        let Some(st) = SaveState::open_image(&image, &S89_RIDER) else { continue };
        assert_eq!(st.bike_search_holds_cars(), holds, "{name}: the path cars in the bike's search");
        let search = st.rider_at(0x7bc).unwrap();
        let (list, count) = (st.word(search).unwrap(), st.word(search + 4).unwrap());
        let ghostly = (0..count).filter(|&k| st.word(st.word(list + 8 * k).unwrap() + 0x84).unwrap() & GHOST != 0).count();
        println!("{name}: {count} candidates, {ghostly} of them with the ghost's category");
        assert_eq!(ghostly == 0, !holds, "{name}: candidates in the ghost's category");
    }

    let image = private_dir("states").join("s129-braked-12.eeMemory.bin");
    let Some(st) = SaveState::open_image(&image, &S89_RIDER) else { return };
    let path = private_dir("capture").join("s129").join("braked.csv");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let cap = Capture::parse(&text).unwrap();
    let original = st.wheel_record();
    assert_eq!(original.len(), 2, "both rays struck in the original");
    in_the_world_with_cars(&st, "race250p.phy", "250_motoxxx", |world, _named, bike, _traffic| {
        let mut sim = acceptance_start(&cap, 0, bike).unwrap();
        let ride = &mut sim.rider.ride;
        st.seed_chassis(ride).unwrap();
        st.seed_wheels(ride).unwrap();
        let rays = game_sim::ride::wheel_rays(&ride.front.w, &ride.rear.w);
        assert_eq!(bits_of_rays(&rays), bits_of_rays(&st.wheel_probe().unwrap()), "the wheel probe");
        let node = game_sim::rider_contact::node(&ride.transform());
        let (field, props, trees) = world.wooded();
        let bounds = field.as_ref().map(|f| {
            let (lo, hi) = (f.min(), f.max());
            ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
        });
        let mut search = game_sim::broad_phase::Search { least: F(30.0), extra: F(50.0), capacity: 256, ..Default::default() };
        let mut scene = game_sim::flight_search::Scene::wooded(props, trees, None, bounds, &rays, &node, &mut search);
        let _ = game_sim::contact_search::Search::rays(&mut scene, true);
        let port = &scene.ray_record().contacts;
        assert_eq!(port.len(), original.len(), "the wheel probe's contacts at frame 16344");
        for (c, (point, normal, fraction)) in port.iter().zip(&original) {
            println!("ray {}: port {:?} {:?} {:?}, original {point:?} {normal:?} {fraction:?}", c.segment, c.point, c.normal, c.fraction);
            // The normal is the rail's face, bit for bit; the point and the fraction a few ulps apart,
            // the probe having run from the substep's pose and not the frame's end the image holds.
            assert_eq!([c.normal.x(), c.normal.y(), c.normal.z()].map(f32::to_bits), [normal.x(), normal.y(), normal.z()].map(f32::to_bits));
            assert!([c.point.x() - point.x(), c.point.y() - point.y(), c.point.z() - point.z()].iter().all(|d| d.abs() < 0.001) && (c.fraction.0 - fraction.0).abs() < 0.001, "ray {}", c.segment);
        }
    });
}

// --- S146: a bike parked on the Dallas train's roof, in the original and replayed ----------------------
//
// `research/s146/roof.py` on S129's along state (copy 2's slot 8): both brakes held, the bike moved 2.5 ft
// over the middle of the tenth path car's roof (S81's order; `train_car_06`, the train's second boxcar)
// with the chassis at the wagon's own velocity, ridden there 30 frames and saved (slot 13). Recorded from
// that state at a quarter speed with `watch/s129-train.toml` and S129's braked pad:
//
//   ./kit pcsx2 with -- ./kit record watch/s129-train.toml --state 13 --pad watch/pads/s129-braked.pad \
//       -n 270 -o context/capture/s146/roof.csv
//
// The state's RAM is `context/states/s146-roof.eeMemory.bin`; its bike search holds all the train's cars.

/// The path car the bike rides in S146's state, by its place among the image's path cars (address order,
/// [`SaveState::path_cars`]).
const S146_WAGON: usize = 9;

/// The tick each row's *bike* is at, where [`quarter_ticks`] numbers a row by its frame. The recorder
/// mostly reads between a frame's path cars and its bike (S129), but now and then after the bike as
/// well: such a row's bike is a tick ahead of its frame's, until a row that repeats it. S146's roof ride
/// has three such runs (rows 161-162, 179, 198 and 211: the original's row 161 is the port's tick 162,
/// lane for lane), and nothing in a row says which it is. The bike there rides the train at a steady
/// 55 ft/s, so its own step says how many ticks lie between two rows: the change in `ch_cofg.x` over
/// the row before's `ch_vel.x` × `dt`, rounded. Each row's tick is then within one of its frame's.
fn bike_ticks(cap: &Capture, ticks: &[u64]) -> Vec<u64> {
    let col = |c: &str| column(cap, c).unwrap_or_else(|e| panic!("{e}"));
    let num = |r: usize, c: &str| -> f64 { cell(cap, r, col(c)).parse().unwrap() };
    let mut out = ticks.to_vec();
    for r in 1..out.len() {
        let step = num(r - 1, "ch_vel.x") * num(r - 1, "dt");
        let n = ((num(r, "ch_cofg.x") - num(r - 1, "ch_cofg.x")) / step).round();
        assert!((0.0..=2.0).contains(&n), "row {r}: {n} ticks after the row before");
        out[r] = out[r - 1] + n as u64;
        assert!(out[r] == ticks[r] || out[r] == ticks[r] + 1, "row {r}: the bike at tick {}, its frame's {}", out[r], ticks[r]);
    }
    out
}

/// `w` with every sample after the first on its tick dropped: a row that repeats the next one's bike.
fn first_on_each_tick(mut w: Watch) -> Watch {
    w.samples.dedup_by_key(|s| s.tick);
    w
}

/// **S146: a braked bike on the moving train's roof, in the original and replayed.** The original's
/// bike stays where it was put on the wagon. The port is replayed from the same image among the same
/// cars, lane by lane, and both bikes' places on the deck (along it, over it) are put side by side.
#[test]
fn a_bike_parked_on_the_trains_roof_rides_it_as_the_original_does() {
    let path = private_dir("capture").join("s146").join("roof.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("no {} — `research/s146/roof.py` says how it is made", path.display());
        return;
    };
    let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let image = private_dir("states").join("s146-roof.eeMemory.bin");
    let Some(st) = SaveState::open_image(&image, &S89_RIDER) else { return };
    assert!(st.bike_search_holds_cars(), "the roof state's bike search holds the cars");
    let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
    let ticks = quarter_ticks(&cap, saved).unwrap_or_else(|e| panic!("roof: {e}"));
    let inputs = save_state_inputs(&cap, &ticks, st.held_controls()).unwrap_or_else(|e| panic!("roof: {e}"));
    let bike = bike_ticks(&cap, &ticks);
    in_the_world_with_cars(&st, "race250p.phy", "250_motoxxx", |world, named, ridden, traffic| {
        assert_eq!(traffic.cars.len(), 21, "free06's path cars");
        let mut sim = acceptance_start(&cap, 0, ridden).unwrap();
        sim.traffic = traffic;
        let ride = &mut sim.rider.ride;
        st.seed_chassis(ride).expect("the chassis");
        st.seed_wheels(ride).expect("both wheels");
        st.seed_balance(ride).expect("the balance");
        st.seed_engine_outputs(ride).expect("the engine");
        st.seed_air(ride).expect("the air state");
        if let Some(d) = st.drivetrain() {
            d.seed(ride);
        }
        st.seed_levers(ride).expect("the levers");
        st.seed_engine_state(ride).expect("the engine's state");

        let mut watches = Vec::new();
        for (col, path) in chassis_lanes() {
            let Ok(w) = cap.float(&col, &path) else { continue };
            watches.push(first_on_each_tick(retick(w, &bike, 0)));
        }
        for (col, path) in slip_lanes() {
            let Ok(w) = cap.bool(&col, &path) else { continue };
            watches.push(first_on_each_tick(retick(w, &bike, 0)));
        }
        for (col, wheel) in [("f", "front"), ("r", "rear")] {
            for (c, p) in [("ray_hit", "w.ray_hit"), ("swept", "w.swept_contact")] {
                if let Ok(w) = cap.int(&format!("{col}_{c}"), &format!("rider.ride.{wheel}.{p}")) {
                    watches.push(first_on_each_tick(retick(w, &bike, 0)));
                }
            }
        }
        // A row's train is a tick ahead of its bike (S129: the cars run before the bike in a frame).
        if let Ok(w) = cap.float("train_clock", "cars[0].clock") {
            watches.push(retick(w, &ticks, 1));
        }
        let last = *ticks.last().unwrap() as usize + 1;
        let mut pads = inputs.clone();
        pads.resize(last.max(pads.len()), Input::default());
        let f = Fixture {
            name: "s146 roof from its save state".into(),
            seed: 0,
            start: Some(sim.clone()),
            inputs: pads.clone(),
            watches,
            world: Some(named.clone()),
            ..Default::default()
        };
        let parts = |f: &Fixture, world: &mut dyn World| match replay::verify_in(f, world) {
            Ok(_) => {
                println!("{}: agrees on every tick", f.name);
                None
            }
            Err(d) => {
                println!("{}: {d}", f.name);
                Some(d.tick)
            }
        };
        let whole = parts(&f, world);
        let mut apart = Vec::new();
        for w in &f.watches {
            let one = Fixture { name: format!("  {}", w.path), watches: vec![w.clone()], hashes: Vec::new(), ..f.clone() };
            if parts(&one, world).is_some() {
                apart.push(w.path.clone());
            }
        }
        assert!(!apart.iter().any(|p| p == "cars[0].clock"), "the train parts from the original");

        // Each bike's place on the wagon's deck, in the wagon's own frame: the port's wagon for both,
        // its clock being the original's on every tick (the lane above).
        let col = |c: &str| column(&cap, c).unwrap_or_else(|e| panic!("{e}"));
        let num = |r: usize, c: &str| -> f32 { cell(&cap, r, col(c)).parse().unwrap_or(f32::NAN) };
        let mut port = sim.clone();
        let mut on_deck = Vec::with_capacity(last);
        let mut wrecked = None;
        for t in 1..last {
            let frame = port.tick_in(pads[t - 1], world);
            if wrecked.is_none() {
                wrecked = frame.wreck.map(|w| (t, w));
            }
            let pose = port.traffic.cars[S146_WAGON].pose;
            let deck = move |p: [f32; 3]| {
                let d = Vf::new(p[0] - pose.0[3].x(), p[1] - pose.0[3].y(), p[2] - pose.0[3].z(), 0.0);
                game_sim::chassis::unrotate(&game_sim::chassis::Mat3::new(pose.0[0], pose.0[1], pose.0[2]), d)
            };
            let c = port.rider.ride.chassis.cofg_world;
            let v = port.rider.ride.chassis.vel;
            on_deck.push((t as u64, deck([c.x(), c.y(), c.z()]), v.x(), deck));
        }
        println!("roof: the port's first wreck {wrecked:?}");
        let mut original_along = (f32::MAX, f32::MIN);
        let mut port_along = (f32::MAX, f32::MIN);
        for (r, &t) in bike.iter().enumerate() {
            let Some((_, p, pv, deck)) = on_deck.iter().find(|(pt, ..)| *pt == t) else { continue };
            let o = deck([num(r, "ch_cofg.x"), num(r, "ch_cofg.y"), num(r, "ch_cofg.z")]);
            original_along = (original_along.0.min(o.z()), original_along.1.max(o.z()));
            port_along = (port_along.0.min(p.z()), port_along.1.max(p.z()));
            if r % 15 == 0 || r + 1 == bike.len() {
                println!(
                    "roof tick {t:4}: original on deck ({:6.2}, {:5.2}, {:6.2}) vel.x {:6.2} wreck {} | port ({:6.2}, {:5.2}, {:6.2}) vel.x {pv:6.2}",
                    o.x(), o.y(), o.z(), num(r, "ch_vel.x"), num(r, "wreck_reason"), p.x(), p.y(), p.z()
                );
            }
            assert_eq!(num(r, "wreck_reason"), 0.0, "roof row {r}: the original's bike went down");
        }
        println!("roof: along the deck the original ranged {original_along:?}, the port {port_along:?}");
        // Parked and carried: the original's bike creeps back along the deck, ~1.5 ft in nine
        // seconds (about 0.2 ft/s, not S136's stock bike's 8 ft/s), and never leaves it.
        assert!(original_along.0 > -3.0 && original_along.1 < 3.0, "the original's bike slid {original_along:?} along the deck");
        assert_eq!(whole, None, "the roof ride parts from the original");

        // The control: the same ride with the bike's air drag off. The drag reads the bike's speed
        // over the world (55 ft/s here, wagon or no wagon), and the original's bits have it in them.
        let drag = sim.phy.mv.drag;
        assert!(drag > F::ZERO, "the 250's drag is {}", drag.0);
        let mut still = sim.clone();
        Arc::make_mut(&mut still.phy).mv.drag = F::ZERO;
        let control = Fixture { name: "s146 roof without the air drag".into(), start: Some(still), hashes: Vec::new(), ..f.clone() };
        let parted = parts(&control, world);
        println!("roof: the 250's drag {}; without it the ride parts at {parted:?}", drag.0);
        assert!(parted.is_some(), "the roof ride agrees without the air drag too");
    });
}

// --- S162: an idle bike rolling off on free06, from S162's own save states --------------------------
//
// Two states made from S89's along state with `research/s162/turn.py` (the bike teleported onto a spot
// of S153's idle sweep, ridden round to the sweep's heading, put back on the spot, braked and saved),
// recorded at a quarter speed with `watch/s89-rails.toml` and nothing on the pad
// (`research/s162/idle.pad`):
//
// - **bump**: 3999.8,4000.3 facing 167°, up free06's slope south of the bump at about 3940,4097. The
//   bike rolls back down the slope, tail first, to 40 ft/s and into the bump, and the original bails
//   the rider Back (4).
// - **prop**: 2500.6,3099.7 facing 72°, beside a prop; the bike rolls off forwards to 7 ft/s, slows and
//   stops 32 ft on. The original never wrecks.

/// S162's rides: `context/capture/s162/<name>.csv` and `context/states/s162-<name>.eeMemory.bin`, what
/// the original does in each — its first wreck's tick and reason, or none — and how far, in feet, the
/// port's centre of mass may stray from the original's while both ride.
const S162_RIDES: [(&str, Option<(u64, u8)>, f32); 2] = [("bump", Some((206, 4)), 2.0), ("prop", None, 0.01)];

/// **S162: an idle bike rolls off and wrecks, or does not, as the original's does**: each of
/// [`S162_RIDES`] ridden by the port from the original's save state (its whole bike, the 250, wrecks
/// on and the body rays carried) on free06 with its props and path cars and the original's pad, and the
/// first wreck held to the capture's: the same reason within a tick, or none (S138's rule). Skipped with
/// a line where a capture or state is missing.
#[test]
fn idle_roll_offs_on_free06_wreck_as_the_original_does() {
    let mut parted = Vec::new();
    for (name, recorded, limit) in S162_RIDES {
        let path = private_dir("capture").join("s162").join(format!("{name}.csv"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            println!("no {} — `research/s162/turn.py` says how it is recorded", path.display());
            continue;
        };
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let image = private_dir("states").join(format!("s162-{name}.eeMemory.bin"));
        let Some(st) = SaveState::open_image(&image, &S89_RIDER) else {
            println!("no {} with the human's rider in it", image.display());
            continue;
        };
        let saved = u64::from(st.word(SaveState::FRAME_COUNTER).unwrap());
        let ticks = quarter_ticks(&cap, saved).unwrap_or_else(|e| panic!("{name}: {e}"));
        let inputs = save_state_inputs(&cap, &ticks, st.held_controls()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let col = |c: &str| column(&cap, c).unwrap_or_else(|e| panic!("{e}"));
        let num = |r: usize, c: &str| -> f32 { cell(&cap, r, col(c)).parse().unwrap_or(f32::NAN) };
        let want = (0..ticks.len()).find(|&r| num(r, "wreck_reason") != 0.0).map(|r| (ticks[r], num(r, "wreck_reason") as u8));
        assert_eq!(want, recorded, "{name}: the original's first wreck (tick, reason)");
        let Some(bike_col) = Disc::open().and_then(|mut disc| the_bike_col(&mut disc)) else {
            println!("no bike collision files on this disc");
            continue;
        };
        in_the_world_with_cars(&st, "race250p.phy", "250_motoxxx", |world, _, bike, traffic| {
            let mut sim = acceptance_start(&cap, 0, bike).unwrap();
            sim.traffic = traffic;
            sim.rider.sense.wrecks_enabled = true;
            sim.rider.carries_bike_col(Arc::new(bike_col));
            let ride = &mut sim.rider.ride;
            st.seed_chassis(ride).expect("the chassis");
            st.seed_wheels(ride).expect("both wheels");
            st.seed_balance(ride).expect("the balance");
            st.seed_engine_outputs(ride).expect("the engine");
            st.seed_air(ride).expect("the air state");
            if let Some(d) = st.drivetrain() {
                d.seed(ride);
            }
            st.seed_levers(ride).expect("the levers");
            st.seed_engine_state(ride).expect("the engine's state");

            let last = *ticks.last().unwrap() as usize + 1;
            let mut pads = inputs.clone();
            pads.resize(last.max(pads.len()), Input::default());
            let mut got = None;
            let mut at = Vec::with_capacity(last);
            for t in 1..last {
                sim.tick_in(pads[t - 1], world);
                if got.is_none() && let Some(w) = sim.rider.state.wreck {
                    got = Some((t as u64, w.0));
                }
                let (c, v) = (sim.rider.ride.chassis.cofg_world, sim.rider.ride.chassis.vel);
                at.push((t as u64, [c.x(), c.y(), c.z()], [v.x(), v.y(), v.z()]));
            }
            for (r, &t) in ticks.iter().enumerate() {
                let near = want.is_some_and(|(w, _)| t + 3 >= w && t <= w + 1) || got.is_some_and(|(g, _)| t + 3 >= g && t <= g + 1);
                if r % 25 != 0 && !near {
                    continue;
                }
                let Some((_, p, v)) = at.iter().find(|(pt, ..)| *pt == t) else { continue };
                println!(
                    "{name} tick {t:4}: original cofg ({:8.2}, {:7.2}, {:8.2}) vel ({:6.2}, {:6.2}, {:6.2}) wreck {} | port ({:8.2}, {:7.2}, {:8.2}) vel ({:6.2}, {:6.2}, {:6.2})",
                    num(r, "ch_cofg.x"), num(r, "ch_cofg.y"), num(r, "ch_cofg.z"), num(r, "ch_vel.x"), num(r, "ch_vel.y"), num(r, "ch_vel.z"),
                    num(r, "wreck_reason"), p[0], p[1], p[2], v[0], v[1], v[2]
                );
            }
            // How far apart the two centres of mass get while both ride.
            let riding = want.map_or(u64::MAX, |w| w.0).min(got.map_or(u64::MAX, |g| g.0));
            let gaps: Vec<(u64, f32)> = ticks
                .iter()
                .enumerate()
                .filter(|&(_, &t)| t < riding)
                .filter_map(|(r, &t)| {
                    let (_, p, _) = at.iter().find(|(pt, ..)| *pt == t)?;
                    let o = [num(r, "ch_cofg.x"), num(r, "ch_cofg.y"), num(r, "ch_cofg.z")];
                    Some((t, (0..3).map(|k| (p[k] - o[k]).powi(2)).sum::<f32>().sqrt()))
                })
                .collect();
            let apart = gaps.iter().map(|g| g.1).fold(0.0, f32::max);
            let first = gaps.iter().find(|g| g.1 > 0.001).map(|g| g.0);
            println!("{name}: the original {want:?}, the port {got:?} (tick, reason); at most {apart:.3} ft apart while both ride, \
                      first more than a thousandth at tick {first:?}");
            if apart > limit {
                parted.push(format!("{name}: the port's bike strays {apart:.3} ft from the original's, over {limit}"));
            }
            let agrees = match (want, got) {
                (None, None) => true,
                (Some((wt, wr)), Some((gt, gr))) => gr == wr && gt.abs_diff(wt) <= 1,
                _ => false,
            };
            if !agrees {
                parted.push(format!("{name}: the original {want:?}, the port {got:?} (tick, reason)"));
            }
        });
    }
    assert!(parted.is_empty(), "{}", parted.join("\n"));
}
