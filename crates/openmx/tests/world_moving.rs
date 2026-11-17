//! MD153: on a world the tiles' path cars, knockable props and animated toys were only in the
//! picture (the user: "in mx world trucks and props have no collision"). `game_sim::tiles` now holds
//! every tile's in one moving list, as a track's `World::moving`, stepped with the cars.
//!
//! These ride the shell's own binary, headless on world_mxu_free, from places in free06 and free09
//! moved by their tiles' offsets, read from the installed mod.json (K56: MD103's reinstall moved
//! every tile 720 ft in x and z), each the place a free06 or free09 test rides alone:
//!
//! - full gas square at Dallas's train (tests/train.rs's user's path): the bike is thrown at the
//!   train's side and never comes out the far side;
//! - full gas at free09's beach balls by the pool: the balls move;
//! - full gas head on at free06's company truck (S207's ctruck, a `HasPhysics` car) on its line: the
//!   woken truck is drawn where its body is, not on its clip (MD153a).
//!
//! The world is openmx-mods' stitch of the user's disc: `$OPENMX_WORLDS/tracks/world_mxu_free`, else
//! the main checkout's `mods/` (as tests/disc_worlds.rs looks); without it, or the disc, they skip.

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Where the installed world puts `tile`, `[x, z]`: its mod.json's `tiles[].at`.
fn tile_at(mods: &Path, tile: &str) -> [f32; 2] {
    let text = std::fs::read_to_string(mods.join("tracks/world_mxu_free/mod.json")).expect("the world's mod.json");
    let spec: serde_json::Value = serde_json::from_str(&text).expect("the world's mod.json is json");
    let tiles = spec["tiles"].as_array().expect("the world lists its tiles");
    let at = tiles.iter().find(|t| t["tile"] == tile).unwrap_or_else(|| panic!("no {tile} in the world"))["at"].clone();
    [at[0].as_f64().expect("an x") as f32, at[1].as_f64().expect("a z") as f32]
}

/// A pad tape of `frames` frames of full throttle (Cross on full pressure).
fn gas(frames: usize) -> PathBuf {
    tape(0, frames)
}

/// A pad tape of `still` frames with nothing held, then `held` of full throttle.
fn tape(still: usize, held: usize) -> PathBuf {
    let frames = still + held;
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape gas\n");
    let gas = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    let none = format!("0079ffff7f7f7f7f{}", "00".repeat(24));
    for f in 0..frames {
        let report = if f < still { &none } else { &gas };
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md153-world-moving");
    std::fs::create_dir_all(&dir).expect("the scratch folder");
    let path = dir.join(format!("gas-{still}-{held}.tape"));
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The shell headless on world_mxu_free from `--trace-at` `[x, z, degrees]` in `tile`'s own feet,
/// on the gas for `frames` frames: what it said and the tile's offset, or `None` with no disc or no
/// world to ride.
fn ride(tile: &str, at: [f32; 3], frames: usize) -> Option<(String, [f32; 2])> {
    ride_tape(tile, at, frames, gas(frames))
}

/// [`ride`] on `tape`.
fn ride_tape(tile: &str, at: [f32; 3], frames: usize, tape: PathBuf) -> Option<(String, [f32; 2])> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let mods = worlds().filter(|m| m.join("tracks/world_mxu_free/mod.json").is_file());
    let Some(mods) = mods else {
        eprintln!("skipped: no world_mxu_free in $OPENMX_WORLDS or the main checkout's mods/");
        return None;
    };
    let off = tile_at(&mods, tile);
    let at = format!("{},{},{}", at[0] + off[0], at[1] + off[1], at[2]);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--track", "world_mxu_free", "--mods", mods.to_str().unwrap()])
        .args(["--size", "640x448", "--overlay", "none", "--controls", "original"])
        .args(["--trace-at", &at, "--play", tape.to_str().unwrap(), "--frames", &frames.to_string()])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some((said, off))
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
    let Some((said, off)) = ride("mxu_free06", [2527.0, 3714.0, 280.0], 240) else { return };
    assert!(said.contains("world moving: "), "no world moving list:\n{said}");
    let at = end(&said);
    // How far east of the train's line, in free06's own feet (tests/train.rs's `east_of_line`).
    let east = (at[0] - off[0] - 2408.0) * 0.98 - (at[2] - off[1] - 3897.0) * 0.17;
    assert!(east > -6.0, "the bike ended {:.1} ft past the line, at {at:?}:\n{said}", -east);
    assert!(!wrecks(&said).is_empty(), "the train never threw the rider:\n{said}");
}

#[test]
fn full_gas_into_the_beach_balls_moves_them() {
    // Sixty feet west of free09's beach ball at (5038, 3538), riding east at it: before MD153 the
    // bike rode through it and nothing moved.
    let Some((said, _)) = ride("mxu_free09", [4978.0, 3538.0, 90.0], 120) else { return };
    let moved: Vec<&str> = said.lines().filter_map(|l| l.strip_prefix("prop moved: ")).collect();
    assert!(moved.iter().any(|m| m.starts_with("BeachBall ")), "no beach ball moved: {moved:?}\n{said}");
    assert!(!said.contains("bodies with no model in the picture"), "a knocked body has no prop in the picture:\n{said}");
}

#[test]
fn a_woken_truck_is_drawn_where_its_body_is() {
    // free06's company truck `Car22` (a `HasPhysics` car) drives west along free06's z 2960 about
    // 28 ft/s: at its x 4001 a second in, 3889 at 5 s. The bike rides east at it on the gas from the
    // start and strikes it head on, which wakes it; 10 s, so it is not yet put back (`ResetTime`,
    // 10 s after it comes to rest).
    let Some((said, _)) = ride_tape("mxu_free06", [3806.0, 2960.0, 90.0], 300, tape(0, 300)) else { return };
    let woken: Vec<&str> = said.lines().filter_map(|l| l.strip_prefix("woken car: ")).collect();
    let cars = said.lines().find(|l| l.starts_with("physics cars: ")).unwrap_or("no physics cars line");
    assert!(!woken.is_empty(), "no truck woke; {cars}:\n{said}");
    for w in &woken {
        eprintln!("woken car: {w}");
        // `<model> body x,z drawn x,z`
        let words: Vec<&str> = w.split_whitespace().collect();
        let xz = |s: &str| -> [f32; 2] {
            let v: Vec<f32> = s.split(',').map(|n| n.parse().expect("a number")).collect();
            [v[0], v[1]]
        };
        let (body, drawn) = (xz(words[2]), xz(words[4]));
        // Blended from the tick before at most: a tick of a truck's slide.
        let off = (body[0] - drawn[0]).hypot(body[1] - drawn[1]);
        assert!(off < 3.0, "{w}: drawn {off:.1} ft from its body:\n{said}");
    }
}

