//! S149e: the cars are machines in the shell. This rides the binary headless, as a user would, on a
//! hand-written tape of full throttle with a `switch` into each car on its own track and back to the
//! bike: the shell takes both switches, and the car went a good way on the gas before it was left.

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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("s149e-car-switch");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Full throttle for `frames` ticks, switching into `car` at `up` and back to the bike at `down`.
fn car_tape(frames: usize, car: &str, up: usize, down: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the {car} on the gas\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text += &format!("switch {up} {car}\nswitch {down} bike\n");
    text
}

#[test]
fn each_car_is_switched_into_and_driven() {
    for (track, car) in [("free09", "monster"), ("free07", "baja"), ("free11", "buggy")] {
        let tape = scratch(&format!("{car}.tape"));
        std::fs::write(&tape, car_tape(240, car, 30, 200)).expect("the tape is written");
        let Some(said) = ride(track, 260, tape.to_str().expect("a path")) else { return };
        println!("{track}:\n{said}");

        let vehicles: Vec<&str> = said.lines().filter(|l| l.starts_with("vehicle: ")).collect();
        assert_eq!(vehicles, ["vehicle: bike", format!("vehicle: {car}").as_str(), "vehicle: bike"], "\n{said}");
        assert!(said.contains("the tape ran out after 240 frames"), "\n{said}");

        // The car said where it was as it was left: 170 ticks on the gas take it a good way.
        let left = said
            .lines()
            .find(|l| l.starts_with(&format!("car: {car} ")))
            .unwrap_or_else(|| panic!("no car line:\n{said}"));
        let gone: f32 = left
            .split_whitespace()
            .nth(2)
            .and_then(|w| w.parse().ok())
            .unwrap_or_else(|| panic!("no distance in `{left}`"));
        assert!(gone > 150.0, "the {car} went only {gone} ft on the gas: {left}");
    }
}
