//! MD10: `--shot-pose x,y,z,yaw,pitch,fov` shoots from the pose, not from the ride camera. The
//! shell rides free06 and is shot from beside its start line, looking at the start: on the first
//! frame the bike stands there, in the middle of the shot; three seconds of full throttle later the
//! same pose shows the same hills, with the bike gone from the middle — the camera stayed where the
//! pose put it while the ride went on.
//!
//! The arithmetic of the pose (its axes, its fov, a ground marker centred at any size) is
//! `shot_pose`'s own tests, inside the crate.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A PNG the shell wrote, back to its RGB pixels: its rows are filter 0 in zlib stored blocks.
fn read_png(path: &Path) -> (usize, usize, Vec<u8>) {
    let png = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
    let (mut at, mut size, mut idat) = (8, (0, 0), Vec::new());
    while at + 8 <= png.len() {
        let (len, tag) = (be(at), &png[at + 4..at + 8]);
        let data = &png[at + 8..at + 8 + len];
        match tag {
            b"IHDR" => size = (be(at + 8), be(at + 12)),
            b"IDAT" => idat.extend_from_slice(data),
            _ => {}
        }
        at += 12 + len;
    }
    let mut raw = Vec::new();
    let mut i = 2;
    loop {
        let last = idat[i] & 1 != 0;
        let n = u16::from_le_bytes([idat[i + 1], idat[i + 2]]) as usize;
        raw.extend_from_slice(&idat[i + 5..i + 5 + n]);
        i += 5 + n;
        if last {
            break;
        }
    }
    let (w, h) = size;
    let rgb = (0..h).flat_map(|y| raw[y * (1 + 3 * w) + 1..(y + 1) * (1 + 3 * w)].to_vec()).collect();
    (w, h, rgb)
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md10-shot-pose");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Full throttle and nothing else: Cross down, at full pressure, under the original's controls.
fn throttle_tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape full throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text
}

/// The shell on free06, headless at 640x448 with no overlay, with these arguments: what it said.
fn ride(args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--size", "640x448", "--overlay", "none", "--controls", "original"])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    said
}

/// The start line and the way it faces, in degrees, off the shell's `start:` line.
fn start(said: &str) -> ([f32; 3], f32) {
    let l = said.lines().find(|l| l.starts_with("start: ")).unwrap_or_else(|| panic!("no start line:\n{said}"));
    let n: Vec<f32> = l.split([' ', ',']).filter_map(|w| w.parse().ok()).collect();
    ([n[0], n[1], n[2]], n[3])
}

/// The fraction of a box's pixels the two frames have apart.
fn apart(a: &[u8], b: &[u8], (x0, y0, x1, y1): (usize, usize, usize, usize)) -> f32 {
    let n = (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| 3 * (y * 640 + x)))
        .filter(|&i| (0..3).any(|k| a[i + k].abs_diff(b[i + k]) > 24))
        .count();
    n as f32 / ((x1 - x0) * (y1 - y0)) as f32
}

#[test]
fn a_shot_from_a_pose_looks_from_that_point() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let ([x, y, z], facing) = start(&ride(&["--frames", "1"]));
    // Twenty feet to the start's side and six up, looking at the bike's middle: the start line is
    // the dirt under its wheels, and the seat is some three feet over it.
    let (s, c) = facing.to_radians().sin_cos();
    let eye = [x + 20.0 * c, y + 6.0, z - 20.0 * s];
    let (dx, dy, dz) = (x - eye[0], y + 3.0 - eye[1], z - eye[2]);
    let yaw = dx.atan2(dz).to_degrees();
    let pitch = dy.atan2((dx * dx + dz * dz).sqrt()).to_degrees();
    let pose = format!("{},{},{},{yaw},{pitch},85", eye[0], eye[1], eye[2]);

    let (first, later, unposed) = (scratch("first.png"), scratch("later.png"), scratch("unposed.png"));
    let tape = scratch("throttle.tape");
    std::fs::write(&tape, throttle_tape(120)).expect("the tape is written");
    for path in [&first, &later, &unposed] {
        let _ = std::fs::remove_file(path);
    }
    let shot = |path: &PathBuf, extra: &[&str]| {
        let mut args = vec!["--shot", path.to_str().unwrap()];
        args.extend_from_slice(extra);
        ride(&args);
        let (w, h, rgb) = read_png(path);
        assert_eq!((w, h), (640, 448));
        rgb
    };
    let first = shot(&first, &["--shot-pose", &pose, "--frames", "2"]);
    let tape = tape.to_str().unwrap();
    let later = shot(&later, &["--shot-pose", &pose, "--play", tape, "--shot-frame", "90", "--frames", "95"]);
    let unposed = shot(&unposed, &["--frames", "2"]);

    // The middle: the bike on the first frame, gone three seconds on.
    let middle = (290, 194, 350, 254);
    let gone = apart(&first, &later, middle);
    assert!(gone > 0.3, "the bike was not in the middle of the pose's first frame, or did not leave it: {gone}");
    // The top corners, the hills and the sky behind the start: the same from the same pose.
    for corner in [(0, 0, 120, 100), (520, 0, 640, 100)] {
        let moved = apart(&first, &later, corner);
        assert!(moved < 0.05, "the pose's camera moved with the ride: {corner:?} is {moved} apart");
    }
    // And the pose is not the ride camera's view of the same frame.
    let whole = apart(&first, &unposed, (0, 0, 640, 448));
    assert!(whole > 0.3, "the shot from the pose is the ride camera's: only {whole} apart");
}
