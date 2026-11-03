//! P216: the Kiowa is a machine in the shell. This rides the binary headless, as a user would, on a
//! hand-written tape with a `switch` into the helicopter and back to the bike: held Cross (the
//! collective's climb) takes it up a long way, and left alone it stands on its skids where it was
//! taken. The shell follows it on its own camera, `HeloCPresets.ini`'s first preset.
//!
//! P216a: it stands and lands on the original's ground (its collision record, C14a), and the crash
//! test's MinWreckSpeed wrecks it: brought down fast and nose-first it wrecks, let straight down on
//! Square it stands.
//!
//! P216a1: the wreck and put-back are the original's (S220a1's `kiowa::Wreckage`): a player's
//! helicopter has no owner, so the put-back stands it at its spawn, where it was taken.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Ride `track` headless for at most `frames` ticks on the steady clock playing `tape`, or `None`
/// with no disc to ride.
fn ride(track: &str, frames: u32, tape: &str) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", track, "--frames", &frames.to_string(), "--steady-clock"])
        .args(["--play", tape])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p216-kiowa-switch");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// `frames` ticks of one pad report with the second buttons byte `buttons` (active low: `0xbf` is
/// Cross held, `0xff` nothing), switching into the helicopter at `up` and back to the bike at `down`.
fn heli_tape(frames: usize, buttons: u8, up: usize, down: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the helicopter\n");
    let cross = if buttons & 0x40 == 0 { 0xff } else { 0x00 };
    let report = format!("0079ff{buttons:02x}7f7f7f7f000000000000{cross:02x}{}", "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text += &format!("switch {up} helicopter\nswitch {down} bike\n");
    text
}

/// The `kiowa:` line the shell says as the helicopter is left: how far it went from where it was
/// taken, and how high over it it was.
fn left(said: &str) -> (f32, f32) {
    let line = said
        .lines()
        .find(|l| l.starts_with("kiowa: ") && l.contains("from where it was taken"))
        .unwrap_or_else(|| panic!("no kiowa line:\n{said}"));
    let words: Vec<&str> = line.split_whitespace().collect();
    let number = |i: usize| words[i].trim_end_matches(',').parse::<f32>().unwrap_or_else(|_| panic!("no number at {i} in `{line}`"));
    (number(1), number(8))
}

#[test]
fn the_helicopter_is_switched_into_and_climbs_on_cross() {
    let tape = scratch("climb.tape");
    std::fs::write(&tape, heli_tape(240, 0xbf, 30, 200)).expect("the tape is written");
    let Some(said) = ride("free09", 260, tape.to_str().expect("a path")) else { return };
    println!("{said}");

    let vehicles: Vec<&str> = said.lines().filter(|l| l.starts_with("vehicle: ")).collect();
    assert_eq!(vehicles, ["vehicle: bike", "vehicle: helicopter", "vehicle: bike"], "\n{said}");
    assert!(said.contains("the tape ran out after 240 frames"), "\n{said}");
    assert!(said.contains("kiowa: its camera starts on Aero Cam 1"), "\n{said}");

    // 170 ticks of the collective held up take it well up, and straight up: no stick.
    let (gone, over) = left(&said);
    assert!(over > 100.0, "the helicopter climbed only {over} ft on Cross:\n{said}");
    assert!(gone < 10.0, "it drifted {gone} ft with the stick centred:\n{said}");
}

#[test]
fn the_helicopter_left_alone_stands_on_its_skids() {
    let tape = scratch("idle.tape");
    std::fs::write(&tape, heli_tape(240, 0xff, 30, 200)).expect("the tape is written");
    let Some(said) = ride("free09", 260, tape.to_str().expect("a path")) else { return };
    println!("{said}");

    let vehicles: Vec<&str> = said.lines().filter(|l| l.starts_with("vehicle: ")).collect();
    assert_eq!(vehicles, ["vehicle: bike", "vehicle: helicopter", "vehicle: bike"], "\n{said}");
    let (gone, over) = left(&said);
    assert!(over.abs() < 1.0 && gone < 1.0, "it moved {gone} ft and {over} ft up left alone:\n{said}");
    assert!(said.contains("on its skids"), "\n{said}");
}

/// One phase of a [`flight`] tape: `frames` ticks with the second buttons byte `buttons` (active
/// low), Cross's and Square's pressure, and the left stick's `y` (`0x7f` centred, `0x00` forward).
struct Phase {
    frames: usize,
    buttons: u8,
    cross: u8,
    square: u8,
    stick_y: u8,
}

/// The tape of `phases`, switching into the helicopter at frame 30 and never out.
fn flight(phases: &[Phase]) -> String {
    let frames: usize = phases.iter().map(|p| p.frames).sum();
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the helicopter\n");
    for p in phases {
        let mut pressure = [0u8; 12];
        (pressure[6], pressure[7]) = (p.cross, p.square);
        let pressure: String = pressure.iter().map(|b| format!("{b:02x}")).collect();
        let report = format!("0079ff{:02x}7f7f7f{:02x}{pressure}{}", p.buttons, p.stick_y, "00".repeat(12));
        for _ in 0..p.frames {
            text += &format!("frame 0x3d08850a 0x{report} -\n");
        }
    }
    text += "switch 30 helicopter\n";
    text
}

/// The `kiowa:` line the shell says of the helicopter at the ride's end: how many times it wrecked
/// since it was taken, and the whole line.
fn wrecks(said: &str) -> (u32, &str) {
    let line = said.lines().rev().find(|l| l.starts_with("kiowa: ") && l.contains("wrecked")).unwrap_or_else(|| panic!("no kiowa line:\n{said}"));
    let n = line.rsplit("wrecked ").next().and_then(|w| w.split_whitespace().next()).and_then(|n| n.parse().ok());
    (n.unwrap_or_else(|| panic!("no wreck count in `{line}`")), line)
}

const REST: Phase = Phase { frames: 30, buttons: 0xff, cross: 0, square: 0, stick_y: 0x7f };
const CLIMB: Phase = Phase { frames: 60, buttons: 0xbf, cross: 0xff, square: 0, stick_y: 0x7f };

#[test]
fn a_landing_at_speed_wrecks() {
    // Up 110 ft, 200 ticks nose down to ~128 ft/s, then down on Square with the stick still
    // forward: it meets the ground going fast, wrecks, and is put back on its skids where it was
    // taken (its spawn).
    let tape = scratch("fast-landing.tape");
    let forward = Phase { frames: 200, stick_y: 0x00, ..REST };
    let down = Phase { frames: 300, buttons: 0x7f, square: 0xff, stick_y: 0x00, ..REST };
    std::fs::write(&tape, flight(&[REST, CLIMB, forward, down])).expect("the tape is written");
    let Some(said) = ride("free09", 600, tape.to_str().expect("a path")) else { return };
    println!("{said}");
    let (n, line) = wrecks(&said);
    assert_eq!(n, 1, "a fast landing wrecked it {n} times: {line}\n{said}");
    assert!(line.contains("on its skids"), "it was not put back: {line}");
    let gone: f32 = line.split_whitespace().nth(1).and_then(|w| w.parse().ok()).unwrap_or_else(|| panic!("no distance in `{line}`"));
    assert!(gone < 5.0, "it was put back {gone} ft from its spawn: {line}");
}

#[test]
fn a_soft_landing_stands() {
    // Up 110 ft, then straight down on Square to the ground, and left there.
    let tape = scratch("soft-landing.tape");
    let down = Phase { frames: 400, buttons: 0x7f, square: 0xff, ..REST };
    std::fs::write(&tape, flight(&[REST, CLIMB, down])).expect("the tape is written");
    let Some(said) = ride("free09", 500, tape.to_str().expect("a path")) else { return };
    println!("{said}");
    let (n, line) = wrecks(&said);
    assert_eq!(n, 0, "a soft landing wrecked it: {line}\n{said}");
    assert!(line.contains("on its skids"), "it is not down: {line}");
    let (_, over) = left(&said);
    assert!(over.abs() < 1.0, "it stands {over} ft over where it was taken: {line}");
}
