//! MD82a: which camera file each MX vs. ATV Unleashed machine follows with, and what SLUS-21104 reads
//! out of it, against the follow camera's own records in seven RAM images.
//!
//! The images are MD39a's five car states (`context/mxvatv-ps2/md39a/<car>.ee.bin`), MD37a's ATV at
//! a spawn and in the air (`md37a/atv-r-spawn`, `atv-s-air`) and the bike in free ride
//! (`state04.ee.bin`). In each, the follow camera is the object whose vtable word ([`FOLLOW_VTABLE`]) sits
//! at +4; it holds the file it loaded at +0x128, the preset count at +0x118, the current preset at
//! +0x110 and the records, 0xa0 bytes each, behind the pointer at +0x124. The files are
//! `Cam\CameraPresets.res`'s, extracted off the disc into `context/mxvatv-ps2/md82/cam/`
//! (`research/journal/2026-11-03-md82-mxvatv-camera-presets`). Each case skips without its files.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use std::path::PathBuf;

use chase::ModMachine;
use game_sim::camera::{Aspect, Preset, Viewport};
use game_sim::phy::Ini;

/// The follow camera's vtable word, at the object's +4.
const FOLLOW_VTABLE: u32 = 0x003c_c6f0;
/// One preset record's size.
const RECORD: usize = 0xa0;

/// Each RAM image and the machine in it.
const IMAGES: [(&str, ModMachine); 8] = [
    ("md39a/buggy", ModMachine::Buggy),
    ("md39a/baja", ModMachine::Baja),
    ("md39a/sandrail", ModMachine::Sandrail),
    ("md39a/golf", ModMachine::GolfCart),
    ("md39a/truck", ModMachine::MonsterTruck),
    ("md37a/atv-r-spawn", ModMachine::Atv),
    ("md37a/atv-s-air", ModMachine::Atv),
    ("state04", ModMachine::Motocross),
];

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

/// A camera file by its in-archive name (`Cam\FC_ATVDefault.pb`), `None` without it.
fn camera_file(name: &str) -> Option<Ini> {
    let base = name.rsplit('\\').next()?;
    let bytes = std::fs::read(context().join("mxvatv-ps2/md82/cam").join(base)).ok()?;
    Some(Ini::parse_bytes(&bytes))
}

fn u32_at(ram: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(ram[at..at + 4].try_into().unwrap())
}

fn cstr(ram: &[u8], at: usize, len: usize) -> String {
    let b = &ram[at..at + len];
    let end = b.iter().position(|&c| c == 0).unwrap_or(len);
    String::from_utf8_lossy(&b[..end]).into_owned()
}

/// The follow camera object in `ram`: the first word-aligned vtable word, less 4.
fn follow_camera(ram: &[u8]) -> Option<usize> {
    let want = FOLLOW_VTABLE.to_le_bytes();
    (4..ram.len() - 4).step_by(4).find(|&i| ram[i..i + 4] == want).map(|i| i - 4)
}

/// Every field of `p` against the record at `r`, bit for bit. Returns the fields that differ.
fn differences(ram: &[u8], r: usize, p: &Preset) -> Vec<String> {
    let mut bad = Vec::new();
    let mut word = |name: &str, off: usize, want: u32| {
        let got = u32_at(ram, r + off);
        if got != want {
            bad.push(format!("{name} (+{off:#x}): ram {got:#010x}, port {want:#010x}"));
        }
    };
    for (name, at, v) in [("LookFilters", 0x00, p.look_filters), ("PosFilters", 0x10, p.pos_filters), ("OffsetFromBodyPoint", 0x20, p.offset_from_body_point)] {
        for (k, bits) in v.to_bits().iter().take(3).enumerate() {
            word(&format!("{name}[{k}]"), at + 4 * k, *bits);
        }
    }
    let flag = |b: bool| b as u32;
    for (name, off, bits) in [
        ("AngleY", 0x50, p.angle_y.to_bits()),
        ("AngleX", 0x54, p.angle_x.to_bits()),
        ("FollowingDistance", 0x58, p.following_distance.to_bits()),
        ("FOV", 0x5c, p.fov.to_bits()),
        ("ReverseAngleXOffset", 0x60, p.reverse_angle_x_offset.to_bits()),
        ("ReverseFollowingOffset", 0x64, p.reverse_following_offset.to_bits()),
        ("ReverseLookAheadOffset", 0x68, p.reverse_look_ahead_offset.to_bits()),
        ("ForwardReverseSwitchSpeed", 0x6c, p.forward_reverse_switch_speed.to_bits()),
        ("CycleThruThisPreset", 0x70, flag(p.cycle_thru_this_preset)),
        ("FilterThisPreset", 0x74, flag(p.filter_this_preset)),
        ("LookAheadDistance", 0x78, p.look_ahead_distance.to_bits()),
        ("IsLookBackCam", 0x7c, flag(p.is_look_back_cam)),
        ("Swing", 0x80, p.camera_swing.to_bits()),
        ("LookSwing", 0x84, p.look_swing.to_bits()),
        ("MinFollowingDistance", 0x88, p.min_following_distance.to_bits()),
        ("MaxFollowingDistance", 0x8c, p.max_following_distance.to_bits()),
        ("MinFOV", 0x90, p.min_fov.to_bits()),
        ("MaxFOV", 0x94, p.max_fov.to_bits()),
        ("LowSpeedAngleXVelCutoff", 0x98, p.low_speed_angle_x_vel_cutoff.to_bits()),
        ("LowSpeedAngleXScaleFactor", 0x9c, p.low_speed_angle_x_scale_factor.to_bits()),
    ] {
        word(name, off, bits);
    }
    let name = cstr(ram, r + 0x30, 0x20);
    if name != p.name {
        bad.push(format!("Name (+0x30): ram {name:?}, port {:?}", p.name));
    }
    bad
}

#[test]
fn each_machine_follows_with_its_own_file_read_as_slus_21104_reads_it() {
    let mut checked = 0;
    for (image, machine) in IMAGES {
        let path = context().join("mxvatv-ps2").join(format!("{image}.ee.bin"));
        let Ok(ram) = std::fs::read(&path) else {
            eprintln!("skip {image}: no {}", path.display());
            continue;
        };
        let file = chase::mod_presets(machine);
        let Some(ini) = camera_file(file) else {
            eprintln!("skip {image}: no {file} in context/mxvatv-ps2/md82/cam");
            continue;
        };
        let o = follow_camera(&ram).unwrap_or_else(|| panic!("{image}: no follow camera"));
        assert_eq!(cstr(&ram, o + 0x128, 0x40), file, "{image}: the file the camera loaded");
        let count = u32_at(&ram, o + 0x118) as usize;
        let records = u32_at(&ram, o + 0x124) as usize;
        // Every image is one player at 4:3, the game's default screen.
        let presets = Preset::read_layout(&ini, Aspect::Standard, Viewport::Full);
        assert_eq!(count, presets.len(), "{image}: preset count");
        assert_eq!(u32_at(&ram, o + 0x110), 0, "{image}: the current preset");
        for (k, p) in presets.iter().enumerate() {
            let bad = differences(&ram, records + k * RECORD, p);
            assert!(bad.is_empty(), "{image} {file} preset {k}:\n  {}", bad.join("\n  "));
        }
        // The control: the files do say CameraSwing, and not zero, so the zero above is the key.
        let first = ini.section("4x3/1p/1").float("CameraSwing").expect("the file's CameraSwing");
        assert_ne!(first.to_bits(), 0, "{image}: {file}'s CameraSwing");
        eprintln!("{image}: {file}, {count} presets, every field as in RAM");
        checked += 1;
    }
    if checked == 0 {
        eprintln!("skip: no RAM image with its camera file");
    }
}

#[test]
fn a_layout_falls_back_to_four_by_three_and_the_race_file_swaps_its_prefix() {
    assert_eq!(chase::mod_race_presets(ModMachine::Buggy), "Cam\\RC_BuggyDefault.pb");
    let Some(ini) = camera_file(chase::mod_presets(ModMachine::Buggy)) else {
        eprintln!("skip: no FC_BuggyDefault.pb in context/mxvatv-ps2/md82/cam");
        return;
    };
    let standard = Preset::read_layout(&ini, Aspect::Standard, Viewport::Full);
    assert!(!standard.is_empty());
    // No file has 1x1: the loader retries at 4:3.
    assert_eq!(Preset::read_layout(&ini, Aspect::Square, Viewport::Full), standard);
    // 16x9 has its own one-player set, which stops at the first missing number.
    let wide = Preset::read_layout(&ini, Aspect::Wide, Viewport::Full);
    let wide_sections = (1..).take_while(|n| ini.names().iter().any(|s| s.eq_ignore_ascii_case(&format!("16x9/1p/{n}")))).count();
    assert_eq!(wide.len(), wide_sections);
    assert!(wide_sections > 0);
    // No 4p anywhere, and no fallback for a viewport.
    assert!(Preset::read_layout(&ini, Aspect::Standard, Viewport::Quad).is_empty());
    assert!(Preset::read_layout(&ini, Aspect::Wide, Viewport::Quad).is_empty());
    // A preset's name defaults to its section's.
    assert!(standard.iter().all(|p| !p.name.is_empty()));
}
