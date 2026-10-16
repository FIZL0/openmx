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
//!    point of the module: `tick 413: ball.vel[1] -58.94952 vs -58.949516`.
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

use game_sim::hash::Leaf;
use game_sim::input::{Control, Report, CONTROL_MAP, CONTROLS as PORT_CONTROLS};
use game_sim::motion::{Clips, Driver, Params, Pose, Request};
use game_sim::replay::{
    self, Capture, Fixture, PadFrame, PadTape, Playback, Reason, Sample, Value, Watch,
};
use game_sim::trick::Trick;
use game_sim::{Input, Sim};
use ps2_float::{F, Vf};

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

/// The fields a fixture of this simulation watches: one lane of each vector and the scalar, which is
/// what a capture of the original would give — a handful of columns, not the whole state.
const WATCHED: [&str; 4] = ["ball.pos[1]", "ball.vel[1]", "ball.spin", "ticks"];

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
    let cases: [Case; 3] = [
        ("ball.pos[3], a lane the simulation ignores", 1, |s| {
            s.ball.pos = Vf([0.0, 0.0, 0.0, 1.0]);
        }),
        ("the RNG word", 1, |s| s.rng ^= 1),
        ("ball.spin, which is watched", 0, |s| {
            s.ball.spin = F::from_bits(s.ball.spin.to_bits() ^ 1);
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
/// tick 413: ball.vel[1] -58.94952 (0xc26bcc4f) vs -58.949516 (0xc26bcc4e)
/// ```
#[test]
fn a_watched_field_fails_as_one_readable_line() {
    let mut f = recorded(600);
    let w = f.watches.iter_mut().find(|w| w.path == "ball.vel[1]").expect("the watch is there");
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
        format!("tick 413: ball.vel[1] {want} vs {}", Leaf::Float(bits)),
        "the tick, the path, the recording's value, then the port's",
    );
    assert!(line.starts_with("tick 413: ball.vel[1] "), "{line}");
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
    let w = f.watches.iter_mut().find(|w| w.path == "ball.spin").unwrap();
    let Value::Float(bits) = w.samples[414].value else { panic!() };
    w.samples[414].value = Value::Float(bits ^ 1);

    let d = replay::verify(&f).expect_err("both checks fail at tick 414");
    assert_eq!(d.tick, 414);
    assert!(matches!(&d.reason, Reason::Field { path, .. } if path == "ball.spin"), "{d}");
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
    let e = replay::record("typo", 1, &inputs(4), &["ball.vel[9]"]).expect_err("a typo must not record");
    assert!(matches!(e, Reason::NoSuchField { .. }), "{e:?}");
}

#[test]
fn a_watch_of_the_wrong_kind_fails() {
    let mut f = recorded(10);
    f.watches.push(Watch::new("ball.spin", vec![Sample { tick: 2, value: Value::Uint(0) }]));
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
         watch 2 ball.vel[1] float 2.7\n\
         watch 1 ball.vel[1] float 0x40366666\n\
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
    // 0x40366666 is the port's own vel.y after one frame of gravity, written as the pattern, so tick 1
    // agrees exactly. Tick 2 is written as a decimal, and that is the whole argument for patterns: the
    // port is at 2.6999998 (0x402ccccc) and `2.7` reads as 0x402ccccd, one bit away.
    let d = replay::verify(&f).expect_err("2.7 is not the port's value to the last bit");
    assert_eq!(d.tick, 2);
    assert!(matches!(&d.reason, Reason::Field { path, .. } if path == "ball.vel[1]"), "{d}");
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
        ("seed 1\nwatch 3 ball.spin float\n", 2, "watch wants"),
        ("seed 1\nwatch 3 ball.spin quux 1\n", 2, "is not a value"),
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
        "frame,ball_pos.y,ball_spin,gear,flags\n\
         101234,{},{},0,0x00\n\
         101235,{},{},1,0x01\n\
         101236,,{},2,0x02\n\
         101237,{},{},3,0x03\n",
        lane("ball.pos[1]", 0),
        lane("ball.spin", 0),
        lane("ball.pos[1]", 1),
        lane("ball.spin", 1),
        lane("ball.spin", 2),
        lane("ball.pos[1]", 3),
        lane("ball.spin", 3),
    );

    let cap = Capture::parse(&csv).expect("the recorder's format");
    assert_eq!(cap.columns.len(), 5);
    assert_eq!(cap.frames, vec![101234, 101235, 101236, 101237]);
    assert_eq!(cap.tick_of(0), 0, "the first row is the state the run starts from");
    assert_eq!(cap.span(), 3);

    let pos = cap.float("ball_pos.y", "ball.pos[1]").unwrap();
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
            cap.float("ball_spin", "ball.spin").unwrap(),
            cap.uint("flags", "ticks").unwrap(), // a hex column, read as the tick counter it mirrors
        ],
        ..Default::default()
    };
    assert_eq!(f.checks(), 3 + 4 + 4);
    replay::verify(&f).expect("the port agrees with a capture of its own numbers");

    // And one wrong cell fails at that row's tick, naming the field.
    let bad = csv.replace("101236,,", "101236,1.0,");
    let mut f2 = f.clone();
    f2.watches[0] = Capture::parse(&bad).unwrap().float("ball_pos.y", "ball.pos[1]").unwrap();
    let d = replay::verify(&f2).expect_err("1.0 is not where the ball is at tick 2");
    assert_eq!(d.tick, 2);
    assert!(d.to_string().starts_with("tick 2: ball.pos[1] 1 "), "{d}");
}

/// A sparse capture — `every = 4` in the watch spec — maps its frame counter to ticks, so the port is
/// compared on the frames that were sampled and simulated through the rest.
#[test]
fn a_sparse_capture_checks_the_frames_it_sampled() {
    let port = recorded(40);
    let spin = port.watches.iter().find(|w| w.path == "ball.spin").unwrap();
    let mut csv = String::from("frame,spin\n");
    for tick in (0..=40).step_by(4) {
        let Value::Float(bits) = spin.samples[tick].value else { panic!() };
        csv += &format!("{},{}\n", 900 + tick, f32::from_bits(bits));
    }
    let cap = Capture::parse(&csv).unwrap();
    let f = Fixture {
        seed: port.seed,
        inputs: port.inputs.clone(),
        watches: vec![cap.float("spin", "ball.spin").unwrap()],
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
    let e = cap.float("pos.x", "ball.pos[0]").unwrap_err();
    assert_eq!(e.line, 2);
    assert!(e.to_string().contains("not float"), "{e}");
    let e = cap.float("pos.z", "ball.pos[2]").unwrap_err();
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
    reference.ball.spin = ps2_float::F::from_bits(mine.ball.spin.to_bits() ^ 1);
    let why = game_sim::hash::first_difference(&reference, &mine).expect("they differ");
    assert_eq!(why.path(), "ball.spin");
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
//  3. **The recording itself**, which is game data and therefore not in this repository, and which does
//     not exist yet — see this task's journal entry and `plan/HUMAN.md`. `the_acceptance_run` is where
//     it lands: drop the capture in `context/capture/acceptance-freeride.csv` and that test is the
//     port's verdict, with no code to write.
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
/// **These paths do not exist in the port yet.** The state is still A1's example ball, so the verdict
/// of a real acceptance capture today is `NoSuchField` at tick 0 — the fixture's own fault in A2's
/// taxonomy, and the correct answer: the port has no bike to compare. They are written here as the
/// names the ported actor is expected to walk as (S3's movement, S10's air), so that landing the
/// recording and landing the physics are separate steps.
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
        inputs,
        watches,
        ..Default::default()
    })
}

/// The eight lanes a capture of the original carries, as the port's own example state names them. Used
/// to build a synthetic capture out of the port's own numbers: a fixture built from that one can
/// actually be replayed, which is how the plumbing above is proved end to end while the ported bike
/// does not exist yet.
const AS_THE_PORT_HAS_THEM: [&str; 8] = [
    "ball.pos[0]",
    "ball.pos[1]",
    "ball.pos[2]",
    "ball.pos[3]",
    "ball.vel[0]",
    "ball.vel[1]",
    "ball.vel[2]",
    "ball.vel[3]",
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
/// the port's own run of `frames` frames, so a fixture built from this capture and aimed at
/// [`AS_THE_PORT_HAS_THEM`] replays exactly. Half **b** carries the frame's own input and half **a**
/// the next frame's, with `stamp_a = stamp_b + 1`, which is what the recorder sees: the game read the
/// older half and the IOP has already refreshed the newer one.
fn acceptance_csv(frames: usize, input: impl Fn(usize) -> (u16, (u8, u8))) -> String {
    let port = replay::record("port", 0x1234_5678, &inputs(frames), &AS_THE_PORT_HAS_THEM)
        .expect("the port's own lanes");
    let lane = |n: usize, tick: usize| match port.watches[n].samples[tick].value {
        Value::Float(bits) => format!("{}", f32::from_bits(bits)),
        v => panic!("{v} is not a float"),
    };
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
/// per frame — and, aimed at lanes the port does have, replays exactly.
///
/// This is the end-to-end proof of the reader: the columns of a capture in the recorder's own shape
/// become inputs, a seed and watches, and the watches line up tick for tick with a run of the
/// simulation. What it cannot prove is that the *inputs* line up, because `Sim::tick` ignores its
/// input today — see this task's journal entry.
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

    // And the same capture, aimed at the lanes this port does have, replays tick-exact: the watches
    // are the port's own numbers, so a divergence here is the harness or the reader, never the game.
    let mut plumbing = f.clone();
    for (watch, path) in plumbing.watches.iter_mut().zip(AS_THE_PORT_HAS_THEM) {
        watch.path = path.to_string();
    }
    replay::verify(&plumbing).expect("the reader's ticks line up with the simulation's");
}

/// Today's verdict, in one line: the port has no bike, so an acceptance capture names the first field
/// it is missing and stops at tick 0.
///
/// When S3 lands the ported movement this test fails, and that is the point — the failure is the
/// moment the acceptance run becomes a real comparison, and whoever lands it owns updating this.
#[test]
fn the_acceptance_fixture_names_the_field_the_port_is_missing() {
    let cap = Capture::parse(&acceptance_csv(60, played)).unwrap();
    let f = acceptance_fixture(&cap).unwrap();
    let d = replay::verify(&f).expect_err("the port has no bike yet");
    assert_eq!(d.tick, 0);
    assert_eq!(d.reason, Reason::NoSuchField { path: "bike.pos[0]".to_string() });
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
    let raw = cap.float("bike_vel.y", "ball.vel[1]").unwrap();
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
    assert_eq!(f.watches[0].path, "ball.pos[1]");
    f.watches[0].samples[deep].value = Value::Float(bits ^ 1); // one bit, the last one

    let d = replay::verify(&f).expect_err("one bit, eight thousand ticks in");
    assert_eq!(d.tick, deep as u64);
    assert!(d.to_string().starts_with("tick 8765: ball.pos[1] "), "{d}");

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
/// - `context/capture/acceptance-freeride.csv` — a capture of `watch/acceptance.toml`, straight off
///   `./kit record`. Read by [`acceptance_fixture`] above, which also checks it really is a recording
///   of a ridden bike at a fixed step with no frame missing.
/// - `context/fixtures/slow/acceptance-freeride.fixture` — the same thing in the harness's own format,
///   for when the capture has been turned into a fixture file once and for all.
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
    let capture = private_dir("capture").join(format!("{ACCEPTANCE}.csv"));

    let f = if fixture.exists() {
        let text = std::fs::read_to_string(&fixture).expect("a readable fixture");
        Fixture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", fixture.display()))
    } else if capture.exists() {
        let text = std::fs::read_to_string(&capture).expect("a readable capture");
        let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", capture.display()));
        println!(
            "{}: {} rows x {} columns, frames {}..{}",
            capture.display(),
            cap.frames.len(),
            cap.columns.len(),
            cap.frames[0],
            cap.frames[cap.frames.len() - 1],
        );
        acceptance_fixture(&cap).unwrap_or_else(|e| panic!("{}: {e}", capture.display()))
    } else {
        println!("{}", acceptance_recipe(&capture));
        return;
    };

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
    acceptance_verdict(&f);
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
    // clamped rather than wrapped (0xff - 0x7f is 128, which is not an i8).
    let stick = |x: u8, y: u8| {
        let mut bytes = Report::idle().0;
        bytes[6] = x;
        bytes[7] = y;
        PadFrame::new(dt, [Report::from_bytes(bytes), Report::missing()]).stick(0)
    };
    assert_eq!(stick(0x7f, 0x7f), (0, 0));
    assert_eq!(stick(0x89, 0x75), (10, -10));
    assert_eq!(stick(0x00, 0xff), (-127, 127), "hard left, hard down, with 0xff clamped");
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
    let bare = Capture::parse("frame,ball_pos.y\n900,1.0\n901,2.0\n").unwrap();
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
    assert!(run.input.pad(0).held(Control::Cross), "the press is latched before the tick that acts");
    assert!(run.input.pad(0).pressed(Control::Cross), "and this is its press frame");
    assert_eq!(run.input.events.len(), 1, "one transition, on the frame it happened");
    assert_eq!(run.input.pad(0).control(Control::Cross).held_ms, 0);

    run.advance(&tape.frames[1]);
    assert!(!run.input.pad(0).pressed(Control::Cross), "the edge does not survive the frame");
    assert_eq!(run.input.pad(0).control(Control::Cross).held_ms, 33, "a flat +33 at the cap");
    assert!(run.input.events.is_empty(), "the queue is cleared at the top of every update");

    run.advance(&tape.frames[3]);
    assert!(run.input.pad(0).released(Control::Cross));
    assert_eq!(run.input.events.len(), 1);
    assert_eq!(run.input.events.events()[0].pressed, 0, "a release");

    // The whole tape in one call, and the same place.
    let whole = replay::play(&tape, 7);
    assert_eq!(whole.tick(), 5);
    assert_eq!(whole.hash(), replay::play(&tape, 7).hash(), "a replay is a function of its tape");

    // The hash covers the input layer, which is the half a fixture's hashes cannot see: these two runs
    // leave the simulation in the same state (it ignores its input today) and the pad in different
    // ones, and a playback hash that could not tell them apart would prove nothing about the input.
    let idle: PadTape = PadTape { frames: vec![PadFrame::of_buttons(dt, 0); 5], ..tape.clone() };
    assert_eq!(replay::play(&idle, 7).sim.hash(), whole.sim.hash(), "the same simulation state");
    assert_ne!(replay::play(&idle, 7).hash(), whole.hash(), "and a different pad");

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
        .map(|c| run.input.pad(0).control(*c).held_ms)
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
/// simulation it drives is still A1's example ball, so the *simulation* side of the acceptance run is
/// `the_acceptance_run` above and waits for its recording.
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
            let before = run.input.pad(0).held_mask();
            run.advance(frame);
            let got = run.input.pad(0).held_mask();
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
                    run.input.events.len(),
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
    assert_eq!(replay::play(&tape, 0).input.pad(1).held_mask(), 0, "so pad 1 holds nothing");
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
                run.input.pad(0).held(Control::Cross)
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
    assert_eq!(run.input.pad(0).control(Control::Cross).held_ms, 99, "three frames at 33 ms");

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
        let before = run.input.pad(0).held_mask();
        run.advance(frame);
        assert_eq!(
            run.input.pad(0).held_mask(),
            script[tick * 2],
            "tick {tick} holds what field {} of the file recorded",
            tick * 2 + 1,
        );
        assert_eq!(run.tick() as usize, tick + 1, "and one tick is one frame of the recording");
        transitions += (before != run.input.pad(0).held_mask()) as usize;
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
/// side: `Sim` is still the example ball, so this is the input half of the acceptance run and S8 is
/// the other half.
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
            let before = run.input.pad(0).held_mask();
            run.advance(frame);
            seen |= run.input.pad(0).held_mask();
            transitions += (before != run.input.pad(0).held_mask()) as usize;
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
// What it did not settle, which is the task's own *Not verified* list:
//
//   * **The original was never driven by the recording's input.** PCSX2 plays a `.p2m2` back from its
//     GUI only (no CLI flag, no ini key), and this machine has no way to reach that GUI — so
//     `acceptance-freeride.csv` is the original's own minute from the recording's start state *under no
//     input at all*. The human's recorded input therefore has no original to agree with yet, and the
//     tick phase I4 left open stays open: the overlap where the recording and that capture agree is the
//     overlap where *both* pads were at rest, which no choice of phase can disagree about.
//
//     What there is instead is `acceptance-ride.csv`: thirty seconds from the same save state with the
//     kit's virtual pad riding it — six distinct control masks, the bike up to 51 units/s and 555 units
//     from where it started — which is an acceptance capture with input and motion in it that any agent
//     can remake with no human and no GUI (`./kit pad script` beside `./kit record`). It is not the
//     human's ride, and that is the whole of the difference: a scripted pad cannot crash, scrape a
//     slope or land sideways on purpose.
//   * **The port has no bike.** S3 (movement) is open, so the verdict of the real fixture today is
//     `NoSuchField bike.pos[0]` at tick 0 — the fixture's own fault in A2's taxonomy, and the honest
//     answer: there is nothing to compare. [`the_acceptance_run`] prints that and passes; the day S3
//     lands it starts comparing numbers, with no code to write here.

/// The `frame_counter` (app+0x374) in `first_recording.p2m2`'s own save state, read with
/// `./kit state extract` — the tick the recording starts from.
///
/// This is what aligns the recording against a capture taken from the same state: a capture row whose
/// counter is *c* was produced by the tick the recording's frame `c - 2773 - 1` drove. The recorder's
/// first row lands a few ticks in (it loads the state, waits for its warmup and samples after a tick),
/// so the first handful of the recording's ticks are not in any capture — measured at 18 on the
/// capture in hand, and the reason the agreement window below starts where it does rather than at 0.
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

/// The acceptance capture in `context/capture/`, parsed — or `None`, with the recipe printed.
///
/// Game data, so git-ignored and absent on every machine but the one that recorded it, and absent in
/// every worktree but the one it was recorded in (`context/` is per-worktree). A test cannot fail over
/// a file the repository is not allowed to hold.
fn acceptance_capture() -> Option<(PathBuf, Capture)> {
    let path = private_dir("capture").join(format!("{ACCEPTANCE}.csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("{}", acceptance_recipe(&path));
        return None;
    };
    let cap = Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Some((path, cap))
}

/// What a replay of the real acceptance fixture means today, in one line, and what makes it fail.
///
/// Three outcomes, and only one of them is a bug in the port:
///
/// - `Ok` — the port agreed with every recorded tick. That is the acceptance run passing.
/// - `NoSuchField` — the fixture watches a field the port's state does not have. The port has no bike
///   until S3 lands the ported movement, so this is today's answer, and it is printed rather than
///   failed: the capture is good, the comparison has nothing to compare.
/// - anything else (`Field`, `Hash`, `WrongKind`, `Unreached`) — the port and the original disagree
///   about a number, which is the failure this whole file exists to produce. Panics, with the tick and
///   the field in the message.
fn acceptance_verdict(f: &Fixture) {
    match replay::verify(f) {
        Ok(end) => println!(
            "the port agrees with every recorded tick of {}: {} ticks, {} checks",
            f.name,
            end.ticks,
            f.checks(),
        ),
        Err(d) => match &d.reason {
            Reason::NoSuchField { path } => println!(
                "the port has no '{path}': the acceptance capture is good and there is nothing in \
                 the port to compare it against yet (S3 — the ported movement — is open). {d}",
            ),
            _ => panic!("the acceptance run diverges: {d}"),
        },
    }
}

/// **Every acceptance capture in `context/capture/` is the original at a step the port can match.**
///
/// The three properties `watch/acceptance.toml` asks a human to check by eye, checked instead, on every
/// capture that carries its bike columns: the step is the clamp on every tick, there is a row per tick
/// with no gap, and the actor the chain resolves to is a vehicle with a *local human* on it rather than
/// one of the area's three AI riders. All three are invisible in a CSV of 1,811 rows, and each of them
/// is a way a capture can look like an acceptance recording and quietly not be one.
///
/// It also reports how much *input* each capture holds, which is the honest difference between the two
/// in hand (see this section's header): `acceptance-freeride.csv` is the recording's own minute with
/// **nobody touching the pad**, because PCSX2 would not play the recording back without a human at its
/// GUI; `acceptance-ride.csv` is a scripted ride through the kit's virtual pad from the same state —
/// input, and a bike that moves — which an agent can remake from nothing. A count of zero is not a
/// failure, it is the state of S8 printed where whoever runs the tests will see it.
#[test]
fn every_acceptance_capture_is_the_original_at_a_step_the_port_can_match() {
    let dir = private_dir("capture");
    let mut found = 0;
    for path in files_in(&dir, "csv") {
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

        acceptance_verdict(&f);
    }
    if found == 0 {
        println!("{}", acceptance_recipe(&dir.join(format!("{ACCEPTANCE}.csv"))));
    }
}

/// **The recording's bytes are the bytes the original read** — `report_of_p2m2` against libpad's own
/// buffer in EE RAM, over the ticks where the two runs' pads agree.
///
/// Two paths to the same twenty bytes, sharing no code: a `.p2m2` on disk read by I4's reader, and the
/// `pad_a`/`pad_b` columns of a capture taken through PINE after each tick (the half with the smaller
/// stamp being the one the game read — C5's measurement, which `PadTape::from_capture` applies). Over
/// the overlap they produce byte-identical `PadFrame`s: status, the mode byte, the button word
/// active-low, the four axes, the twelve pressures, the `dt`, and the absent second pad.
///
/// **What this does not prove**, and the reason it is a cross-check rather than the acceptance run:
/// the capture was taken under no input, so the window where the two agree is the window where both
/// pads were at rest — fields 1..=55 of the recording are a written, neutral pad. The agreement
/// therefore says the neutral encoding is right on real data and that the two readers agree about what
/// "nothing held" looks like; it cannot say which of a tick's two fields the game read, because both
/// fields of every tick in that window are identical. Settling the phase needs the original driven by
/// the recording, which is the one thing this machine cannot do (see this section's header).
///
/// With teeth all the same: the agreement is asserted to run *exactly* up to the tick where the
/// recording's human first presses something, so a reader that mangled the neutral word, the mode byte
/// or the second pad, or an alignment that slipped, would show up as a shorter run than the recording's
/// own idle prefix explains.
#[test]
fn the_recording_and_the_capture_hold_the_same_pad_where_both_were_idle() {
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
        assert_eq!(
            agreed,
            played.saturating_sub(offset).min(overlap),
            "phase {}: the agreement ends at tick {}, which is not where the recording's human \
             first pressed something (tick {played}) — the capture was taken under no input, so \
             every tick before that one must match and the first one after it must not",
            phase.name(),
            offset + agreed,
        );
    }
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
// S8's answer to that — replaying a human's recording tick by tick — is blocked on a person at
// PCSX2's GUI. This section is the whole-frame proof that is reachable without one: `watch/
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
