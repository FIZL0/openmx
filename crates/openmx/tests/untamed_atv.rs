//! MD144: MX vs. ATV Untamed's race quad in the shell, an addition off by default. openmx-mods'
//! `untamed_atv` machine pack (import-atv untamed-atv: SLUS-21701's `res\veh\VehicleType_1.res` whole)
//! lists `Veh/atv_Race250P.phy.bpb` and `Veh/atv_Pro250P.phy.bpb` with `"step": "untamed"`;
//! `--atv atv_Race250P@untamed_atv` offers the quad at the pause's Switch To Machine, built off the
//! pack's BPB `.phy`, chassis and collision objects alone and ridden on Untamed's step
//! ([`game_sim::quad::Ride::untamed`]). A bare `atv_Race250P` stays MX vs. ATV Unleashed's
//! (`mxvatv_atv`).
//!
//! This rides the binary headless on hand-written tapes, as mxvatv_atv.rs does. It skips with no disc,
//! and the quad half with no `untamed_atv` pack in this checkout's `mods/machines/`.

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

/// Whether this checkout has pack `id`; says SKIPPED when not.
fn pack(id: &str) -> bool {
    let there = root().join("mods/machines").join(id).join("mod.json").is_file();
    if !there {
        eprintln!("SKIPPED: no mods/machines/{id} (openmx-mods' import-atv makes it)");
    }
    there
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md144-untamed-atv");
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

fn vehicles(said: &str) -> Vec<&str> {
    said.lines().filter_map(|l| l.strip_prefix("vehicle: ")).collect()
}

fn ride_hash(said: &str) -> &str {
    said.lines().find(|l| l.starts_with("ride hash ")).unwrap_or_else(|| panic!("no ride hash:\n{said}"))
}

/// The quad line said as it is left, and how far it went.
fn quad_line(said: &str) -> (&str, f32) {
    let line = said.lines().find(|l| l.starts_with("atv: atv_Race250P ")).unwrap_or_else(|| panic!("no quad line:\n{said}"));
    let gone: f32 = line.split_whitespace().nth(2).and_then(|w| w.parse().ok()).unwrap_or_else(|| panic!("no distance in `{line}`"));
    (line, gone)
}

/// **The Untamed race quad rides in the shell on Untamed's step**; picked and never switched to, the
/// bike's ride is the one with none picked.
#[test]
fn the_untamed_quad_rides_in_the_shell_on_untameds_step() {
    if !pack("untamed_atv") {
        return;
    }
    let switched = tape("on.tape", 300, &["30 atv", "280 bike"]);
    let Some(said) = ride(320, &switched, &["--atv", "atv_Race250P@untamed_atv"]) else { return };
    assert!(said.contains("atv: Race 250 P (atv_Race250P) off untamed_atv, on MX vs. ATV Untamed's step (switch from the pause)\n"), "\n{said}");
    assert_eq!(vehicles(&said), ["bike", "atv", "bike"], "\n{said}");
    let (line, gone) = quad_line(&said);
    assert!(gone > 100.0, "the quad went only {gone} ft on the gas: {line}");

    let Some(pro) = ride(320, &switched, &["--atv", "atv_Pro250P@untamed_atv"]) else { return };
    assert_eq!(vehicles(&pro), ["bike", "atv", "bike"], "\n{pro}");
    assert!(pro.lines().any(|l| l.starts_with("atv: atv_Pro250P ")), "\n{pro}");

    let bike = tape("bike.tape", 200, &[]);
    let Some(none) = ride(220, &bike, &[]) else { return };
    let Some(picked) = ride(220, &bike, &["--atv", "atv_Race250P@untamed_atv"]) else { return };
    assert_eq!(ride_hash(&none), ride_hash(&picked));
}

/// **A bare `atv_Race250P` stays MX vs. ATV Unleashed's** with both quad packs in: off `mxvatv_atv` on
/// Unleashed's step, and its ride is not the Untamed quad's.
#[test]
fn a_bare_quad_is_unleashed_s_beside_the_untamed_pack() {
    if !pack("untamed_atv") || !pack("mxvatv_atv") {
        return;
    }
    let switched = tape("both.tape", 300, &["30 atv", "280 bike"]);
    let Some(bare) = ride(320, &switched, &["--atv", "atv_Race250P"]) else { return };
    assert!(bare.contains("atv: Race 250 P (atv_Race250P) off mxvatv_atv (switch from the pause)\n"), "\n{bare}");
    let Some(untamed) = ride(320, &switched, &["--atv", "atv_Race250P@untamed_atv"]) else { return };
    assert_ne!(quad_line(&bare).0, quad_line(&untamed).0, "the Untamed quad rode as Unleashed's");
}

/// **MD144b: the Untamed quad is drawn**: its compiled build (`ATV_R_250_KTM.sub.bpb`) read as the
/// `.phy.bpb` is, its parts and materials (`.psm.bpb`) and Untamed's ATV rider set, the pack's own.
/// Under `--trace-picture` each drawn frame names its chassis and parts and the rider sat on it; a
/// shot is written for the eye.
#[test]
fn the_untamed_quad_is_drawn_with_untameds_rider() {
    if !pack("untamed_atv") {
        return;
    }
    let switched = tape("drawn.tape", 160, &["30 atv"]);
    let shot = scratch("drawn.png");
    let shot_flags = ["--trace-picture", "--size", "640x448", "--overlay", "none", "--shot-frame", "148", "--shot"];
    let mut more: Vec<&str> = shot_flags.to_vec();
    more.extend([shot.to_str().expect("a path"), "--atv", "atv_Race250P@untamed_atv"]);
    let Some(said) = ride(150, &switched, &more) else { return };
    let drawn = said.lines().find(|l| l.starts_with("atv: drawn as ")).unwrap_or_else(|| panic!("not drawn:\n{said}"));
    assert!(drawn.starts_with("atv: drawn as Veh/Anim/ATV_R_250_KTM.sub.bpb ("), "{drawn}");
    let frames: Vec<&str> = said.lines().filter(|l| l.starts_with("picture atv: seat 0 ")).collect();
    assert!(frames.len() >= 100, "the quad was hardly drawn ({} frames):\n{said}", frames.len());
    assert!(frames.iter().all(|l| l.contains(" quad:250r_atv_chasis at ") && !l.contains(" parts 0 ")), "{}", frames[0]);
    let riders = said.lines().filter(|l| l.starts_with("pilot: seat 0 ") && !l.ends_with(" of 0")).count();
    assert!(riders >= frames.len(), "frames without a rider:\n{said}");
    assert!(std::fs::metadata(&shot).is_ok_and(|m| m.len() > 0), "no shot at {}", shot.display());
}

fn line<'a>(said: &'a str, start: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(start)).unwrap_or_else(|| panic!("no `{start}` line:\n{said}"))
}

/// **MD206: the Untamed quad rides its pack's own camera, engine and particles**, as MD136's
/// Unleashed quad does: Untamed keeps them compiled (`Cam\FC_ATVDefault.pb.bpb` off the PSR,
/// `Aud\vehicle.aud.bpb` and `Part\*.bpb` in its archive, the `.VAG`s off `MX08AUD.PSR`), and the
/// pack's reads take a file's `.bpb` twin as its text. The follow camera starts on the pack's first
/// preset, the engine reads every sound the `.aud` names, and its tires throw the pack's dust.
#[test]
fn the_untamed_quad_says_its_cameras_engine_and_particles_are_the_packs() {
    if !pack("untamed_atv") {
        return;
    }
    let switched = tape("own.tape", 130, &["30 atv"]);
    let Some(said) = ride(130, &switched, &["--atv", "atv_Race250P@untamed_atv"]) else { return };
    // Its camera: FC_ATVDefault.pb.bpb is in Untamed's slow/fast layout, which the follow camera does
    // not read, so FC_ATV.ini(.bpb)'s presets, in SLUS-21104's layout, starting on the first.
    assert!(said.contains("atv: untamed_atv's Cam\\FC_ATVDefault.pb is in MX vs. ATV Untamed's slow/fast layout, "), "\n{said}");
    let camera = line(&said, "atv: untamed_atv's camera, ");
    assert!(camera.starts_with("atv: untamed_atv's camera, 12 presets in Cam\\FC_ATV.ini: RaceCam1, RaceCam2, "), "{camera}");
    assert_eq!(line(&said, "atv: its camera starts on"), "atv: its camera starts on RaceCam1, 12 presets in Cam\\FC_ATV.ini");

    let engine = line(&said, "atv: untamed_atv's engine, ");
    assert!(!said.contains("atv: untamed_atv's engine: "), "a sound not read:\n{said}");
    let n: usize = engine.split_whitespace().nth(3).and_then(|w| w.parse().ok()).unwrap_or_else(|| panic!("no count in `{engine}`"));
    assert!(n > 0 && engine.ends_with(" sounds in Aud/vehicle.aud.bpb, 4 wreck"), "{engine}");

    assert_eq!(line(&said, "atv: particles: zones off"), "atv: particles: zones off Part/atvparticlezones.dat.bpb");
    let list = line(&said, "atv: particles: 18 entries off ");
    assert!(list.contains(" entries off untamed_atv's Part/atvparticles.dat.bpb;"), "{list}");
    let air = said.lines().filter(|l| l.starts_with("atv: particles: ") && l.ends_with(" in the air")).last().unwrap_or_else(|| panic!("no particles line:\n{said}"));
    assert!(air.contains("Tire Dust"), "{air}");
}
