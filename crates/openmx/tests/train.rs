//! S137: the bike rode through the Dallas train (free06's path cars, S81) in the shell. The cars
//! were in the world and moving, but the ride's only body probe, one ray from the centre of mass
//! along the velocity, passes under a car's deck, and a ray that starts inside a car meets only back
//! faces. `game_sim::mover_contact` now runs the bike's eleven body rays against the cars once a
//! tick, as the rider-against-rider contacts are run.
//!
//! These ride the shell's own binary, headless on free06, so the cars are the ones the shell loads
//! and steps and the ride is the one `vehicle.rs` steps through `Sim::step`:
//!
//! - the user's path: full gas square at the line from the east, into the train's side;
//! - a bike standing on the rails, which the train's nose runs into, head-on and from behind (S170).

use std::path::{Path, PathBuf};
use std::process::Command;

/// A pad tape of `frames` frames: full throttle (Cross on full pressure), or nothing held.
fn tape(name: &str, frames: usize, gas: bool) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape {name}\n");
    let report = if gas {
        format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17))
    } else {
        format!("0079ffff7f7f7f7f{}", "00".repeat(24))
    };
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("s137-train");
    std::fs::create_dir_all(&dir).expect("the scratch folder");
    let path = dir.join(format!("{name}.tape"));
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The shell headless on free06 from `--trace-at at` riding `tape` for `frames` frames: what it
/// said, or `None` with no disc to ride.
fn ride(at: &str, tape: &Path, frames: usize) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--size", "640x448", "--overlay", "none", "--controls", "original"])
        .args(["--trace-at", at, "--play", tape.to_str().unwrap(), "--frames", &frames.to_string()])
        // The original's air control: these tapes hold the gas through the hops the train throws the
        // bike into, and the port's gas gyro (P255) pitches the bike back there, which is not what
        // these paths were laid out on (nor what S170's and S191's captures rode).
        .arg("--no-gas-gyro")
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// The wreck reasons the shell said, in order.
fn wrecks(said: &str) -> Vec<u32> {
    said.lines()
        .filter_map(|l| l.strip_prefix("wreck: reason "))
        .map(|r| r.split_whitespace().next().and_then(|n| n.parse().ok()).expect("a reason number"))
        .collect()
}

/// Where the bike ended, off the shell's `end:` line.
fn end(said: &str) -> [f32; 3] {
    let l = said.lines().find_map(|l| l.strip_prefix("end: ")).unwrap_or_else(|| panic!("no end line:\n{said}"));
    let v: Vec<f32> = l.split(',').map(|w| w.trim().parse().expect("a number")).collect();
    [v[0], v[1], v[2]]
}

/// How far east of the train's line a point is, in feet: the straight the cars run down south past
/// the east start, through car 0's first place, heading (-0.17, -0.98). A car is about five feet
/// either side of it.
fn east_of_line(p: [f32; 3]) -> f32 {
    (p[0] - 2408.0) * 0.98 - (p[2] - 3897.0) * 0.17
}

/// Reason 1: over the bars, the fore-and-aft load.
const FRONT: u32 = 1;

/// Reason 6: scenery struck.
const SCENERY: u32 = 6;

#[test]
fn full_gas_into_the_trains_side_never_comes_out_the_far_side() {
    // The user's path: 148 ft east of the line, square at it. The bike meets car 0's side, is put
    // back after its first fall, and rides into the train again where car 4 meets car 5. Before
    // S137 it came out 35 ft west of the line (2346, 135, 3748) with no fall but its first.
    let Some(said) = ride("2527,3714,280", &tape("gas", 240, true), 240) else { return };
    let at = end(&said);
    assert!(east_of_line(at) > -6.0, "the bike ended {:.1} ft past the line, at {at:?}:\n{said}", -east_of_line(at));
    // Since S138 the body rays meet every prop and a head-on side is a g wreck (reason 1), as a wall is.
    assert!(wrecks(&said).iter().any(|r| [1, SCENERY].contains(r)), "the train never threw the rider: {:?}\n{said}", wrecks(&said));
}

#[test]
fn a_bike_standing_on_the_rails_is_thrown_by_the_trains_nose() {
    // On the line ahead of the train, facing it, nothing held: car 0's nose reaches it at about
    // tick 50. Before S137 the train ran through it and it stood on.
    // Since S151 the car is the contacts' mover, as the original's record names it: the nose's
    // response carries the bike back towards the train's speed, and that is a fore-and-aft wreck
    // over the bars (1). S170 captured the original's bike standing on the rails facing the nose
    // (4 degrees off head-on): it enters reason 1 on the first tick the nose touches it, and its
    // record's entries name the engine as their mover.
    let Some(said) = ride("2385.8,3779.9,10", &tape("idle", 100, false), 100) else { return };
    assert_eq!(wrecks(&said).first(), Some(&FRONT), "the nose did not throw the rider over the bars:\n{said}");
}

#[test]
fn a_bike_the_nose_meets_from_behind_is_carried_without_a_fore_and_aft_wreck() {
    // S170's other capture: the original's bike standing on the rails with the train coming from 36
    // degrees off its tail (the train runs at 190 degrees here, so the bike faces 154). The nose
    // carries it up towards the train's speed with no fore-and-aft wreck. When its own head ray
    // strikes the car is the ride's: seven ticks on in S170's braked capture, forty in S191's stand,
    // which tests/replay.rs replays to that wreck bit for bit. Until S191a the port's own head-inside-
    // a-car contact wrecked it here within a few ticks of the nose; this spot is neither capture, so
    // only the carrying is asked: the nose meets it at about tick 50 and moves it on.
    let Some(said) = ride("2385.8,3779.9,154", &tape("behind", 100, false), 100) else { return };
    assert!(!wrecks(&said).contains(&FRONT), "the nose from behind threw the rider over the bars:\n{said}");
    let at = end(&said);
    let moved = ((at[0] - 2385.8).powi(2) + (at[2] - 3779.9).powi(2)).sqrt();
    assert!(moved > 20.0, "the nose carried the bike only {moved:.1} ft, to {at:?}:\n{said}");
}

// S137's "a bike inside a car comes off at once" (stood where car 0 is at the first tick) asked for the
// port's own head-inside-a-car contact, which S191a removed: the original's record holds no such entry
// (S191's stand capture rides on with the head ray's end inside the engine). What the original does
// with a bike the train already holds is not recorded.

/// A pad tape of `frames` frames: full throttle for the first `gas`, then nothing held.
fn burst(name: &str, frames: usize, gas: usize) -> PathBuf {
    let on = std::fs::read_to_string(tape(&format!("{name}-on"), gas, true)).expect("the gas tape");
    let off = std::fs::read_to_string(tape(&format!("{name}-off"), frames - gas, false)).expect("the idle tape");
    let frames_of = |t: &str| t.lines().filter(|l| l.starts_with("frame ")).map(|l| format!("{l}\n")).collect::<String>();
    let text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape {name}\n{}{}", frames_of(&on), frames_of(&off));
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("s137-train").join(format!("{name}.tape"));
    std::fs::write(&path, text).expect("the tape is written");
    path
}

#[test]
fn a_bike_riding_up_beside_the_dump_truck_rides_on() {
    // S216, the user's: "hitting the moving vehicles on dallas makes us bail instantly". A short
    // burst from beside the dump truck by the east road, rolling in alongside it at a glancing angle:
    // the bike's head is inside the truck's 28 ft wide box all along, clear of its body, and before
    // S216 the rider came off at once with nothing struck.
    let Some(said) = ride("2955,3800,240", &burst("beside", 200, 25), 200) else { return };
    assert_eq!(wrecks(&said), [] as [u32; 0], "a bike beside the dump truck came off:\n{said}");
}
