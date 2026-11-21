//! S266: in a world each car parks at its own track's tile. The lot is parked by the home tile's
//! `machine.txt` (world_mxu_free's is free06, which parks no car), so before S266 no car stood in a
//! world. Now each car stands where its own track parks it (S245: the truck on free09, the baja on
//! free07, the buggy on free11), moved by that tile's offset in the world: the shell's
//! `--trace-picture` parked line in world_mxu_free is its spot on the track alone, plus the tile's
//! `at`, at about the same height (F74: each car settles on its own; its bay is not drawn). A shot of the truck from behind its bay is written and was looked at by eye.
//! Skips without the disc or the installed world.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Each car: its track, its tile in world_mxu_free and its model.
const CARS: [(&str, &str, &str); 3] = [("free09", "mxu_free09", "monstertruck"), ("free07", "mxu_free07", "baja"), ("free11", "mxu_free11", "buggy")];

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

/// The shell headless with `--trace-picture` for a few frames, plus `more`: what it said.
fn ride(args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--size", "640x448", "--overlay", "none", "--trace-picture", "--frames", "6"])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    said
}

/// Where the first `picture parked:` line of `model` stands it (F72; the see-through bay is not
/// drawn inside it since F74).
fn bay(said: &str, model: &str) -> [f32; 3] {
    let key = format!(" gpu car:{model} at ");
    let line = said.lines().filter(|l| l.starts_with("picture parked: ")).find_map(|l| l.split_once(key.as_str()).map(|(_, at)| at));
    let line = line.unwrap_or_else(|| panic!("no {model} parked:\n{said}"));
    let w: Vec<f32> = line.split_whitespace().take(3).map(|n| n.parse().expect("a number")).collect();
    [w[0], w[1], w[2]]
}

#[test]
fn world_mxu_free_parks_each_car_on_its_own_tile() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let Some(mods) = worlds().filter(|m| m.join("tracks/world_mxu_free/mod.json").is_file()) else {
        eprintln!("skipped: no world_mxu_free in $OPENMX_WORLDS or the main checkout's mods/");
        return;
    };
    let truck = tile_at(&mods, "mxu_free09");
    let shot = Path::new(env!("CARGO_TARGET_TMPDIR")).join("s266-world-truck.png");
    // From free09's tile, by the truck; the shot looks along +z at its bay from behind it.
    let start = format!("{},{},45", 4560.0 + truck[0], 3800.0 + truck[1]);
    let pose = format!("{},{},{},0,-12,60", 4594.0 + truck[0], 144.5, 3804.0 + truck[1]);
    let world = ride(&["--track", "world_mxu_free", "--mods", mods.to_str().unwrap(), "--trace-at", &start, "--shot", shot.to_str().unwrap(), "--shot-frame", "4", "--shot-pose", &pose]);
    assert!(world.contains("buggy, monster, baja parked here"), "not every car is parked in the world:\n{world}");
    assert!(shot.is_file(), "no shot at {}", shot.display());
    for (track, tile, model) in CARS {
        let own = bay(&ride(&["--track", track]), model);
        let at = tile_at(&mods, tile);
        let got = bay(&world, model);
        let want = [own[0] + at[0], own[1], own[2] + at[1]];
        // Each car is settled on the ground where it is put (P378), so it rests a little apart
        // in the world, far from the origin: up to 0.16 ft across, 0.05 ft up on free07's baja.
        assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.25), "{model}: parked at {got:?} in the world, {want:?} is {track}'s spot moved by {tile}'s {at:?}");
    }
    // And no car's bay is drawn inside its parked car (F74).
    for line in world.lines().filter(|l| l.starts_with("picture bay: car:")) {
        let w: Vec<&str> = line.split_whitespace().collect();
        let at: Vec<f32> = w[4..7].iter().map(|n| n.parse().expect("a number")).collect();
        let rest = bay(&world, w[2].trim_start_matches("car:"));
        assert!((at[0] - rest[0]).hypot(at[2] - rest[2]) > 10.0, "a bay is drawn inside its parked car at {rest:?}: {line}");
    }
}
