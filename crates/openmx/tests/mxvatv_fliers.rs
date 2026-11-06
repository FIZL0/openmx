//! MD80e: MX vs. ATV's biplane and Kiowa in the shell, off by default. With `--fliers mxvatv` (since
//! P315 the Garage's Fliers tab picks them per seat) and openmx-mods' `mxvatv_fliers` machine pack in the mods folder, the plane and the
//! helicopter the garage and Switch To Machine give are built off the pack's `biplane.phy` and
//! `kiowa.phy` for the picked skin's model, drawn in its models, and flown on SLUS-21104's steps
//! (MD80b, MD80c). Without the switch the ride is MX Unleashed's: no pack is read, and the
//! helicopter's climb is the one MX Unleashed's step flies.
//!
//! This rides the binary headless on hand-written tapes, as kiowa_switch.rs does. It skips with no
//! disc, and the MX vs. ATV half with no pack in this checkout's `mods/machines/`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Ride free09 headless for `frames` ticks on the steady clock playing `tape`, with `more` flags
/// and this checkout's mods folder, or `None` with no disc to ride.
fn ride(frames: u32, tape: &Path, more: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .args(["--headless", "--track", "free09", "--frames", &frames.to_string(), "--steady-clock", "--mods", "mods"])
        .args(["--play", tape.to_str().expect("a path")])
        .args(more)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// Whether this checkout has the fliers pack; says SKIPPED when not.
fn pack() -> bool {
    let there = root().join("mods/machines/mxvatv_fliers/mod.json").is_file();
    if !there {
        eprintln!("SKIPPED: no mods/machines/mxvatv_fliers (openmx-mods' import-atv fliers makes it)");
    }
    there
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md80e-mxvatv-fliers");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// `frames` ticks of Cross held at full pressure (the plane's throttle, the helicopter's climb),
/// switching into `machine` at `up` and back to the bike at `down`.
fn tape(name: &str, frames: usize, machine: &str, up: usize, down: usize) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape Cross held\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text += &format!("switch {up} {machine}\nswitch {down} bike\n");
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The `kiowa:` line said as the helicopter is left.
fn left(said: &str) -> &str {
    said.lines().find(|l| l.starts_with("kiowa: ") && l.contains("from where it was taken")).unwrap_or_else(|| panic!("no kiowa line:\n{said}"))
}

/// How far the helicopter went from where it was taken and how high over it it was, off [`left`].
fn gone_and_over(line: &str) -> (f32, f32) {
    let words: Vec<&str> = line.split_whitespace().collect();
    let number = |i: usize| words[i].trim_end_matches(',').parse::<f32>().unwrap_or_else(|_| panic!("no number at {i} in `{line}`"));
    (number(1), number(8))
}

/// The `plane hash` line the shell says at the ride's end, of the plane's flight (P102).
fn plane_hash(said: &str) -> &str {
    said.lines().find(|l| l.starts_with("plane hash ")).unwrap_or_else(|| panic!("no plane hash:\n{said}"))
}

#[test]
fn off_by_default_the_fliers_are_mx_unleasheds() {
    let heli = tape("heli-off.tape", 240, "helicopter", 30, 200);
    let Some(said) = ride(260, &heli, &[]) else { return };
    let Some(asked) = ride(260, &heli, &["--fliers", "original"]) else { return };
    for s in [&said, &asked] {
        assert!(!s.contains("MX vs. ATV's") && !s.contains("mxvatv_fliers's"), "a ride without the switch read the fliers pack:\n{s}");
        assert!(!s.contains("fliers:"), "\n{s}");
    }
    // The same helicopter flown the same way, unset or asked for: MX Unleashed's step's climb.
    assert_eq!(left(&said), left(&asked));
    let (gone, over) = gone_and_over(left(&said));
    assert!(over > 100.0 && gone < 10.0, "{}", left(&said));

    // And the same plane: its flight hashes alike unset and asked for.
    let plane = tape("plane-off.tape", 400, "biplane", 30, 380);
    let Some(said) = ride(420, &plane, &[]) else { return };
    let Some(asked) = ride(420, &plane, &["--fliers", "original"]) else { return };
    assert!(!said.contains("MX vs. ATV's") && !asked.contains("MX vs. ATV's"), "\n{said}\n{asked}");
    assert_eq!(plane_hash(&said), plane_hash(&asked));
}

#[test]
fn on_the_kiowa_is_mx_vs_atvs_on_both_models() {
    if !pack() {
        return;
    }
    let heli = tape("heli-on.tape", 240, "helicopter", 30, 200);
    let Some(mxu) = ride(260, &heli, &[]) else { return };
    let mut lines = Vec::new();
    for (skin, model) in [("2", "kiowa2"), ("3", "kiowa3")] {
        let Some(said) = ride(260, &heli, &["--fliers", "mxvatv", "--flier-skin", skin]) else { return };
        assert!(said.contains(&format!("kiowa: mxvatv_fliers's, on its steps, drawn in {model}\n")), "skin {skin}:\n{said}");
        let vehicles: Vec<&str> = said.lines().filter(|l| l.starts_with("vehicle: ")).collect();
        assert_eq!(vehicles, ["vehicle: bike", "vehicle: helicopter", "vehicle: bike"], "\n{said}");
        // Cross climbs it straight up, as it does MX Unleashed's, but on SLUS-21104's controller.
        let (gone, over) = gone_and_over(left(&said));
        assert!(over > 100.0, "skin {skin} climbed only {over} ft on Cross:\n{said}");
        assert!(gone < 10.0, "skin {skin} drifted {gone} ft with the stick centred:\n{said}");
        assert_ne!(left(&said), left(&mxu), "skin {skin} flew MX Unleashed's climb");
        lines.push(left(&said).to_string());
    }
    println!("{}\n{}\nMX Unleashed's: {}", lines[0], lines[1], left(&mxu));
}

#[test]
fn on_the_biplane_is_mx_vs_atvs_on_both_models() {
    if !pack() {
        return;
    }
    let plane = tape("plane-on.tape", 400, "biplane", 30, 380);
    let Some(mxu) = ride(420, &plane, &[]) else { return };
    let mut hashes = Vec::new();
    for (skin, model) in [("1", "biplane"), ("4", "biplane4")] {
        let Some(said) = ride(420, &plane, &["--fliers", "mxvatv", "--flier-skin", skin]) else { return };
        assert!(said.contains("biplane: flies on MX vs. ATV's biplane.phy, on its steps"), "skin {skin}:\n{said}");
        assert!(said.contains(&format!("biplane: mxvatv_fliers's, drawn in {model}\n")), "skin {skin}:\n{said}");
        let vehicles: Vec<&str> = said.lines().filter(|l| l.starts_with("vehicle: ")).collect();
        assert_eq!(vehicles, ["vehicle: bike", "vehicle: biplane", "vehicle: bike"], "\n{said}");
        hashes.push(plane_hash(&said).to_string());
    }
    // Model one and model two are different planes on the same tape, and neither flies as MX
    // Unleashed's does.
    assert_ne!(hashes[0], hashes[1], "model one and two flew alike");
    assert!(hashes.iter().all(|h| h != plane_hash(&mxu)), "{hashes:?} against MX Unleashed's {}", plane_hash(&mxu));
}
