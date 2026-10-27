//! MD50: a wrapped world starts the rider on its middle tile, as far from the seams as it can be.
//!
//! The world file's `spawn` names a tile (stitch has written the first tile whose start is kept,
//! a corner map on world_mxu_free); on a world whose `wrap` is on, the shell starts instead on the
//! tile whose kept rectangle's middle is nearest the middle of the world's extent, at that tile's
//! own start ([`openmx::worlds::WorldMod::middle_tile`]). A start in world feet is kept, and so is
//! the sky: the home tile stays the file's.
//!
//! * A generated 3x3 world of flat 124-foot tiles, its file's spawn the corner tile `a`: wrapped,
//!   the shell says it starts on the middle tile `e` and a ride's every tick stands on `e`; without
//!   the switch it starts on `a` (60 feet from the world's outer edge, which throws it at once). The rides skip without the disc, since the bike is the disc's.
//! * The installed wrapped worlds (`$OPENMX_WORLDS`, or `~/repos/openmx/mods`), ignored: each starts
//!   on the tile nearest its middle as stitched on 2026-10-27, and its ride's first tick is by that start.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A tile's kept side: 31 cells of 4 feet. The tiles abut, a side apart.
const SIDE: f32 = 124.0;

const NAMES: [&str; 9] = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A `mods/` folder holding the 3x3 world wrapped, `md50_round`, and without the switch, `md50_edged`.
fn mods_folder(test: &str) -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md50").join(test).join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    for (id, wrap) in [("md50_round", r#""wrap": "xz", "#), ("md50_edged", "")] {
        let world = mods.join("tracks").join(id);
        let mut listed = Vec::new();
        for (n, name) in NAMES.iter().enumerate() {
            let at = [(n % 3) as f32 * SIDE, (n / 3) as f32 * SIDE];
            let stem = format!("MD50{}", name.to_uppercase());
            let dir = world.join("tiles").join(name);
            write(&dir, "mod.json", format!(r#"{{"standard": 1, "kind": "track", "id": "md50_{name}", "name": "MD50 {name}"}}"#));
            let scene = format!("[SceneInfo]\r\nSceneName=MD50 {name}\r\nDefaultStartPosition=60.00,10.00,60.00\r\nDefaultStartDirection=1.00,0.00,0.00\r\n[Environment]\r\nTerrainFile={stem}.trn\r\nTerrainWidth=2\r\nTerrainBreadth=2\r\nTerrainWidthScale=4.00\r\nTerrainBreadthScale=4.00\r\nTerrainBorderWidth=0.00\r\nMinDivisionSize=96.00\r\n[Fog]\r\nColorRGB=150,151,139\r\n");
            write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), game_sim::tiles::generate::terrain(name, (2, 2), 4.0, |_, _| 10.0, |_, _| 1));
            listed.push(format!(r#"{{"tile": "{name}", "at": [{}, {}], "keep": [0, 0, 31, 31]}}"#, at[0], at[1]));
        }
        let json = format!(
            r#"{{"standard": 1, "kind": "world", "id": "{id}", "name": "MD50 grid", {wrap}"tiles": [{}], "joins": [], "drop": [], "sky": "a", "spawn": {{"tile": "a"}}, "todo": []}}"#,
            listed.join(", ")
        );
        write(&world, "mod.json", json);
    }
    mods
}

/// The shell's output for a world in a `mods/` folder, or `None` without the disc.
fn shell(mods: &Path, track: &str, args: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--track", track, "--mods", mods.to_str().unwrap()])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    Some(said)
}

fn line<'a>(said: &'a str, prefix: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no `{prefix}` line:\n{said}"))
}

/// The tile the shell says the ride starts on, and where.
fn spawn(said: &str) -> (String, [f32; 2]) {
    let l = line(said, "world spawn: ");
    let w: Vec<&str> = l.split_whitespace().collect();
    let at: Vec<f32> = w[4].split(',').map(|v| v.parse().unwrap()).collect();
    (w[2].to_string(), [at[0], at[1]])
}

/// `--trace-feel` rows' `(frame, x, z, wreck)`, standing.
fn ticks(said: &str) -> Vec<(usize, f32, f32, bool)> {
    let rows: Vec<_> = said
        .lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let frame = w.first()?.parse::<usize>().ok()?;
            (w.len() == 12).then(|| (frame, w[5].parse().unwrap(), w[7].parse().unwrap(), w[11] == "1"))
        })
        .collect();
    assert!(!rows.is_empty(), "no trace rows:\n{said}");
    rows
}

/// Wrapped, the grid starts on its middle tile `e`, at `e`'s own start, and the ride stands there;
/// with edges it starts where the file says, on the corner tile `a`, which its outer edge throws.
#[test]
fn a_wrapped_world_rides_from_its_middle_tile() {
    let mods = mods_folder("ride");
    for (id, tile, cell) in [("md50_round", "e", [1.0, 1.0]), ("md50_edged", "a", [0.0, 0.0])] {
        let Some(said) = shell(&mods, id, &["--trace", "30", "--trace-feel"]) else { return };
        let corner: [f32; 2] = [cell[0] * SIDE, cell[1] * SIDE];
        assert_eq!(spawn(&said), (tile.to_string(), [corner[0] + 60.0, corner[1] + 60.0]), "{id}");
        // 60 feet in from an outer edge, the open world's start is thrown by it at once (MD8).
        if id == "md50_edged" {
            continue;
        }
        for (frame, x, z, wreck) in ticks(&said) {
            assert!(!wreck, "{id}: down on tick {frame}");
            assert!((corner[0]..corner[0] + SIDE).contains(&x) && (corner[1]..corner[1] + SIDE).contains(&z), "{id}: tick {frame} at {x},{z} is off {tile}");
        }
    }
}

/// openmx-mods' wrapped worlds as installed: each starts on the tile nearest its middle (as stitched
/// on 2026-10-27; a restitched world may move it), and its ride's first tick stands by that start.
#[test]
#[ignore = "needs openmx-mods' stitched worlds in $OPENMX_WORLDS/tracks (or ~/repos/openmx/mods), and the disc; release build"]
fn the_installed_wrapped_worlds_start_on_their_middle_tile() {
    let mods = std::env::var_os("OPENMX_WORLDS").map(PathBuf::from).unwrap_or_else(|| Path::new(&std::env::var("HOME").unwrap()).join("repos/openmx/mods"));
    for (id, middle) in [("world_mxu_free", "mxu_free10"), ("world_mxu", "mxu_nat17"), ("world_ultra", "mxvatv_nat06")] {
        if !mods.join("tracks").join(id).is_dir() {
            eprintln!("{id}: not installed");
            continue;
        }
        let said = shell(&mods, id, &["--trace", "2", "--trace-feel"]).expect("the disc");
        let (tile, at) = spawn(&said);
        println!("{id}: {} — starts on {tile} at {},{}", line(&said, "world: "), at[0], at[1]);
        assert_eq!(tile, middle, "{id}");
        let (_, x, z, _) = ticks(&said)[0];
        // A national lines the bike up on its gate beside the scene's start (22 feet on mxu_nat17).
        assert!((x - at[0]).hypot(z - at[1]) < 50.0, "{id}: the first tick at {x},{z}, not by the start {at:?}");
    }
}
