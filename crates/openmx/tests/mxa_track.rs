//! MD259: an MXA Track Studio `.mxatrack` dropped in `mods/tracks/` loads as a track mod and rides.
//!
//! No real track exists on this machine, so the package is made from the format
//! (`stitch::mxa::package`): 720 feet a side, as Track Studio's default is wide, of rolling ground
//! with a dip below MXA's 0. Only the file goes in the folder; `--track mxa_<id>` makes the mod
//! (`track_mods::find`) and rides it. The ride skips without the disc, since the bike and its
//! physics are the disc's.

use std::path::Path;
use std::process::Command;

const SIDE: f32 = 720.0;
const N: u32 = 257;

/// The ground in MXA's frame (0 at the middle), feet.
fn relief(x: f32, z: f32) -> f32 {
    3.0 * (x / 60.0).sin() * (z / 80.0).cos() - 2.0
}

fn package() -> Vec<u8> {
    let at = |i: u32| i as f32 / (N - 1) as f32 * SIDE - SIDE / 2.0;
    let t = stitch::mxa::Track {
        id: "md259-ride".into(),
        title: "MD259 ride".into(),
        author: "crates/openmx/tests/mxa_track.rs".into(),
        environment: "Outdoor".into(),
        venue: "SX01".into(),
        ground_texture: "".into(),
        description: "".into(),
        location: "".into(),
        width: SIDE,
        depth: SIDE,
        resolution: N,
        rows: N,
        surface: 0,
        heights: (0..N * N).map(|i| relief(at(i % N), at(i / N))).collect(),
        surfaces: None,
        spawn: [-200.0, relief(-200.0, 0.0) + 4.0, 0.0],
        direction: [1.0, 0.0, 0.0],
    };
    stitch::mxa::package(&t, "")
}

/// From the scene's own start, on the gas for 150 ticks: the bike rides on along +x, on the
/// package's ground (lifted by its lowest point, 5 feet), never through it and never down. (By
/// tick 175 it is in the edge's 150-foot band and thrown, as on any map.)
#[test]
fn a_dropped_in_mxatrack_rides() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md259").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    std::fs::create_dir_all(mods.join("tracks")).unwrap();
    std::fs::write(mods.join("tracks").join("ride.mxatrack"), package()).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "mxa_md259ride", "--mods", mods.to_str().unwrap()])
        .args(["--trace", "150", "--trace-gas", "--trace-feel"])
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    assert!(mods.join("tracks/mxa_md259ride/mod.json").is_file(), "no mod made:\n{said}");
    let mut rows = Vec::new();
    for l in said.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        let Some(frame) = w.first().and_then(|f| f.parse::<usize>().ok()) else { continue };
        if w.len() != 12 {
            continue;
        }
        let f = |i: usize| w[i].parse::<f32>().unwrap();
        rows.push((frame, f(5), f(6), f(7), w[11] == "1"));
    }
    assert_eq!(rows.len(), 150, "trace rows:\n{said}");
    let lift = 5.0;
    let (start, end) = (rows[0], rows[149]);
    assert!((start.1 - 160.0).abs() < 5.0 && (start.3 - 360.0).abs() < 5.0, "started at {start:?}");
    assert!(end.1 - start.1 > 100.0 && (end.3 - start.3).abs() < 40.0, "rode from {start:?} to {end:?}");
    for &(frame, x, y, z, wreck) in &rows {
        assert!(!wreck, "tick {frame}: down at ({x}, {y}, {z})");
        let ground = relief(x - SIDE / 2.0, z - SIDE / 2.0) + lift;
        assert!(y > ground - 1.0 && y < ground + 12.0, "tick {frame}: at y {y} over ground at {ground} ({x}, {z})");
    }
}
