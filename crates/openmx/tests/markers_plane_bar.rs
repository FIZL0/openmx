//! P124: the session markers' bar on the plane, drawn by the shell itself. The shell starts free06 in
//! the biplane (`--vehicle biplane`), headless, on a hand-written tape — the plane parked, Right
//! pressed once (the plane's slot 2; since P135 Left from slot 1 is the spawn), then Down held to
//! place it — and is shot from a fixed pose
//! over the start line looking up into the sky, so nothing the plane does reaches the picture but
//! the overlay. The shot is taken while Down is still held, after the placement has fired. Under the
//! original's HUD the bar is drawn over the plane's view; with the overlay off (P117) not a pixel of
//! it. The same tape with the d-pad left alone is the frame with no markers to show. The PNG reader,
//! the shell's runner and the pose are P117's (`markers_hud_off.rs`).

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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p124-markers-plane-bar");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Ticks parked before the Right press, and between it and the placing hold.
const WAIT: usize = 20;
/// Ticks the Down hold lasts: past `markers::HOLD_MS`'s 200 ms, six 30 Hz ticks.
const HOLD: usize = 12;
/// The tick shot: the Down hold's tenth, after the placement and with Down still down.
const SHOT: usize = 2 * WAIT + 1 + 9;

/// The plane parked, nothing but the d-pad: Right for one tick and then Down held for [`HOLD`] ticks
/// when `markers`, or nothing at all.
fn tape(markers: bool) -> String {
    let frames = SHOT + 4;
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape p124\n");
    // Byte 2 holds the d-pad (Right 0x20, Down 0x40, active low), bytes 8 and 11 their pressures.
    let report = |dpad: u8, right: u8, down: u8| {
        format!("0079{:02x}ff7f7f7f7f{right:02x}0000{down:02x}0000{}", 0xff ^ dpad, "00".repeat(18))
    };
    for t in 0..frames {
        let r = match t {
            _ if !markers => report(0, 0, 0),
            t if t == WAIT => report(0x20, 0xff, 0),
            t if (2 * WAIT + 1..2 * WAIT + 1 + HOLD).contains(&t) => report(0x40, 0, 0xff),
            _ => report(0, 0, 0),
        };
        text += &format!("frame 0x3d08850a 0x{r} -\n");
    }
    text
}

/// The shell on free06 in the plane, headless at 640x448 under the original's controls: what it said.
fn ride(args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--size", "640x448", "--controls", "original", "--vehicle", "biplane"])
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

/// The bar's panel, top and centre of a 640x448 frame, with room to spare.
const BAR: (usize, usize, usize, usize) = (200, 0, 440, 90);
const WHOLE: (usize, usize, usize, usize) = (0, 0, 640, 448);

#[test]
fn flying_the_plane_the_shell_draws_its_bar() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
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
        // The plane's markers: Right onto its slot 2, Down puts the plane's marker in it.
        let (said, original_marked) = shot(&with, "original");
        assert_eq!(marker_lines(&said), ["marker: slot 2", "marker: placed in slot 2"], "{renderer}:\n{said}");
        let (said, original_plain) = shot(&without, "original");
        assert!(marker_lines(&said).is_empty(), "{renderer}:\n{said}");
        // Under the original's HUD the bar is there over the plane's view, top and centre.
        let n = apart(&original_marked, &original_plain, BAR);
        assert!(n > 500, "{renderer}: flying, the bar drew over only {n} pixels");

        // With the overlay off (P117) the plane's markers draw no bar either, and still work.
        let (said, none_marked) = shot(&with, "none");
        assert_eq!(marker_lines(&said), ["marker: slot 2", "marker: placed in slot 2"], "{renderer}:\n{said}");
        let (_, none_plain) = shot(&without, "none");
        let n = apart(&none_marked, &none_plain, WHOLE);
        assert_eq!(n, 0, "{renderer}: with the overlay off the plane's markers drew over {n} pixels");
    }
}
