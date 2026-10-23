//! MD15: a world's whole picture — every tile's props, not only the sky tile's, each stood where its
//! tile is.
//!
//! The world is written by the test: three tiles in a row along `x`, 720 feet apart, each a track
//! mod of its own whose scene places one prop at the same spot in its own feet. The prop's model is
//! the disc's beach ball, copied into each tile's folder, so the ride skips without the disc; and
//! since what is read is where the GPU stands the props, it skips without a GPU too. The shell starts
//! on the first tile with a reach of 600 feet to load, which takes in the second tile (520 feet off)
//! and not the third (1240).
//!
//! What is proved: the first and second tiles' props are drawn, each at its own spot moved by its
//! tile's offset (the second's 720 feet along `x`), and the third tile, out of reach, has none.
//! Dropping a far tile and building it again as a rider comes back is proved by the streams' own
//! test (`world_dress::tests`), which drives them out and back without a shell.

use game_render::disc::Disc;
use game_sim::tiles::generate;
use std::path::{Path, PathBuf};
use std::process::Command;

const SPACING: f32 = 9.0;
/// Where each tile's prop stands in its own feet.
const PROP: [f32; 2] = [100.0, 150.0];

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// The beach ball's model off the disc, or why not.
fn beach_ball() -> Result<Vec<u8>, String> {
    let mut disc = Disc::open_configured()?;
    let mut names: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().ends_with(".res")).collect();
    // The freeride maps hold it; look there first.
    names.sort_by_key(|n| !n.to_ascii_lowercase().contains("free10"));
    for name in names {
        if let Ok(bytes) = disc.res(&name).and_then(|r| r.read_suffix("beachball.psg")) {
            return Ok(bytes);
        }
    }
    Err("no beachball.psg on the disc".into())
}

/// A `mods/` folder holding the world, `md15_row`.
fn mods_folder(test: &str, ball: &[u8]) -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md15").join(test).join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let world = mods.join("tracks").join("md15_row");
    let mut tiles = Vec::new();
    for col in 0..3u32 {
        let (name, at) = (format!("t{col}"), 720 * col);
        let stem = name.to_uppercase();
        let dir = world.join("tiles").join(&name);
        write(
            &dir,
            "mod.json",
            format!(r#"{{"standard": 1, "kind": "track", "id": "md15_{name}", "name": "MD15 {name}", "source": {{"game": "Real elevation"}}, "version": "1.0.0", "author": "crates/openmx/tests/world_picture.rs", "permission": "public-domain", "todo": []}}"#),
        );
        let scene = [
            "[SceneInfo]".to_string(),
            format!("SceneName=MD15 {name}"),
            "DefaultStartPosition=200.00,40.00,200.00".into(),
            "DefaultStartDirection=1.00,0.00,0.00".into(),
            "[Environment]".into(),
            format!("TerrainFile={stem}.trn"),
            "TerrainWidth=4".into(),
            "TerrainBreadth=4".into(),
            format!("TerrainWidthScale={SPACING:.2}"),
            format!("TerrainBreadthScale={SPACING:.2}"),
            "TerrainBorderWidth=0.00".into(),
            "MinDivisionSize=96.00".into(),
            "[Fog]".into(),
            "ColorRGB=150,151,139".into(),
            "[Model1]".into(),
            "SLT=BeachBall".into(),
            format!("Position={:.2},40.00,{:.2}", PROP[0], PROP[1]),
            "LookVector=1.000000,0.000000,0.000000".into(),
            String::new(),
        ]
        .join("\r\n");
        write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
        let ox = f64::from(at);
        let sp = f64::from(SPACING);
        let terrain = generate::terrain(&name, (4, 4), SPACING, |x, z| 40.0 + 1.5 * ((ox + f64::from(x) * sp) / 140.0).sin() * (f64::from(z) * sp / 90.0).cos(), |_, _| 1);
        write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), terrain);
        write(&dir, "geom/beachball.psg", ball);
        tiles.push(format!(r#"{{"tile": "{name}", "at": [{at}.0, 0.0], "keep": [0, 0, 63, 63]}}"#));
    }
    write(
        &world,
        "mod.json",
        format!(
            r#"{{
  "standard": 1, "kind": "world", "id": "md15_row", "name": "MD15 three tiles",
  "source": {{"game": "Combined", "games": ["Real elevation"]}},
  "version": "1.0.0", "author": "crates/openmx/tests/world_picture.rs", "permission": "public-domain",
  "tiles": [{}],
  "joins": [],
  "drop": [], "sky": "t0", "spawn": {{"tile": "t0"}}, "todo": []
}}"#,
            tiles.join(", ")
        ),
    );
    mods
}

/// One frame of the shell on the world from its start line: what it said, or `None` (and why)
/// without the disc or a GPU.
fn shell(test: &str) -> Option<String> {
    let ball = match beach_ball() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let mods = mods_folder(test, &ball);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "md15_row", "--mods", mods.to_str().unwrap()])
        .args(["--size", "320x224", "--overlay", "none", "--controls", "original"])
        .args(["--world-reach", "600,900", "--frames", "1"])
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    if !said.lines().any(|l| l == "renderer: gpu") {
        eprintln!("skipped: no GPU, so no props read back");
        return None;
    }
    Some(said)
}

/// What the shell said of `tile`'s props on the GPU: how many, and the lowest `x` and `z`.
fn drawn(said: &str, tile: &str) -> Option<(usize, f32, f32)> {
    let l = said.lines().find_map(|l| l.strip_prefix(&format!("world props drawn: {tile} ")))?;
    let (n, rest) = l.split_once(" props, lowest x ")?;
    let (x, z) = rest.split_once(", lowest z ")?;
    Some((n.parse().ok()?, x.parse().ok()?, z.parse().ok()?))
}

#[test]
fn every_near_tiles_props_are_drawn_at_its_offset() {
    let Some(said) = shell("offset") else { return };
    for (tile, at) in [("t0", 0.0), ("t1", 720.0)] {
        let got = drawn(&said, tile).unwrap_or_else(|| panic!("no props drawn for {tile}:\n{said}"));
        assert_eq!(got, (1, PROP[0] + at, PROP[1]), "{tile}'s prop, moved by its offset:\n{said}");
        assert!(said.contains(&format!("world props: {tile} built, 1 props")), "{tile} built:\n{said}");
    }
    assert_eq!(drawn(&said, "t2"), None, "t2 is out of reach and dressed:\n{said}");
}
