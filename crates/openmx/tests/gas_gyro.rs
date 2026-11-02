//! P255: the app rides the port's gas gyro — the throttle held in the air pulls the nose up — and
//! `--no-gas-gyro` rides the original's air control, which reads no throttle. The step itself is
//! game-sim's `tests/air_gas_gyro.rs`; this is the wiring.

use std::path::Path;
use std::process::Command;

/// free09's big jump flat out from `tests/shadows.rs`'s spot, `--trace-air`'s pitch (degrees, nose
/// up +) a frame, up to frame 89: about a second of air after the lip at about frame 65.
fn pitches(extra: &[&str]) -> Option<Vec<f32>> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free09", "--controls", "original", "--trace-at", "3864.2,1592.5,-132"])
        .args(["--trace", "90", "--trace-gas", "--trace-air"])
        .args(extra)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    let head = said.lines().position(|l| l.starts_with("frame ground fr yaw pitch")).expect("the trace's header");
    Some(said.lines().skip(head + 1).filter_map(|l| l.split_whitespace().nth(4)?.parse().ok()).collect())
}

#[test]
fn the_gas_in_the_air_pulls_the_nose_up_unless_told_not_to() {
    let Some(on) = pitches(&[]) else { return };
    let Some(off) = pitches(&["--no-gas-gyro"]) else { return };
    assert_eq!(on.len(), off.len());
    // On the ramp the two ride alike (a hair apart: the gyro acts on any hop before the lip).
    assert!((on[50] - off[50]).abs() < 1.0, "on the ramp: {} on, {} off", on[50], off[50]);
    // A second into the air the original's nose has dropped a little; the gas has pulled it well up.
    let (a, b) = (on[on.len() - 1], off[off.len() - 1]);
    assert!(b < off[60] && a > b + 15.0, "a second into the air: {a} on, {b} off (at the lip {} / {})", on[60], off[60]);
}
