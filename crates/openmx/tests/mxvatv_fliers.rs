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

/// Ride free09 headless for two players on `garage`, a `garage.txt` beside a link to the disc in a
/// scratch folder of its own (the shell keeps its files beside the disc it was given), or `None`
/// with no disc to ride.
fn ride_two(name: &str, garage: &str) -> Option<String> {
    let iso = match game_render::disc::Disc::configured_path() {
        Ok(iso) => iso,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let dir = scratch(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    let link = dir.join("disc.iso");
    std::os::unix::fs::symlink(std::path::absolute(&iso).expect("a path"), &link).expect("the disc is linked");
    std::fs::write(dir.join("garage.txt"), garage).expect("the garage is written");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .arg(&link)
        .args(["--headless", "--track", "free09", "--frames", "10", "--steady-clock", "--mods", "mods", "--players", "2", "--garage"])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

#[test]
fn p1_and_p2_fly_their_own_picks() {
    // P315b: each seat's garage picks build its own plane and helicopter, pack or disc, model one or two.
    if !pack() {
        return;
    }
    let garage = "seat1.flier = biplane mxvatv_fliers biplane3\nseat1.flier = kiowa original kiowa2\n\
                  seat2.flier = biplane mxvatv_fliers biplane\nseat2.flier = kiowa mxvatv_fliers kiowa3\n";
    let Some(said) = ride_two("p315b-own", garage) else { return };
    for line in [
        "biplane: P1 mxvatv_fliers's, drawn in biplane3",
        "biplane: P2 mxvatv_fliers's, drawn in biplane",
        "biplane: P2 flies on MX vs. ATV's biplane.phy as model one, on its steps",
        "kiowa: P1 MX Unleashed's, drawn in kiowa2",
        "kiowa: P2 mxvatv_fliers's, on its steps, drawn in kiowa3",
    ] {
        assert!(said.lines().any(|l| l == line), "no `{line}`:\n{said}");
    }
    let seat_lines: Vec<&str> = said.lines().filter(|l| l.starts_with("fliers: P")).collect();
    assert!(seat_lines.iter().any(|l| l.starts_with("fliers: P1 biplane ") && l.ends_with("(biplane3), from mxvatv_fliers, model 2")), "{seat_lines:?}");
    assert!(seat_lines.iter().any(|l| l.starts_with("fliers: P2 biplane ") && l.ends_with("(biplane), from mxvatv_fliers, model 1")), "{seat_lines:?}");
    assert!(seat_lines.iter().any(|l| l.starts_with("fliers: P2 kiowa ") && l.ends_with("(kiowa3), from mxvatv_fliers, model 2")), "{seat_lines:?}");
    assert!(!seat_lines.iter().any(|l| l.starts_with("fliers: P1 kiowa ")), "P1's helicopter is MX Unleashed's: {seat_lines:?}");

    // Both on one skin: the second wears the other skin of the same model, as the original's second
    // player's machine wears the alternate livery.
    let same = "seat1.flier = biplane mxvatv_fliers biplane4\nseat2.flier = biplane mxvatv_fliers biplane4\n";
    let Some(said) = ride_two("p315b-same", same) else { return };
    for line in ["biplane: P1 mxvatv_fliers's, drawn in biplane4", "biplane: P2 mxvatv_fliers's, drawn in biplane3"] {
        assert!(said.lines().any(|l| l == line), "no `{line}`:\n{said}");
    }
}

/// `frames` ticks of Cross held, as [`tape`]'s, with its own `switches` (`(frame, word)`).
fn switching_tape(name: &str, frames: usize, switches: &[(usize, &str)]) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape Cross held\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    for (at, word) in switches {
        text += &format!("switch {at} {word}\n");
    }
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

#[test]
fn the_switch_flies_the_skin_picked_mid_ride() {
    // P315g: the pause's Switch To Machine picks the flier's skin and the ride rebuilds it there; a
    // tape keeps the skin with each switch (`biplane@pack/skin`). A ride loaded on MX Unleashed's
    // planes switches into the pack's model two, out, into its other skin, out, and into the disc's
    // second livery, and the ride lines name each skin flown.
    if !pack() {
        return;
    }
    let steps = switching_tape(
        "p315g-steps.tape",
        400,
        &[
            (30, "biplane@mxvatv_fliers/biplane3"),
            (110, "bike"),
            (140, "biplane@mxvatv_fliers/biplane"),
            (220, "bike"),
            (250, "biplane@original/biplane2"),
            (330, "bike"),
            (350, "helicopter@mxvatv_fliers/kiowa4"),
            (390, "bike"),
        ],
    );
    let Some(said) = ride(420, &steps, &[]) else { return };
    let skins: Vec<&str> = said.lines().filter(|l| l.starts_with("skin: ")).collect();
    assert_eq!(
        skins,
        [
            "skin: P1 biplane Plane (3) (biplane3) from mxvatv_fliers (the tape's)",
            "skin: P1 biplane Plane (1) (biplane) from mxvatv_fliers (the tape's)",
            "skin: P1 biplane Biplane, Second Livery (biplane2) from MX Unleashed (the tape's)",
            "skin: P1 kiowa Helicopter (4) (kiowa4) from mxvatv_fliers (the tape's)",
        ],
        "\n{said}"
    );
    let vehicles: Vec<&str> = said.lines().filter(|l| l.starts_with("vehicle: ")).collect();
    assert_eq!(vehicles, ["vehicle: bike", "vehicle: biplane", "vehicle: bike", "vehicle: biplane", "vehicle: bike", "vehicle: biplane", "vehicle: bike", "vehicle: helicopter", "vehicle: bike"], "\n{said}");

    // The plane rebuilt mid-ride is the one a ride loaded on that skin flies: the same tape's flight
    // hashes alike, switched into the pack's biplane4 off MX Unleashed's or loaded on it.
    let rebuilt = switching_tape("p315g-rebuilt.tape", 400, &[(30, "biplane@mxvatv_fliers/biplane4"), (380, "bike")]);
    let loaded = tape("p315g-loaded.tape", 400, "biplane", 30, 380);
    let Some(rebuilt) = ride(420, &rebuilt, &[]) else { return };
    let Some(loaded) = ride(420, &loaded, &["--fliers", "mxvatv", "--flier-skin", "3"]) else { return };
    let Some(mxu) = ride(420, &tape("p315g-mxu.tape", 400, "biplane", 30, 380), &[]) else { return };
    assert_eq!(plane_hash(&rebuilt), plane_hash(&loaded), "\n{rebuilt}");
    assert_ne!(plane_hash(&rebuilt), plane_hash(&mxu), "the switch flew MX Unleashed's plane");

    // And a tape written off it keeps the skin with the switch.
    let again = scratch("p315g-again.tape");
    let Some(_) = ride(420, &steps, &["--record", again.to_str().expect("a path")]) else { return };
    let text = std::fs::read_to_string(&again).expect("the tape was written");
    let words: Vec<&str> = text.lines().filter(|l| l.starts_with("switch ")).map(|l| l.split_whitespace().nth(2).unwrap_or("")).collect();
    assert_eq!(words, ["biplane@mxvatv_fliers/biplane3", "bike", "biplane@mxvatv_fliers/biplane", "bike", "biplane@original/biplane2", "bike", "helicopter@mxvatv_fliers/kiowa4", "bike"], "\n{text}");
}

#[test]
fn the_fliers_read_the_packs_cameras_and_particles() {
    // MD108: a pack flier follows on the pack's own FC_ presets and throws its own particle lists,
    // loaded on the switch or switched into mid-ride; switched back to the disc's flier, the camera
    // is MX Unleashed's again. The sounds (its engine loops, wrecks and touchdown) need a sound
    // device and are not heard headless.
    if !pack() {
        return;
    }
    let both = switching_tape("md108-both.tape", 240, &[(20, "biplane"), (90, "bike"), (110, "helicopter"), (180, "bike")]);
    let Some(said) = ride(220, &both, &["--fliers", "mxvatv", "--flier-skin", "3"]) else { return };
    for line in [
        "biplane: particles: mxvatv_fliers's own Part/biplaneparticles.dat",
        "kiowa: particles: mxvatv_fliers's own Part/helicopterparticles.dat",
        "biplane: mxvatv_fliers's camera, 8 presets in Cam/FC_BiplaneDefault.pb",
        "kiowa: mxvatv_fliers's camera, 8 presets in Cam/FC_HeliDefault.pb",
    ] {
        assert!(said.lines().any(|l| l == line), "no `{line}`:\n{said}");
    }

    let steps = switching_tape("md108-steps.tape", 240, &[(20, "biplane@mxvatv_fliers/biplane3"), (90, "bike"), (110, "biplane@original/biplane2"), (180, "bike")]);
    let Some(said) = ride(220, &steps, &[]) else { return };
    let cameras: Vec<&str> = said.lines().filter(|l| l.starts_with("biplane: ") && (l.ends_with(" camera") || l.contains(" camera,") || l.contains("camera starts"))).collect();
    assert_eq!(
        cameras,
        [
            "biplane: mxvatv_fliers's camera, 8 presets in Cam/FC_BiplaneDefault.pb",
            "biplane: its camera starts on Race Camera 1",
            "biplane: MX Unleashed's camera",
            "biplane: its camera starts on Flight Cam 1",
        ],
        "\n{said}"
    );
}

/// kiowa_switch.rs's fast landing (P265): into the helicopter at 30, up on Cross, nose down on the
/// stick, then down on Square with the stick still forward, into the ground going fast: on
/// SLUS-21104's steps by about tick 380, and put back on its skids some 150 ticks after.
fn fast_landing(name: &str) -> PathBuf {
    // (ticks, second buttons byte (active low), Cross's pressure, Square's, the left stick's y)
    let phases = [(30, 0xff, 0, 0, 0x7f), (60, 0xbf, 0xff, 0, 0x7f), (200, 0xff, 0, 0, 0x00), (300, 0x7f, 0, 0xff, 0x00)];
    let frames: usize = phases.iter().map(|p| p.0).sum();
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the helicopter\n");
    for (ticks, buttons, cross, square, stick_y) in phases {
        let report = format!("0079ff{buttons:02x}7f7f7f{stick_y:02x}{}{cross:02x}{square:02x}{}", "00".repeat(6), "00".repeat(16));
        for _ in 0..ticks {
            text += &format!("frame 0x3d08850a 0x{report} -\n");
        }
    }
    text += "switch 30 helicopter\n";
    let path = scratch(name);
    std::fs::write(&path, text).expect("the tape is written");
    path
}

#[test]
fn the_packs_kiowa_throws_the_packs_blades() {
    // MD109: a pack's helicopter builds the blades its wreck throws off the pack's wreckblade.phy and
    // bladebent.psg (the bytes MX Unleashed's disc has too); the pack has no blade collision, so
    // those two are the disc's. On the fast landing it wrecks and throws all four, which are still out
    // at tick 440.
    if !pack() {
        return;
    }
    let tape = fast_landing("md109-thrown-blades.tape");
    for skin in ["1", "3"] {
        let Some(said) = ride(440, &tape, &["--fliers", "mxvatv", "--flier-skin", skin]) else { return };
        let line = said.lines().find(|l| l.starts_with("kiowa: thrown blades: ")).unwrap_or_else(|| panic!("no thrown-blades line:\n{said}"));
        let from: usize = line.split_whitespace().nth(3).and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("no count in `{line}`"));
        assert!(from >= 2, "skin {skin}: wreckblade.phy and bladebent.psg are the pack's: {line}");
        let blades = said.lines().find(|l| l.starts_with("kiowa: blades: ")).unwrap_or_else(|| panic!("skin {skin} threw no blades:\n{said}"));
        assert_eq!(blades.split_whitespace().nth(2), Some("4"), "skin {skin}: {blades}");
        println!("skin {skin}: {line}; {blades}");
    }
}
