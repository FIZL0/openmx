//! V108: `--shadows better|original|off`. The shell stands on free06's start line and is shot from
//! straight over it with each setting. The sun is low there, so the bike's shadow lies beside it on
//! the flat dirt: Original's and Better's pictures have a bike-sized patch darker than Off's, and
//! nothing brighter. Original's patch is the original's blend, the sun's `ShadowRGB` over the dirt at
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
    let shot = |mode: &str| {
        let path = scratch(&format!("{mode}.png"));
        let _ = std::fs::remove_file(&path);
        let said = ride(&["--shadows", mode, "--shot-pose", &pose, "--frames", "2", "--shot", path.to_str().unwrap()]);
        assert!(said.contains(&format!("shadows: {mode}")), "the shell did not take --shadows {mode}:\n{said}");
        let (w, h, rgb) = read_png(&path);
        assert_eq!((w, h), (640, 448));
        (said, rgb)
    };
    let (_, off) = shot("off");
    let (said, original) = shot("original");
    let (_, better) = shot("better");

    for (mode, rgb) in [("original", &original), ("better", &better)] {
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
