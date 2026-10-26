//! P117: with the HUD off (`--overlay none`, Settings' Overlay None) the session markers' bar is not
//! drawn either, on the GPU or on the software renderer, while the markers still work; under the
//! HUD (the ride HUD since P170) its marker menu is drawn as before. The shell rides free06 headless on a hand-written tape — full
//! throttle, Down held to place slot 1, then Up held to ride back to it — and is shot from a fixed
//! pose behind the start line, looking away from the ride, so nothing the ride does reaches the
//! picture but the overlay. The shot is taken while Up is still held, after the teleport has fired:
//! the bar is up then whatever the wall clock did (`MarkerBanner` shows while a direction is held).
//! The same tape with the d-pad left alone is the frame with no markers to show.

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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p117-markers-hud-off");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Ticks of throttle before the placing hold, and between it and the riding-back one.
const RIDE: usize = 30;
/// Ticks each d-pad hold lasts: past `markers::HOLD_MS`'s 200 ms, six 30 Hz ticks.
const HOLD: usize = 10;
/// The tick shot: the Up hold's ninth, after the teleport and with Up still down.
const SHOT: usize = 2 * RIDE + HOLD + 8;

/// Full throttle (Cross at full pressure) at the engine's capped `dt`, with Down and then Up held
/// for [`HOLD`] ticks each when `markers` — or the same ride with the d-pad left alone.
fn tape(markers: bool) -> String {
    let frames = SHOT + 4;
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape p117\n");
    // Byte 2 holds the d-pad (Up 0x10, Down 0x40, active low), bytes 10 and 11 their pressures.
    let report = |dpad: u8, up: u8, down: u8| {
        format!("0079{:02x}bf7f7f7f7f0000{up:02x}{down:02x}0000ff{}", 0xff ^ dpad, "00".repeat(17))
    };
    for t in 0..frames {
        let r = match t {
            _ if !markers => report(0, 0, 0),
            t if (RIDE..RIDE + HOLD).contains(&t) => report(0x40, 0, 0xff),
            t if t >= 2 * RIDE + HOLD => report(0x10, 0xff, 0),
            _ => report(0, 0, 0),
        };
        text += &format!("frame 0x3d08850a 0x{r} -\n");
    }
    text
}

/// The shell on free06, headless at 640x448 under the original's controls: what it said.
fn ride(args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--size", "640x448", "--controls", "original"])
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

/// The marker lines the shell printed.
fn marker_lines(said: &str) -> Vec<&str> {
    said.lines().filter(|l| l.starts_with("marker: ")).collect()
}

/// How many of a box's pixels the two frames have apart, at all.
fn apart(a: &[u8], b: &[u8], (x0, y0, x1, y1): (usize, usize, usize, usize)) -> usize {
    (y0..y1).flat_map(|y| (x0..x1).map(move |x| 3 * (y * 640 + x))).filter(|&i| a[i..i + 3] != b[i..i + 3]).count()
}

/// The ride HUD's marker menu, top right of a 640x448 frame (`ride_hud::top_right`), with room to
/// spare; the original's top-centre bar left with its HUD (P170).
const BAR: (usize, usize, usize, usize) = (320, 0, 640, 160);
const WHOLE: (usize, usize, usize, usize) = (0, 0, 640, 448);

#[test]
fn with_the_hud_off_markers_draw_no_bar_and_still_teleport() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    // Thirty feet up over the start line, looking back the way the ride goes and up into the sky: the
    // ride, the placed marker's portal and the teleport are all behind the camera.
    let ([x, y, z], facing) = start(&ride(&["--frames", "1", "--overlay", "none"]));
    let pose = format!("{x},{},{z},{},45,70", y + 30.0, facing + 180.0);
    let (with, without) = (scratch("markers.tape"), scratch("plain.tape"));
    std::fs::write(&with, tape(true)).expect("the tape is written");
    std::fs::write(&without, tape(false)).expect("the tape is written");

    for renderer in ["gpu", "software"] {
        let shot = |tape: &PathBuf, overlay: &str| {
            let path = scratch(&format!("{renderer}-{overlay}-{}.png", tape.file_stem().unwrap().to_str().unwrap()));
            let _ = std::fs::remove_file(&path);
            let (shot_frame, frames) = (SHOT.to_string(), (SHOT + 2).to_string());
            let said = ride(&[
                "--renderer",
                renderer,
                "--overlay",
                overlay,
                "--shot-pose",
                &pose,
                "--play",
                tape.to_str().unwrap(),
                "--shot",
                path.to_str().unwrap(),
                "--shot-frame",
                &shot_frame,
                "--frames",
                &frames,
            ]);
            let (w, h, rgb) = read_png(&path);
            assert_eq!((w, h), (640, 448), "{renderer}, {overlay}");
            (said, rgb)
        };
        // The markers work with the HUD off: slot 1 placed, and ridden back to.
        let (said, none_marked) = shot(&with, "none");
        assert_eq!(marker_lines(&said), ["marker: placed in slot 1", "marker: to slot 1"], "{renderer}:\n{said}");
        let (said, none_plain) = shot(&without, "none");
        assert!(marker_lines(&said).is_empty(), "{renderer}:\n{said}");
        // And change no pixel of the picture: no bar, no hold bar, no label.
        let n = apart(&none_marked, &none_plain, WHOLE);
        assert_eq!(n, 0, "{renderer}: with the overlay off the markers drew over {n} pixels");

        // Under the HUD its marker menu is there, top right (since P170; it was the original's bar).
        let (said, hud_marked) = shot(&with, "hud");
        assert_eq!(marker_lines(&said), ["marker: placed in slot 1", "marker: to slot 1"], "{renderer}:\n{said}");
        let (_, hud_plain) = shot(&without, "hud");
        let n = apart(&hud_marked, &hud_plain, BAR);
        assert!(n > 500, "{renderer}: under the HUD the marker menu drew over only {n} pixels");
    }
}
