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
//!
//! Plus the two tests that hook into the private side: every fixture file in `context/fixtures/` is
//! replayed, and every capture in `context/capture/` is parsed. Those directories hold data read out
//! of the game, so they are never committed — the tests say what they found and pass when there is
//! nothing there, which is the only honest thing a test can do about a file it is not allowed to ship.
//! Run the long ones with:
//!
//! ```sh
//! cargo test -p game-sim --test replay -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use game_sim::hash::Leaf;
use game_sim::replay::{self, Capture, Fixture, Reason, Sample, Value, Watch};
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
