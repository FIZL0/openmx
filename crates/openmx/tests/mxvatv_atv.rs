//! MD37d: MX vs. ATV's quad in the shell, an addition off by default. With `--atv NAME` (the Garage's
//! Quad row under the front end) and openmx-mods' `mxvatv_atv` machine pack in the mods folder, the
//! pause's Switch To Machine offers the quad on that handling file, built off the pack's `.phy`, its
//! chassis model and its collision objects alone and ridden on MX vs. ATV's step (MD122, MD123).
//! Without it the ride is MX Unleashed's: no pack is read and a switch to the quad is refused. With it
//! and no switch, the bike's ride hashes as it does without.
//!
//! This rides the binary headless on hand-written tapes, as mxvatv_fliers.rs does. It skips with no
//! disc, and the quad half with no pack in this checkout's `mods/machines/`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Ride free09 headless for `frames` ticks on the steady clock playing `tape`, with `more` flags
/// and this checkout's mods folder, or `None` with no disc to ride.
fn ride(frames: u32, tape: &Path, more: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .args(["--headless", "--track", "free09", "--frames", &frames.to_string(), "--steady-clock", "--mods", "mods"])
        .args(["--play", tape.to_str().expect("a path")])
        .args(more)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// Whether this checkout has the quad pack; says SKIPPED when not.
fn pack() -> bool {
    let there = root().join("mods/machines/mxvatv_atv/mod.json").is_file();
    if !there {
        eprintln!("SKIPPED: no mods/machines/mxvatv_atv (openmx-mods' import-atv makes it)");
    }
    there
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md37d-mxvatv-atv");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// `frames` ticks of Cross held at full pressure (the gas), with `switches` (`30 atv`) after them.
fn tape(name: &str, frames: usize, switches: &[&str]) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape Cross held\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    for s in switches {
        text += &format!("switch {s}\n");
    }
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The machines the ride's `vehicle:` lines say it was on, in order.
fn vehicles(said: &str) -> Vec<&str> {
    said.lines().filter_map(|l| l.strip_prefix("vehicle: ")).collect()
}

/// The `ride hash` line the shell says at the ride's end.
fn ride_hash(said: &str) -> &str {
    said.lines().find(|l| l.starts_with("ride hash ")).unwrap_or_else(|| panic!("no ride hash:\n{said}"))
}

#[test]
fn off_by_default_there_is_no_quad() {
    let switched = tape("off.tape", 300, &["30 atv", "280 bike"]);
    let Some(said) = ride(320, &switched, &[]) else { return };
    assert!(!said.lines().any(|l| l.starts_with("atv:")), "a ride without a quad picked read the quad pack:\n{said}");
    assert_eq!(vehicles(&said), ["bike", "this ride has no quad", "bike"], "\n{said}");
}

#[test]
fn a_picked_quad_rides_and_leaves_the_bike_ride_alone() {
    if !pack() {
        return;
    }
    let switched = tape("on.tape", 300, &["30 atv", "280 bike"]);
    let Some(said) = ride(320, &switched, &["--atv", "atv_Race250P"]) else { return };
    assert!(said.contains("atv: Race 250 P (atv_Race250P) off mxvatv_atv (switch from the pause)\n"), "\n{said}");
    assert_eq!(vehicles(&said), ["bike", "atv", "bike"], "\n{said}");
    // The gas held over 250 ticks rides it well away: the line said as it is left. Since MD133's wreck
    // test a hard landing on the way may wreck it (a front load past the `.phy`'s limit), so the
    // wrecks are not asserted.
    let line = said.lines().find(|l| l.starts_with("atv: atv_Race250P ")).unwrap_or_else(|| panic!("no quad line:\n{said}"));
    let gone: f32 = line.split_whitespace().nth(2).and_then(|w| w.parse().ok()).unwrap_or_else(|| panic!("no distance in `{line}`"));
    assert!(gone > 100.0, "the quad went only {gone} ft on the gas: {line}");

    // The sport chassis' twin builds and rides too.
    let Some(sport) = ride(320, &switched, &["--atv", "atv_s_Pro250A"]) else { return };
    assert_eq!(vehicles(&sport), ["bike", "atv", "bike"], "\n{sport}");
    assert!(sport.lines().any(|l| l.starts_with("atv: atv_s_Pro250A ")), "\n{sport}");

    // Picked and never switched to, the bike's ride is the one with none picked.
    let bike = tape("bike.tape", 200, &[]);
    let Some(none) = ride(220, &bike, &[]) else { return };
    let Some(picked) = ride(220, &bike, &["--atv", "atv_Race250P"]) else { return };
    assert_eq!(ride_hash(&none), ride_hash(&picked));
}
