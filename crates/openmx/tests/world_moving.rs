//! MD153: on a world the tiles' path cars, knockable props and animated toys were only in the
//! picture (the user: "in mx world trucks and props have no collision"). `game_sim::tiles` now holds
//! every tile's in one moving list, as a track's `World::moving`, stepped with the cars.
//!
//! These ride the shell's own binary, headless on world_mxu_free, from places in free06 and free09
//! moved by their tiles' offsets (x 144 and 16560), each the place a free06 or free09 test rides
//! alone:
//!
//! - full gas square at Dallas's train (tests/train.rs's user's path): the bike is thrown at the
//!   train's side and never comes out the far side;
//! - full gas at free09's beach balls by the pool: the balls move.
//!
//! The world is openmx-mods' stitch of the user's disc: `$OPENMX_WORLDS/tracks/world_mxu_free`, else
//! the main checkout's `mods/` (as tests/disc_worlds.rs looks); without it, or the disc, they skip.

use std::path::{Path, PathBuf};
use std::process::Command;

/// free06's offset in the world, `x`.
const FREE06_X: f32 = 144.0;

/// free09's.
const FREE09_X: f32 = 16560.0;

/// openmx-mods' installed worlds: `$OPENMX_WORLDS`, else the main checkout's mods folder.
fn worlds() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("OPENMX_WORLDS") {
        return Some(PathBuf::from(dir));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("git").current_dir(&root).args(["rev-parse", "--path-format=absolute", "--git-common-dir"]).output().ok()?;
    let common = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
    Some(common.parent()?.join("mods"))
}

/// A pad tape of `frames` frames of full throttle (Cross on full pressure).
fn gas(frames: usize) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape gas\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md153-world-moving");
    std::fs::create_dir_all(&dir).expect("the scratch folder");
    let path = dir.join(format!("gas-{frames}.tape"));
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The shell headless on world_mxu_free from `--trace-at at` on the gas for `frames` frames: what
/// it said, or `None` with no disc or no world to ride.
fn ride(at: &str, frames: usize) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let mods = worlds().filter(|m| m.join("tracks/world_mxu_free/mod.json").is_file());
    let Some(mods) = mods else {
        eprintln!("skipped: no world_mxu_free in $OPENMX_WORLDS or the main checkout's mods/");
        return None;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--track", "world_mxu_free", "--mods", mods.to_str().unwrap()])
        .args(["--size", "640x448", "--overlay", "none", "--controls", "original"])
        .args(["--trace-at", at, "--play", gas(frames).to_str().unwrap(), "--frames", &frames.to_string()])
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

#[test]
fn full_gas_into_the_trains_side_is_thrown_there() {
    // tests/train.rs's user's path, 148 ft east of the line in free06, moved into the world. Before
    // MD153 the bike rode through the train untouched.
    let Some(said) = ride(&format!("{},3714,280", 2527.0 + FREE06_X), 240) else { return };
    assert!(said.contains("world moving: "), "no world moving list:\n{said}");
    let at = end(&said);
    // How far east of the train's line, in free06's own feet (tests/train.rs's `east_of_line`).
    let east = (at[0] - FREE06_X - 2408.0) * 0.98 - (at[2] - 3897.0) * 0.17;
    assert!(east > -6.0, "the bike ended {:.1} ft past the line, at {at:?}:\n{said}", -east);
    assert!(!wrecks(&said).is_empty(), "the train never threw the rider:\n{said}");
}

#[test]
fn full_gas_into_the_beach_balls_moves_them() {
    // Sixty feet west of free09's beach ball at (5038, 3538), riding east at it: before MD153 the
    // bike rode through it and nothing moved.
    let Some(said) = ride(&format!("{},3538,90", 4978.0 + FREE09_X), 120) else { return };
    let moved: Vec<&str> = said.lines().filter_map(|l| l.strip_prefix("prop moved: ")).collect();
    assert!(moved.iter().any(|m| m.starts_with("BeachBall ")), "no beach ball moved: {moved:?}\n{said}");
    assert!(!said.contains("bodies with no model in the picture"), "a knocked body has no prop in the picture:\n{said}");
}
