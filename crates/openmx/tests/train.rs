//! S137: the bike rode through the Dallas train (free06's path cars, S81) in the shell. The cars
//! were in the world and moving, but the ride's only body probe, one ray from the centre of mass
//! along the velocity, passes under a car's deck, and a ray that starts inside a car meets only back
//! faces. `game_sim::mover_contact` now runs the bike's eleven body rays against the cars once a
//! tick, as the rider-against-rider contacts are run, and the head ray inside a car is a contact too.
//!
//! These ride the shell's own binary, headless on free06, so the cars are the ones the shell loads
//! and steps and the ride is the one `vehicle.rs` steps through `Sim::step`:
//!
//! - the user's path: full gas square at the line from the east, into the train's side;
//! - a bike standing on the rails, which the train's nose runs into;
//! - a bike the train already holds (stood where car 0 is at the first tick).

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
    // response carries the bike towards the train's speed and that is a g wreck (1), where the
    // scenery response (S137) braked it against a still wall and the head ray's reach made it 6.
    // Which of the two the original enters is S170's capture.
    let Some(said) = ride("2385.8,3779.9,10", &tape("idle", 100, false), 100) else { return };
    assert!(wrecks(&said).first().is_some_and(|r| [1, SCENERY].contains(r)), "the nose did not throw the rider:\n{said}");
}

#[test]
fn a_bike_inside_a_car_comes_off_at_once() {
    // Stood on the rails where car 0 is at the first tick: every ray starts inside the car.
    let Some(said) = ride("2407,3902,190", &tape("still", 5, false), 5) else { return };
    assert_eq!(wrecks(&said), [SCENERY], "a bike inside the train rode on:\n{said}");
}
