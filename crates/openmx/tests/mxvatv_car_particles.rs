//! MD176: MX vs. ATV's cars' dirt and dust. With `--mxvatv-car` and openmx-mods' `mxvatv_cars`
//! pack in this checkout's mods folder, the shell rides free09 headless on a hand-written tape
//! (switch to the car, the gas held), and says what the car's particles read and what they threw:
//! its kind's own list off the pack (`buggyparticles.dat`, `SRparticles.dat`, …, as SLUS-21104's
//! loader picks it), zoned by the pack's `mxparticlezones.dat`, every entry on a node the chassis
//! has, and, on the gas over free09's dirt, its tires' dirt spray in the air. Picked and never
//! switched to, the car throws nothing. Skips without a disc or the pack.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md176-mxvatv-car-particles");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// The tick the tape switches to the car.
const IN: usize = 30;

/// A tape of `frames` ticks with Cross (the car's gas) held throughout, and with `switch` the
/// switch to the car at [`IN`].
fn tape(name: &str, frames: usize, switch: bool) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the car\n");
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x0079ffbf7f7f7f7f000000000000ff{} -\n", "00".repeat(17));
    }
    if switch {
        text += &format!("switch {IN} mxvatv_car\n");
    }
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The shell on free09 with `car` picked, headless, for `frames`: what it said. `None` with no disc.
fn ride(car: &str, frames: usize, switch: bool) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .args(["--headless", "--track", "free09", "--mods", "mods", "--size", "320x224", "--overlay", "none", "--steady-clock"])
        .args(["--frames", &frames.to_string(), "--play"])
        .arg(tape(&format!("{car}-{switch}.tape"), frames + 5, switch))
        .args(["--mxvatv-car", car])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// The line starting `start`, or a panic with everything said.
fn line<'a>(said: &'a str, start: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(start)).unwrap_or_else(|| panic!("no `{start}` line:\n{said}"))
}

/// Each `<name> <count>` of a `particles: … in the air` line.
fn counts(air: &str) -> Vec<(String, usize)> {
    let body = air.trim_start_matches("mxvatv car: particles: ").trim_end_matches(" in the air");
    if body == "none" {
        return Vec::new();
    }
    body.split(", ")
        .map(|e| {
            let (name, n) = e.rsplit_once(' ').unwrap_or_else(|| panic!("not a count: {e}"));
            (name.to_string(), n.parse().unwrap_or_else(|_| panic!("not a count: {e}")))
        })
        .collect()
}

#[test]
fn a_car_on_the_gas_over_dirt_throws_spray() {
    if !root().join("mods/machines/mxvatv_cars/mod.json").is_file() {
        eprintln!("SKIPPED: no mods/machines/mxvatv_cars (openmx-mods' import-atv cars makes it)");
        return;
    }
    let cars = [("buggy", "buggyparticles.dat"), ("monstertruck", "monsterparticles.dat"), ("baja", "bajaparticles.dat"), ("golfcart", "golfparticles.dat"), ("sandrail", "SRparticles.dat")];
    for (car, list) in cars {
        let Some(said) = ride(car, 112, true) else { return };
        assert!(said.lines().any(|l| l == "vehicle: mxvatv_car"), "{car}: never in the car:\n{said}");
        assert_eq!(line(&said, "mxvatv car: particles: zones off"), "mxvatv car: particles: zones off mxvatv_cars's mxparticlezones.dat", "{car}");
        let loaded = said.lines().find(|l| l.starts_with("mxvatv car: particles: ") && l.contains(" entries off ")).unwrap_or_else(|| panic!("{car}: no list line:\n{said}"));
        assert!(loaded.contains(&format!(" entries off mxvatv_cars's {list}; ")) && loaded.ends_with("; 0 not run: "), "{car}: {loaded}");
        assert!(!said.lines().any(|l| l.contains("which the chassis model does not have")), "{car}: an entry off the chassis:\n{said}");

        // Down on the dirt by the end, on the gas: the tires' dirt spray is in the air.
        let report = said.lines().filter(|l| l.starts_with("mxvatv car: ") && l.contains(" ft from where it was taken")).last().unwrap_or_else(|| panic!("{car}: no report:\n{said}"));
        assert!(!report.contains("wrecked,"), "{car}: {report}");
        let air = said.lines().filter(|l| l.starts_with("mxvatv car: particles: ") && l.ends_with(" in the air")).last().unwrap_or_else(|| panic!("{car}: no particles line:\n{said}"));
        let spray: usize = counts(air).iter().filter(|(n, _)| n.contains("Tire Dirt Spray")).map(|(_, n)| n).sum();
        assert!(spray > 0, "{car}: no dirt spray: {air}\n{report}");
    }

    // Picked and never switched to: nothing thrown, nothing said of it.
    let Some(said) = ride("buggy", 112, false) else { return };
    assert!(!said.lines().any(|l| l.starts_with("mxvatv car: particles: ") && l.ends_with(" in the air")), "the bike's ride said car particles:\n{said}");
}
