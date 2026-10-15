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
    let at = column(cap, "frame_counter")?;
    let mut counters = Vec::with_capacity(cap.rows.len());
    for row in 0..cap.rows.len() {
        let text = cell(cap, row, at);
        counters.push(text.parse::<u64>().map_err(|_| {
            format!("row {row}: frame_counter '{text}' is not a number")
        })?);
    }
    for (row, pair) in counters.windows(2).enumerate() {
        if pair[1] != pair[0] + 1 {
            return Err(format!(
                "frame_counter {} follows {} (row {}): an acceptance recording needs every tick's \
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

/// The left stick, from the half of libpad's port buffer that the game actually read.
///
/// **The half with the *smaller* stamp.** `scePadRead` takes the half whose stamp is the larger, but a
/// PINE row is sampled after the tick, and by then the IOP has refreshed the other half — so the half
/// the game read this frame is the older one by the time the recorder sees it. C5 measured that on 300
/// of 300 rows, with the two halves disagreeing on the 6 rows where a press began or ended. Getting
/// this backwards shifts the input by a frame on exactly those rows, which is the hardest kind of
/// desync to find: the replay is right for minutes and then is not.
///
/// Bytes `[4..8]` are RX RY LX LY, centre 0x7f, so the left stick is bytes 6 and 7 with 0x7f taken off.
/// The dead zone and whatever curve the game applies are I2's, and are the port's business rather than
/// the recording's: this is the number that arrived.
fn stick(cap: &Capture, row: usize, at: &PadColumns) -> Result<(i8, i8), String> {
    let stamp = |column| -> Result<u64, String> {
        let text = cell(cap, row, column);
        text.parse()
            .map_err(|_| format!("frame {}: stamp '{text}' is not a number", cap.frames[row]))
    };
    let older = if stamp(at.stamp_a)? <= stamp(at.stamp_b)? { at.pad_a } else { at.pad_b };
    let bytes = half(cap, row, older)?;
    let axis = |b: u8| (i16::from(b) - 0x7f).clamp(-127, 127) as i8;
    Ok((axis(bytes[6]), axis(bytes[7])))
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
    ticks_of(cap)?;
    let held: Vec<usize> = (0..CONTROLS)
        .map(|n| column(cap, &format!("ctl{n}.held")))
        .collect::<Result<_, _>>()?;
    let pad = PadColumns {
        pad_a: column(cap, "pad_a")?,
        stamp_a: column(cap, "stamp_a")?,
        pad_b: column(cap, "pad_b")?,
        stamp_b: column(cap, "stamp_b")?,
    };
    (1..cap.rows.len())
        .map(|row| {
            Ok(Input {
                buttons: held_mask(cap, row, &held)?,
                stick: stick(cap, row, &pad)?,
            })
        })
        .collect()
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
    for row in 0..cap.rows.len() {
        let frame = cap.frames[row];
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
    csv += ",pad_a,stamp_a,pad_b,stamp_b,bike_type,bike_player";
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
            ",{},{},{},{},4,19873344",
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
    assert_eq!(f.inputs[0], Input { buttons: 1 << 14, stick: (-5, 1) });
    assert_eq!(f.inputs[1], Input { buttons: (1 << 14) | (1 << 6), stick: (-4, 2) });
    assert_eq!(f.inputs[4], Input { buttons: 1 << 4, stick: (-1, 0) });
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

/// Four ways a capture can look like an acceptance recording and not be one, each reported as the
/// capture's own problem with the frame it happened on. All four are invisible in a CSV of 9,000 rows,
/// which is why they are checked rather than eyeballed.
#[test]
fn a_capture_that_is_not_of_the_bike_is_refused() {
    let csv = acceptance_csv(30, played);
    let fixture_of = |text: &str| acceptance_fixture(&Capture::parse(text).unwrap());

    // (1) the chain resolved to something that is not a vehicle actor
    let e = fixture_of(&csv.replacen(",4,19873344", ",2,19873344", 1)).expect_err("type 2");
    assert!(e.contains("bike_type is '2'") && e.contains("0x8 hops"), "{e}");

    // (2) nobody is riding it — an AI's bike, or a world object
    let e = fixture_of(&csv.replace(",4,19873344", ",4,0")).expect_err("no player");
    assert!(e.contains("bike_player is '0'"), "{e}");

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
        .chain(["bike_type", "bike_player", "bike_vel", "bike_xform"])
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
/// Ignored by default because the data is several minutes of game data, which this repository is not
/// allowed to hold, so it is absent on every machine but the one that recorded it:
///
/// ```sh
/// cargo test -p game-sim --test replay -- --ignored --nocapture the_acceptance_run
/// ```
///
/// When neither file is there it says what to record and stops — the same thing
/// `replaying_every_fixture_in_context` does about an empty directory, and for the same reason: a test
/// cannot fail over a file it is not allowed to ship. **The absence is the status of the port, and it
/// is written down in `plan/HUMAN.md` and in this task's journal entry, not here.**
#[test]
#[ignore = "needs the acceptance recording, which is game data; see plan/HUMAN.md"]
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
        println!(
            "no acceptance recording on this machine. To make one:\n\
             \x20 1. reach a freeride area and save it in slot 4 (plan/HUMAN.md), or boot straight \
             into one (R7);\n\
             \x20 2. ride for several minutes — jump slow and fast, land clean and sideways, crash, \
             scrape a slope, sit still;\n\
             \x20 3. ./kit record watch/acceptance.toml --state 4 -n {SEVERAL_MINUTES} -o {}\n\
             then run this test again. It needs no code.",
            capture.display(),
        );
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
        f.inputs.len() >= 60 * 30,
        "{} frames is not an acceptance run: the point is several minutes of play, where the \
         hundredth awkward landing is",
        f.inputs.len(),
    );
    match replay::verify(&f) {
        Ok(end) => println!("the port agrees with every recorded frame; ticks {}", end.ticks),
        Err(d) => panic!("the acceptance run diverges: {d}"),
    }
}
