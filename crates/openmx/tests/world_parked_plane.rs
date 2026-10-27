//! MD32: a world ride's parked biplanes stand in world feet, inside the tile the ride starts on.
//!
//! The world is written by the test: two tiles from one rolling height function in world feet
//! ([`game_sim::tiles::generate`]), `a` at the origin and `b` far from it at [`B_AT`]. The ride
//! starts on `b` and is lit by it, and `b`'s scene is stemmed `FREE06`, so the disc's
//! `tracks\free06\machine.txt` parks the lot: a file in free06's own feet, whose `[Biplane1]` stands
//! near 2304, 2281 and would land on no tile of this world if it were not moved (MD24's
//! `vehicle::world_spots`). Two players, so both the first seat's plane and the second's are
//! checked. Skips without the disc, since the bike, the plane and the machine file are the disc's.

use game_sim::tiles::generate;
use std::path::{Path, PathBuf};
use std::process::Command;

const SPACING: f32 = 18.0;
/// Root cells a side (sixteen samples each): 160 samples, 2862 feet, so free06's spots in its own
/// feet are on `b`'s ground too, which is what each spot's clearance is measured over.
const CELLS: u32 = 10;
/// Where tile `b` lies in the world, feet.
const B_AT: [u32; 2] = [6000, 4000];

fn relief(x: f64, z: f64) -> f64 {
    40.0 + 1.5 * (x / 140.0).sin() * (z / 90.0).cos()
}

fn side() -> f32 {
    (CELLS * 16 - 1) as f32 * SPACING
}

fn terrain(name: &str, at: [u32; 2]) -> Vec<u8> {
    let (ox, oz) = (f64::from(at[0]), f64::from(at[1]));
    let sp = f64::from(SPACING);
    generate::terrain(name, (CELLS, CELLS), SPACING, |x, z| relief(ox + f64::from(x) * sp, oz + f64::from(z) * sp), |_, _| 1)
}

/// A tile's scene, its start line in the middle of it facing `+x`.
fn scene(name: &str, stem: &str) -> String {
    [
        "[SceneInfo]",
        &format!("SceneName=MD32 {name}"),
        "DefaultStartPosition=570.00,40.00,570.00",
        "DefaultStartDirection=1.00,0.00,0.00",
        "[Environment]",
        &format!("TerrainFile={stem}.trn"),
        &format!("TerrainWidth={CELLS}"),
        &format!("TerrainBreadth={CELLS}"),
        &format!("TerrainWidthScale={SPACING:.2}"),
        &format!("TerrainBreadthScale={SPACING:.2}"),
        "TerrainBorderWidth=0.00",
        "MinDivisionSize=96.00",
        "[Fog]",
        "ColorRGB=150,151,139",
        "",
    ]
    .join("\r\n")
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A `mods/` folder holding the world, `md32_world`.
fn mods_folder() -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md32").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let world = mods.join("tracks").join("md32_world");
    let keep = CELLS * 16 - 1;
    let mut tiles = Vec::new();
    for (name, stem, at) in [("a", "MD32A", [0, 0]), ("b", "FREE06", B_AT)] {
        let dir = world.join("tiles").join(name);
        write(
            &dir,
            "mod.json",
            format!(
                r#"{{"standard": 1, "kind": "track", "id": "md32_{name}", "name": "MD32 {name}", "source": {{"game": "Real elevation"}}, "version": "1.0.0", "author": "crates/openmx/tests/world_parked_plane.rs", "permission": "public-domain", "todo": []}}"#
            ),
        );
        write(&dir, &format!("Env/{stem}/{stem}.scn"), scene(name, stem));
        write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), terrain(name, at));
        tiles.push(format!(r#"{{"tile": "{name}", "at": [{}.0, {}.0], "keep": [0, 0, {keep}, {keep}]}}"#, at[0], at[1]));
    }
    write(
        &world,
        "mod.json",
        format!(
            r#"{{
  "standard": 1, "kind": "world", "id": "md32_world", "name": "MD32 a far tile",
  "source": {{"game": "Combined", "games": ["Real elevation"]}},
  "version": "1.0.0", "author": "crates/openmx/tests/world_parked_plane.rs", "permission": "public-domain",
  "tiles": [{}],
  "joins": [],
  "drop": [], "sky": "b", "spawn": {{"tile": "b"}}, "todo": []
}}"#,
            tiles.join(", ")
        ),
    );
    mods
}

/// The first three numbers after `after` on the line starting `prefix`.
fn numbers(said: &str, prefix: &str, after: &str) -> Vec<f32> {
    let l = said.lines().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no `{prefix}` line:\n{said}"));
    let tail = &l[l.find(after).unwrap() + after.len()..];
    tail.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|w| w.parse().ok()).take(3).collect()
}

#[test]
fn a_world_rides_parked_plane_stands_within_its_tiles_rectangle() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = mods_folder();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--track", "md32_world", "--mods", mods.to_str().unwrap()])
        .args(["--size", "320x224", "--overlay", "none", "--players", "2", "--frames", "1"])
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    let (x0, z0) = (B_AT[0] as f32, B_AT[1] as f32);
    let (x1, z1) = (x0 + side(), z0 + side());
    let start = numbers(&said, "start:", "start:");
    assert!((start[0] - (x0 + 570.0)).abs() < 1.0 && (start[2] - (z0 + 570.0)).abs() < 1.0, "the start is not b's: {start:?}");
    let planes = [numbers(&said, "biplane: parked by", " at "), numbers(&said, "player 2: biplane parked at", " at ")];
    for (seat, p) in planes.iter().enumerate() {
        assert!(
            (x0..=x1).contains(&p[0]) && (z0..=z1).contains(&p[2]),
            "seat {}'s plane at {p:?} is outside b's {x0}..{x1} x {z0}..{z1}",
            seat + 1
        );
        // Over b's ground by the clearance free06's file gives the spot over its own map's (under
        // 100 feet over this flat tile), not under it nor off in the sky over no ground.
        let over = p[1] - relief(f64::from(p[0]), f64::from(p[2])) as f32;
        assert!((0.0..150.0).contains(&over), "seat {}'s plane at {p:?} stands {over} feet over b's ground", seat + 1);
    }
    println!("start {start:?}, planes {planes:?}");
}
