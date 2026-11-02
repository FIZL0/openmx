//! V108: `--shadows`, and `--shadow-quality` (user 2026-10-30). The shell stands on free06's start
//! line and is shot from straight over it with each setting. The sun is low there, so the bike's
//! shadow lies beside it on the flat dirt: Original's, Low's and High's pictures have a bike-sized
//! patch darker than Off's, and nothing brighter. V108's words `better` and `original` are still
//! taken, as On at High and On at Original. Original's patch is the original's blend, the sun's `ShadowRGB` over the dirt at
//! its `ShadowIntensity` squared, so its core is that blend of Off's pixels; and it leaves the share
//! of the dirt PCSX2's picture of free06's start does.

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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v108-shadows");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// The shell on free06, headless at 640x448 with no overlay, with these arguments: what it said.
fn ride(args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--size", "640x448", "--overlay", "none", "--controls", "original"])
        // The steady clock (K17): on the wall's, a loaded machine ran one tick or two before the shot's
        // picture and blended the rider between them by the time it took, so Off's and Original's
        // riders stood apart and a few of their pixels made the "deepest" shadow (a core of 2).
        .args(["--steady-clock"])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    said
}

/// The start line, off the shell's `start:` line.
fn start(said: &str) -> [f32; 3] {
    let l = said.lines().find(|l| l.starts_with("start: ")).unwrap_or_else(|| panic!("no start line:\n{said}"));
    let n: Vec<f32> = l.split([' ', ',']).filter_map(|w| w.parse().ok()).collect();
    [n[0], n[1], n[2]]
}

/// The original's shadow colour (0..255) and strength, off the shell's `shadows:` line.
fn tint(said: &str) -> (f32, f32) {
    let l = said.lines().find(|l| l.starts_with("shadows: ")).unwrap_or_else(|| panic!("no shadows line:\n{said}"));
    let n: Vec<f32> = l.split([' ', ',', '(', ')']).filter_map(|w| w.parse().ok()).collect();
    assert_eq!(n.len(), 4, "the shadows line has no tint: {l}");
    ((n[0] + n[1] + n[2]) / 3.0, n[3])
}

fn grey(rgb: &[u8], i: usize) -> f32 {
    (rgb[3 * i] as f32 + rgb[3 * i + 1] as f32 + rgb[3 * i + 2] as f32) / 3.0
}

#[test]
fn the_bike_s_shadow_darkens_the_ground_under_original_and_better_and_not_off() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let [x, y, z] = start(&ride(&["--frames", "1"]));
    // Thirty-four feet over the bike, looking down: the bike in the middle, its shadow beside it.
    let pose = format!("{x},{},{},0,-85,60", y + 34.0, z + 2.0);
    let shot = |name: &str, flags: &[&str], taken: &str| {
        let path = scratch(&format!("{name}.png"));
        let _ = std::fs::remove_file(&path);
        let mut args = flags.to_vec();
        args.extend(["--shot-pose", &pose, "--frames", "2", "--shot", path.to_str().unwrap()]);
        let said = ride(&args);
        assert!(said.contains(&format!("shadows: {taken} (")), "the shell did not take {flags:?} as {taken}:\n{said}");
        let (w, h, rgb) = read_png(&path);
        assert_eq!((w, h), (640, 448));
        (said, rgb)
    };
    let (_, off) = shot("off", &["--shadows", "off"], "off, quality high");
    // V108's old words: Original is On at the original's quality, Better On at High.
    let (said, original) = shot("original", &["--shadows", "original"], "on, quality original");
    let (_, better) = shot("better", &["--shadows", "better"], "on, quality high");
    let (_, low) = shot("low", &["--shadows", "on", "--shadow-quality", "low"], "on, quality low");

    for (mode, rgb) in [("original", &original), ("high", &better), ("low", &low)] {
        let darker = (0..640 * 448).filter(|&i| grey(&off, i) - grey(rgb, i) > 20.0).count();
        let brighter = (0..640 * 448).filter(|&i| grey(rgb, i) - grey(&off, i) > 20.0).count();
        // The bike's shadow from 34 ft at 60° is some 8,000 pixels; the bike alone over 1,000.
        assert!(darker > 3000, "{mode}: only {darker} pixels are darker than off's");
        assert!(brighter < 300, "{mode}: {brighter} pixels are brighter than off's");
    }

    // Original's core, where its shadow is fullest: off's dirt blended toward the tint.
    let (rgb, alpha) = tint(&said);
    let deepest = (0..640 * 448).map(|i| grey(&off, i) - grey(&original, i)).fold(0.0, f32::max);
    let core: Vec<usize> = (0..640 * 448).filter(|&i| grey(&off, i) - grey(&original, i) > 0.8 * deepest).collect();
    assert!(core.len() > 500, "original's shadow has no core: {} pixels", core.len());
    let a = alpha * alpha;
    let miss = core.iter().map(|&i| grey(&original, i) - (grey(&off, i) * (1.0 - a) + rgb * a)).sum::<f32>() / core.len() as f32;
    assert!(miss.abs() < 8.0, "original's shadow is {miss} grey levels off the blend of {rgb} at {alpha} squared");
    // PCSX2's own (V108): the rider's shadow behind the bike at the start, its core 74 grey on dirt
    // of 104-121 round it, about 0.63 of the dirt.
    let left = core.iter().map(|&i| grey(&original, i) / grey(&off, i).max(1.0)).sum::<f32>() / core.len() as f32;
    assert!((left - 0.63).abs() < 0.07, "original's shadow leaves {left} of the dirt, PCSX2's about 0.63");
}

/// A tape of `n` frames at 30 Hz with the gas held (Cross on the original's controls).
fn gas_tape(n: usize) -> String {
    let mut pad = [0u8; 32];
    pad[..8].copy_from_slice(&[0x00, 0x79, 0xff, 0xbf, 0x7f, 0x7f, 0x7f, 0x7f]);
    pad[14] = 0xff;
    let hex: String = pad.iter().map(|b| format!("{b:02x}")).collect();
    let mut tape = format!("# game-sim pad tape: {n} frames\nformat 1\ntape gas\n");
    for _ in 0..n {
        tape += &format!("frame 0x3d08850a 0x{hex} -\n");
    }
    tape
}

/// The user's "when my shadow gets long enough it just disappears" (V108's follow-up): free09's big
/// jump, ridden flat out from 3864, 1592 heading -132, has the bike still well up off the dirt
/// at tick 130 (some 55 ft up at its top, tick 102) under a low sun, its shadow far off in the
/// bottom right of the ride camera's picture. Original's box reached only 40 ft along the sun past the bike, so the
/// shadow was gone there (no pixel darker than Off's in that corner); now it reaches the ground.
#[test]
fn a_high_jump_s_long_shadow_still_reaches_the_ground() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let tape = scratch("gas.tape");
    std::fs::write(&tape, gas_tape(140)).expect("the tape");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shot = |quality: &str| {
        let path = scratch(&format!("jump-{quality}.png"));
        let _ = std::fs::remove_file(&path);
        let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
            .current_dir(&root)
            .args(["--headless", "--track", "free09", "--size", "640x360", "--overlay", "none", "--controls", "original"])
            .args(["--trace-at", "3864.2,1592.5,-132", "--play", tape.to_str().unwrap(), "--frames", "132", "--steady-clock"])
            .args(if quality == "off" { ["--shadows", "off", "--shadow-quality", "high"] } else { ["--shadows", "on", "--shadow-quality", quality] })
            .args(["--shot", path.to_str().unwrap(), "--shot-frame", "130"])
            .output()
            .expect("the shell runs");
        assert!(out.status.success(), "the shell failed: {}", String::from_utf8_lossy(&out.stderr));
        let (w, h, rgb) = read_png(&path);
        assert_eq!((w, h), (640, 360));
        rgb
    };
    let off = shot("off");
    for mode in ["original", "high"] {
        let rgb = shot(mode);
        let corner = (240..360).flat_map(|y| (400..640).map(move |x| y * 640 + x));
        let darker = corner.filter(|&i| grey(&off, i) - grey(&rgb, i) > 20.0).count();
        // About 550-600 pixels of shadow either way; none at all when it fell out of the box.
        assert!(darker > 300, "{mode}: the jump's shadow is gone, only {darker} pixels darker than off's");
    }
}
