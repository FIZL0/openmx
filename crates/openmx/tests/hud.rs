//! V28: the original's HUD shows. V26 drew it, yet a launch showed none: a settings file saved before
//! V26 said `overlay = on`, which V26 read as its debug HUD, and on the GPU a pause menu's patch
//! wrote over the HUD's. These ride the shell itself, as a user would, and look at its first frame:
//! the speedometer's and the radar's boxes are drawn over — the frame differs there from the same
//! ride with no overlay — on the GPU and on the software renderer, and under the pause menu.
//!
//! The settings word is held in tests/menu.rs; these pass `--overlay` so the file beside the disc,
//! whatever it says, cannot decide them.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The speedometer's dial and the radar, in a 640x448 frame's pixels: left, top, right, bottom.
const DIAL: (usize, usize, usize, usize) = (480, 301, 608, 429);
/// The default speedometer's readout with the preload meter beside it (V42).
const READOUT: (usize, usize, usize, usize) = (520, 314, 620, 400);
const RADAR: (usize, usize, usize, usize) = (23, 299, 151, 427);

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

/// The shell's first frame of a ride with these arguments, or `None` with no disc.
fn first_frame(name: &str, args: &[&str]) -> Option<Vec<u8>> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let shot: PathBuf = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("v28-{name}.png"));
    let _ = std::fs::remove_file(&shot);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--size", "640x448", "--frames", "2"])
        .args(args)
        .arg("--shot")
        .arg(&shot)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{name}: the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    let (w, h, rgb) = read_png(&shot);
    assert_eq!((w, h), (640, 448), "{name}");
    Some(rgb)
}

/// How many of a box's pixels the two frames have apart.
fn apart(a: &[u8], b: &[u8], (x0, y0, x1, y1): (usize, usize, usize, usize)) -> usize {
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| 3 * (y * 640 + x)))
        .filter(|&i| (0..3).any(|k| a[i + k].abs_diff(b[i + k]) > 24))
        .count()
}

/// The HUD is over the frame: a good part of the speedometer's box, `speedo`, and of the radar's is
/// drawn on.
fn assert_hud(name: &str, with: &[u8], without: &[u8], speedo: (usize, usize, usize, usize)) {
    let area = |b: (usize, usize, usize, usize)| (b.2 - b.0) * (b.3 - b.1);
    for (part, b) in [("speedometer", speedo), ("radar", RADAR)] {
        let n = apart(with, without, b);
        assert!(n * 10 > area(b), "{name}: only {n} of the {part}'s {} pixels were drawn on", area(b));
    }
}

/// A default launch's first ride frame has the original's HUD over it, on either renderer: the
/// readout and the preload meter, or the dial when asked for (V42).
#[test]
fn a_default_launch_shows_the_originals_hud() {
    for renderer in ["gpu", "software"] {
        let Some(with) = first_frame(renderer, &["--renderer", renderer]) else { return };
        let without = first_frame(&format!("{renderer}-none"), &["--renderer", renderer, "--overlay", "none"]).unwrap();
        assert_hud(renderer, &with, &without, READOUT);
        let dial = first_frame(&format!("{renderer}-analog"), &["--renderer", renderer, "--speedometer", "analog"]).unwrap();
        assert_hud(&format!("{renderer}, analog"), &dial, &without, DIAL);
    }
}

/// Paused, the ride's HUD is still under the menu: on the GPU the menu's patch used to replace it.
/// Since P118 the pause dims the ride and its black band and hint tab cover the bottom of the
/// frame, as the concept's do, so the HUD is looked for in what shows above them, through the dim:
/// a twentieth of each box drawn on, where a replaced HUD draws none.
#[test]
fn the_pause_menu_keeps_the_hud_under_it() {
    for renderer in ["gpu", "software"] {
        let pause = |overlay| ["--renderer", renderer, "--menu", "--screen", "pause", "--overlay", overlay];
        let Some(with) = first_frame(&format!("{renderer}-pause"), &pause("original")) else { return };
        let without = first_frame(&format!("{renderer}-pause-none"), &pause("none")).unwrap();
        for (part, b) in [("speedometer", (READOUT.0, READOUT.1, READOUT.2, 338)), ("radar", (RADAR.0, RADAR.1, RADAR.2, 338))] {
            let (n, area) = (apart(&with, &without, b), (b.2 - b.0) * (b.3 - b.1));
            assert!(n * 20 > area, "{renderer}, paused: only {n} of the {part}'s {area} pixels above the band were drawn on");
        }
    }
}
