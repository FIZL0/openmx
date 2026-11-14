//! MD136: the quad's own camera, engine and particles. On MX vs. ATV's quad (MD37d) the follow
//! camera reads the pack's `Cam\FC_ATVDefault.pb` — the file MD82a's RAM images show the game's own
//! follow camera loading for the ATV — rather than the bike's track presets, the engine note plays the
//! pack's `Aud/vehicle.aud` through the bike's engine voice, and its tires throw the pack's
//! `Part/atvparticles.dat` off the chassis model's wheel nodes, zoned by its own
//! `Part/atvparticlezones.dat`.
//!
//! The presets half reads the pack's files straight; the ride half rides the binary headless on a
//! hand-written tape, as mxvatv_atv.rs does. Each skips without the pack in this checkout's
//! `mods/machines/`, the ride also with no disc.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use std::path::{Path, PathBuf};
use std::process::Command;

use game_sim::camera::{Aspect, Viewport};

/// FC_ATVDefault.pb's presets, in its order.
const FC_ATV: [&str; 8] = [
    "Race Camera 1",
    "Race Camera 2",
    "Offset Camera Right",
    "Slot Camera Right",
    "Offset Camera Left",
    "Slot Camera Left",
    "Look Back Camera",
    "1P Stunt Camera",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The quad pack's folder; says SKIPPED and `None` when this checkout has none.
fn pack() -> Option<PathBuf> {
    let dir = root().join("mods/machines/mxvatv_atv");
    if !dir.join("mod.json").is_file() {
        eprintln!("SKIPPED: no mods/machines/mxvatv_atv (openmx-mods' import-atv makes it)");
        return None;
    }
    Some(dir)
}

#[test]
fn the_quads_presets_are_fc_atvs() {
    let Some(dir) = pack() else { return };
    let pb = std::fs::read(dir.join("Cam/FC_ATVDefault.pb")).expect("the pack's FC_ATVDefault.pb");
    let ini = std::fs::read(dir.join("Cam/FC_ATV.ini")).expect("the pack's FC_ATV.ini");
    let (presets, from) = chase::atv_presets(Some(&pb), Some(&ini), Aspect::Standard, Viewport::Full);
    let names: Vec<&str> = presets.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(from, "Cam\\FC_ATVDefault.pb");
    assert_eq!(names, FC_ATV);
    // Every screen shape reads them all.
    for (aspect, viewport) in [(Aspect::Wide, Viewport::Full), (Aspect::Standard, Viewport::SplitH), (Aspect::Standard, Viewport::Quad)] {
        let (presets, _) = chase::atv_presets(Some(&pb), Some(&ini), aspect, viewport);
        assert_eq!(presets.len(), FC_ATV.len(), "{aspect:?} {viewport:?}");
    }
    // Without the .pb the .ini's, which names them "Race Cam 1" and on.
    let (presets, from) = chase::atv_presets(None, Some(&ini), Aspect::Standard, Viewport::Full);
    assert_eq!(from, chase::ATV_CAMERA_INI);
    assert!(!presets.is_empty() && presets[0].name.starts_with("Race Cam"), "{:?}", presets.first().map(|p| &p.name));
    // And none with neither: the bike's camera follows.
    assert_eq!(chase::atv_presets(None, None, Aspect::Standard, Viewport::Full).1, "");
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md136-atv-camera-sound");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// `frames` ticks of Cross held at full pressure (the gas), with `switches` (`30 atv`) after them.
fn tape(name: &str, frames: usize, switches: &[&str]) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape Cross held\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    for s in switches {
        text += &format!("switch {s}\n");
    }
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The line starting `start`, or a panic with everything said.
fn line<'a>(said: &'a str, start: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(start)).unwrap_or_else(|| panic!("no `{start}` line:\n{said}"))
}

#[test]
fn the_quad_rides_its_own_camera_engine_and_particles() {
    if pack().is_none() {
        return;
    }
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    // On the gas from tick 30: by 130 it is on all four on free09's flat, its tires throwing dust.
    let switched = tape("on.tape", 130, &["30 atv"]);
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .args(["--headless", "--track", "free09", "--frames", "130", "--steady-clock", "--mods", "mods"])
        .args(["--play", switched.to_str().expect("a path"), "--atv", "atv_Race250P"])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));

    // Its camera: FC_ATVDefault.pb's presets, starting on the first.
    let names = format!("atv: mxvatv_atv's camera, 8 presets in Cam\\FC_ATVDefault.pb: {}", FC_ATV.join(", "));
    assert_eq!(line(&said, "atv: mxvatv_atv's camera"), names);
    assert_eq!(line(&said, "atv: its camera starts on"), "atv: its camera starts on Race Camera 1, 8 presets in Cam\\FC_ATVDefault.pb");

    // Its engine: vehicle.aud's sounds read through the bike's reader.
    let engine = line(&said, "atv: mxvatv_atv's engine, ");
    let n: usize = engine.split_whitespace().nth(3).and_then(|w| w.parse().ok()).unwrap_or_else(|| panic!("no count in `{engine}`"));
    assert!(n > 0 && engine.contains(" sounds in Aud/vehicle.aud"), "{engine}");

    // Its particles: the pack's list on its own zones, and its tires' dust in the air at the end.
    assert_eq!(line(&said, "atv: particles: zones off"), "atv: particles: zones off Part/atvparticlezones.dat");
    let list = line(&said, "atv: particles: 18 entries off mxvatv_atv's Part/atvparticles.dat");
    assert!(list.contains("2 tire tracks not laid"), "{list}");
    let quad = line(&said, "atv: atv_Race250P ");
    assert!(quad.contains("on all four"), "{quad}");
    let air = said.lines().filter(|l| l.starts_with("atv: particles: ") && l.ends_with(" in the air")).last().unwrap_or_else(|| panic!("no particles line:\n{said}"));
    assert!(air.contains("Tire Dust"), "{air}");
}
