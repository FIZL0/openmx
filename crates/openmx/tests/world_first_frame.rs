//! P150: a world's first frame was its slowest. On world_mxu tick 1 cost 26 ms (MD28): the ground's
//! index lists for the first view built in its frame, the first GPU frame's pipelines and uploads,
//! and — since — the first tick cutting the world's near shapes for the bike's searches.
//!
//! Now the shell draws the ride's first picture once offscreen before the ride's clock starts, while
//! the window still shows the loading screen, and cuts the near shapes about each rider at the load
//! (`--warm on|off`, on by default).
//!
//! What is proved: a headless ride on a generated nine-tile world with `--timings` has no slow frame
//! at tick 1 and says it drew the warm frame, and the ride is the same to the bit with the warm-up
//! and without it. The ignored test rides the user's world_mxu as MD28 measured it. The rides skip
//! without the disc.

use game_sim::tiles::generate;
use std::path::{Path, PathBuf};
use std::process::Command;

const SPACING: f32 = 9.0;

/// Rolling ground, as MD17's world has it.
fn relief(x: f64, z: f64) -> f64 {
    40.0 + 1.5 * (x / 140.0).sin() * (z / 90.0).cos()
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn scene(name: &str, cells: (u32, u32)) -> String {
    [
        "[SceneInfo]",
        &format!("SceneName=P150 {name}"),
        "DefaultStartPosition=200.00,40.00,200.00",
        "DefaultStartDirection=1.00,0.00,0.00",
        "[Environment]",
        &format!("TerrainFile={}.trn", name.to_uppercase()),
        &format!("TerrainWidth={}", cells.0),
        &format!("TerrainBreadth={}", cells.1),
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

/// A `mods/` folder holding `p150_world`: nine tiles 720 feet apart and the four joins between them.
fn mods_folder() -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p150").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let world = mods.join("tracks").join("p150_world");
    let terrain = |name: &str, at: [u32; 2], cells: (u32, u32)| {
        let (ox, oz, sp) = (f64::from(at[0]), f64::from(at[1]), f64::from(SPACING));
        generate::terrain(name, cells, SPACING, |x, z| relief(ox + f64::from(x) * sp, oz + f64::from(z) * sp), |_, _| 1)
    };
    let (mut tiles, mut joins) = (Vec::new(), Vec::new());
    for row in 0..3 {
        for col in 0..3 {
            let (name, at) = (format!("t{col}{row}"), [720 * col, 720 * row]);
            let (dir, stem) = (world.join("tiles").join(&name), name.to_uppercase());
            write(
                &dir,
                "mod.json",
                format!(
                    r#"{{"standard": 1, "kind": "track", "id": "p150_{name}", "name": "P150 {name}", "source": {{"game": "Real elevation"}}, "version": "1.0.0", "author": "crates/openmx/tests/world_first_frame.rs", "permission": "public-domain", "todo": []}}"#
                ),
            );
            write(&dir, &format!("Env/{stem}/{stem}.scn"), scene(&name, (4, 4)));
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), terrain(&name, at, (4, 4)));
            tiles.push(format!(r#"{{"tile": "{name}", "at": [{}.0, {}.0], "keep": [0, 0, 63, 63]}}"#, at[0], at[1]));
        }
    }
    let gaps = [("v0", [432, 0], (3, 15)), ("v1", [1152, 0], (3, 15)), ("h0", [0, 432], (15, 3)), ("h1", [0, 1152], (15, 3))];
    for (name, at, cells) in gaps {
        write(&world, &format!("joins/{name}/{name}.ps2.trn"), terrain(name, at, cells));
        joins.push(format!(r#"{{"join": "{name}", "at": [{}.0, {}.0]}}"#, at[0], at[1]));
    }
    write(
        &world,
        "mod.json",
        format!(
            r#"{{
  "standard": 1, "kind": "world", "id": "p150_world", "name": "P150 nine tiles",
  "source": {{"game": "Combined", "games": ["Real elevation"]}},
  "version": "1.0.0", "author": "crates/openmx/tests/world_first_frame.rs", "permission": "public-domain",
  "tiles": [{}],
  "joins": [{}],
  "drop": [], "sky": "t00", "spawn": {{"tile": "t00"}}, "todo": []
}}"#,
            tiles.join(", "),
            joins.join(", ")
        ),
    );
    mods
}

/// Full throttle and nothing else, `frames` long: Cross down, at full pressure.
fn throttle_tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape full throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text
}

/// The shell headless with `--timings` on `track` from `mods`, on the gas for `frames` ticks: what
/// it said.
fn ride(mods: &Path, track: &str, frames: usize, args: &[&str]) -> String {
    let tape = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p150").join(format!("gas{frames}.tape"));
    write(tape.parent().unwrap(), &format!("gas{frames}.tape"), throttle_tape(frames));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--mods", mods.to_str().unwrap(), "--track", track, "--timings"])
        .args(["--play", tape.to_str().unwrap(), "--frames", &frames.to_string()])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    said
}

fn line<'a>(said: &'a str, start: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(start)).unwrap_or_else(|| panic!("no {start:?} line:\n{said}"))
}

/// The slow-frame lines at tick 1.
fn slow_at_tick_1(said: &str) -> Vec<&str> {
    said.lines().filter(|l| l.contains("slow frame") && l.ends_with("at tick 1")).collect()
}

#[test]
fn a_worlds_first_frame_is_not_slow_and_the_ride_is_the_same() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = mods_folder();
    let size = ["--size", "640x360"];
    let warm = ride(&mods, "p150_world", 60, &size);
    let cold = ride(&mods, "p150_world", 60, &[&size[..], &["--warm", "off"]].concat());
    eprintln!("tick 1 without the warm-up: {:?}", slow_at_tick_1(&cold));
    // The ride is the same either way: the near sets cut early are the ones tick 1 would have cut.
    assert_eq!(line(&warm, "ride hash"), line(&cold, "ride hash"));
    assert_eq!(line(&warm, "end:"), line(&cold, "end:"));
    assert!(cold.lines().all(|l| !l.starts_with("warm frame:")), "--warm off draws no warm frame");
    if !warm.lines().any(|l| l == "renderer: gpu") {
        eprintln!("skipped the first frame's time: no GPU");
        return;
    }
    let drew = line(&warm, "warm frame:");
    let first = warm.lines().position(|l| l.contains("ms a frame") || l.contains("slow frame")).expect("timings");
    assert!(warm.lines().position(|l| l == drew).unwrap() < first, "the warm frame comes before the ride's first");
    let slow = slow_at_tick_1(&warm);
    assert!(slow.is_empty(), "the first frame is slow with the warm-up:\n{}", slow.join("\n"));
}

/// The user's world_mxu as MD28 measured it, headless at 1920x1080: no slow frame at tick 1.
/// Ignored: it needs the user's mods (`P150_MODS`, or `~/repos/openmx/mods`) and a quiet machine,
/// since a frame's stages are wall-clock time, and every frame of world_mxu is over 16.7 ms on a
/// GPU shared with the other slots.
#[test]
#[ignore]
fn world_mxus_first_frame_is_under_a_60hz_frame() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = std::env::var_os("P150_MODS").map(PathBuf::from).unwrap_or_else(|| Path::new(&std::env::var("HOME").unwrap()).join("repos/openmx/mods"));
    if !mods.join("tracks/world_mxu/mod.json").exists() {
        eprintln!("skipped: no world_mxu in {}", mods.display());
        return;
    }
    let said = ride(&mods, "world_mxu", 120, &["--size", "1920x1080"]);
    eprintln!("{}", said.lines().filter(|l| l.starts_with("warm frame") || l.starts_with("world shapes") || l.starts_with("session")).collect::<Vec<_>>().join("\n"));
    let slow = slow_at_tick_1(&said);
    assert!(slow.is_empty(), "world_mxu's first frame is slow:\n{}", slow.join("\n"));
}
