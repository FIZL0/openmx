//! P327: a race started from the front end rides what `game_sim::race` has of one — sx01's grid
//! slot behind the drop-gate prop's start gate, the gate closed until the start calls drop it — and
//! a Free Ride on the same track has none of it (one racer, no gate).
//!
//! These ride the binary headless, as `--race` starts the ride the menu's Start Race does; the
//! front end's own walk to it is `tests/menu.rs`' `race_from_the_main_menu_rides_its_setup_and_shows_results`.

use std::path::Path;
use std::process::Command;

/// Ride sx01 headless for five seconds with these arguments, or `None` with no disc to ride.
fn ride(args: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--steady-clock", "--track", "sx01", "--seconds", "5", "--no-sound"])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// The line starting `prefix`, if there is one.
fn line<'a>(said: &'a str, prefix: &str) -> Option<&'a str> {
    said.lines().find(|l| l.starts_with(prefix))
}

/// The three numbers after `word` in `l`, `x,y,z`.
fn xyz(l: &str, word: &str) -> [f32; 3] {
    let rest = l.split(word).nth(1).unwrap_or_else(|| panic!("no `{word}` in {l}"));
    let w: Vec<f32> = rest.trim_start().split([',', ' ', ';']).take(3).map(|n| n.parse().unwrap_or_else(|_| panic!("{l}"))).collect();
    [w[0], w[1], w[2]]
}

#[test]
fn a_race_starts_on_the_grid_behind_a_closed_gate_that_drops() {
    let Some(said) = ride(&["--race", "--laps", "3", "--racers", "4"]) else { return };
    let start = line(&said, "race: 3 laps, 4 riders;").unwrap_or_else(|| panic!("no race start:\n{said}"));
    // The gate stands where S196's image of sx01 has it, closed.
    let gate = xyz(start, "gate at");
    for (got, want) in gate.into_iter().zip([101.49, 87.87, 454.58]) {
        assert!((got - want).abs() < 0.01, "{start}");
    }
    assert!(start.contains(" closed;"), "{start}");
    // The grid is behind it: sx01's gate faces +z, the riders short of it.
    let grid = xyz(start, "grid");
    assert!(grid[2] < gate[2] - 2.0 && (grid[0] - gate[0]).abs() < 60.0, "{start}");
    // It drops on its countdown — 1.5 s and the start call's — and the rider is still behind it.
    let delay: f32 = start.split("drop in ").nth(1).and_then(|d| d.trim_end_matches('s').parse().ok()).unwrap_or_else(|| panic!("{start}"));
    let dropped = line(&said, "race: gate dropped at tick ").unwrap_or_else(|| panic!("the gate never dropped:\n{said}"));
    let tick: f32 = dropped.split_whitespace().nth(5).and_then(|t| t.trim_end_matches(',').parse().ok()).unwrap_or_else(|| panic!("{dropped}"));
    assert!((tick / 30.0 - delay).abs() < 0.2, "{dropped} for a drop in {delay}s");
    let rider = xyz(dropped, "rider at");
    assert!(rider[2] < gate[2], "{dropped}: the rider got past the closed gate");
}

#[test]
fn a_free_ride_on_the_same_track_is_no_race() {
    let Some(said) = ride(&[]) else { return };
    assert!(said.lines().all(|l| !l.starts_with("race:")), "{said}");
}
