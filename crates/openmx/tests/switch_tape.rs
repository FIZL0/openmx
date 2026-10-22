//! P102: a switch of machine is an input at a tick in a tape. This rides the binary headless, as a
//! user would, on a hand-written tape of full throttle with `switch` lines to the plane and back:
//! the shell takes both switches where the tape says, and two plays of it end on exactly the same
//! bike and the same plane.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Ride headless for `seconds` with these arguments, or `None` with no disc to ride.
fn ride(seconds: u32, args: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--seconds", &seconds.to_string()])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// The line starting `prefix`.
fn line<'a>(said: &'a str, prefix: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no `{prefix}` line:\n{said}"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p102-switch-tape");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Full throttle at the engine's capped `dt` for `frames` ticks, switching to the plane at `up` and
/// back to the bike at `down`.
fn switching_tape(frames: usize, up: usize, down: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape throttle and two switches\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text += &format!("switch {up} biplane\nswitch {down} bike\n");
    text
}

#[test]
fn a_played_tape_takes_its_switches_and_replays_bit_for_bit() {
    let tape = scratch("switching.tape");
    std::fs::write(&tape, switching_tape(240, 60, 180)).expect("the tape is written");
    let tape = tape.to_str().expect("a path");
    let Some(first) = ride(9, &["--play", tape]) else { return };
    let again = ride(9, &["--play", tape]).expect("the disc was there a moment ago");
    println!("{first}");

    // Both switches were taken, in order, and the ride ended on the bike.
    let said: Vec<&str> = first.lines().filter(|l| l.starts_with("vehicle: ")).collect();
    assert_eq!(said, ["vehicle: bike", "vehicle: biplane", "vehicle: bike"], "\n{first}");
    // Exactly the same bike and the same plane, not nearly.
    assert_eq!(line(&first, "ride hash"), line(&again, "ride hash"));
    assert_eq!(line(&first, "plane hash"), line(&again, "plane hash"));
    assert!(line(&first, "plane hash").ends_with("(bike)"), "\n{first}");
}
