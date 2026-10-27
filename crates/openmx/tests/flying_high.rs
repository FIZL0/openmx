//! V96: flying the biplane high over a wrapped world holds 60 Hz. Before, the plane's collision
//! search built its broad phase from every prop in the world twice a tick, the tile stream measured
//! every tile and join against the plane each tick, and the world's path cars replayed their warm-up
//! on fresh ground queries each picture — over world_ultra a frame took 243 ms at 500 feet and
//! 24–29 ms at 2000 and 5000.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Full throttle and nothing else, `frames` long: Cross down, at full pressure.
fn throttle_tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape full throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text
}

/// The `session ms a frame` line's stage `name`, or its `all`.
fn stage(session: &str, name: &str) -> f32 {
    let key = format!(" {name} ");
    let rest = session.split(&key).nth(1).unwrap_or_else(|| panic!("no {name} in {session}"));
    rest.split([' ', ',', ';']).next().and_then(|v| v.parse().ok()).unwrap_or_else(|| panic!("{name} in {session}"))
}

/// The number before ` over 16.7` on the session line.
fn over(session: &str) -> usize {
    let head = session.split(" over 16.7").next().unwrap();
    head.rsplit(' ').next().and_then(|v| v.parse().ok()).unwrap_or_else(|| panic!("no over count in {session}"))
}

/// The user's world_ultra flown 15 s on full throttle at 2000 feet, headless at 1920x1080 on Far Fog
/// with `--timings`, put in the air with `--fly-at`: a frame holds 16.7 ms on average, a slow frame is
/// rare, and the simulation is a small part of a frame. Ignored: it needs the user's mods (`V92_MODS`,
/// or `~/repos/openmx/mods`) and a quiet machine, since a frame's stages are wall-clock time.
#[test]
#[ignore]
fn a_plane_flown_high_over_world_ultra_holds_60_hz() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = std::env::var_os("V92_MODS").map(PathBuf::from).unwrap_or_else(|| Path::new(&std::env::var("HOME").unwrap()).join("repos/openmx/mods"));
    if !mods.join("tracks/world_ultra/mod.json").exists() {
        eprintln!("skipped: no world_ultra in {}", mods.display());
        return;
    }
    let tape = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v96").join("gas.tape");
    std::fs::create_dir_all(tape.parent().unwrap()).unwrap();
    std::fs::write(&tape, throttle_tape(1200)).expect("the tape is written");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--size", "1920x1080", "--mods", mods.to_str().unwrap(), "--track", "world_ultra"])
        .args(["--vehicle", "biplane", "--biplane-controls", "original", "--graphics", "far", "--fly-at", "86692,22046,0,2000"])
        .args(["--timings", "--play", tape.to_str().unwrap(), "--frames", "900"])
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    let session = said.lines().find(|l| l.starts_with("session ms a frame")).unwrap_or_else(|| panic!("no session line:\n{said}"));
    eprintln!("{session}");
    let (all, sim, slow) = (stage(session, "all"), stage(session, "sim"), over(session));
    assert!(all <= 16.9, "a frame took {all} ms on average: {session}");
    assert!(slow <= 18, "{slow} of 900 frames took more than 16.7 ms: {session}");
    assert!(sim < 2.0, "the simulation took {sim} ms a frame: {session}");
}

#[test]
fn the_session_line_reads() {
    let line = "session ms a frame: input 0.01 menu 0.00 sim 0.15 stream 0.89 anim 0.00 pace 12.44, all 16.68; worst 34.12 ms (stream 15.72, hud 9.74), 8 over 16.7; peaks: sim 2.68";
    assert_eq!((stage(line, "all"), stage(line, "sim"), over(line)), (16.68, 0.15, 8));
}
