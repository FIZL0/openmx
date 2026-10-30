//! V117: our own baked shadows of the hills under Shadows All (user 2026-10-30). The shell is shot
//! from 700 ft over a free09 hill whose lee faces away from the scene's sun, once under On (the
//! disc's light map) and once under All (our bake): the ground in the hill's shadow is darker under
//! All, the open ground beside it the same within a few levels, and the second load reads the bake
//! from the mods folder's cache. That a hill is what darkens it — the same ground flattened is lit —
//! is `game_render::terrain_shadow`'s own test.

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

fn scratch() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("v117-terrain-shadows");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// The mean grey of the pixels `x0..x1` by `y0..y1` of a 640-wide picture.
fn mean(rgb: &[u8], (x0, x1): (usize, usize), (y0, y1): (usize, usize)) -> f32 {
    let mut sum = 0.0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = 3 * (y * 640 + x);
            sum += (rgb[i] as f32 + rgb[i + 1] as f32 + rgb[i + 2] as f32) / 3.0;
        }
    }
    sum / ((x1 - x0) * (y1 - y0)) as f32
}

#[test]
fn a_hill_s_lee_is_darker_under_all_and_the_open_ground_is_not() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let dir = scratch();
    let mods = dir.join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    std::fs::create_dir_all(&mods).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let shot = |name: &str, shadows: &str| {
        let path = dir.join(format!("{name}.png"));
        let _ = std::fs::remove_file(&path);
        let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
            .current_dir(&root)
            .args(["--headless", "--track", "free09", "--size", "640x448", "--overlay", "none", "--controls", "original"])
            .args(["--shadows", shadows, "--shadow-quality", "high", "--mods", mods.to_str().unwrap()])
            // Straight down over the hill's lee, 700 ft up.
            .args(["--shot-pose", "3617,700,3948,0,-89,40", "--frames", "2", "--shot", path.to_str().unwrap()])
            .output()
            .expect("the shell runs");
        let said = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
        let (w, h, rgb) = read_png(&path);
        assert_eq!((w, h), (640, 448));
        (said, rgb)
    };
    let (said_on, on) = shot("on", "on");
    assert!(!said_on.contains("terrain shadows:"), "On baked the hills:\n{said_on}");
    let (said, all) = shot("all", "all");
    assert!(said.contains("terrain shadows: free09's hills baked, "), "All did not bake the hills:\n{said}");
    // The hill's lee, in the picture's middle: darker under All.
    let lee = ((288, 352), (192, 256));
    let (lee_on, lee_all) = (mean(&on, lee.0, lee.1), mean(&all, lee.0, lee.1));
    assert!(lee_all < lee_on - 4.0, "the hill's lee under All {lee_all:.1}, under On {lee_on:.1}");
    // The open ground left of it, and the road over it: as On's.
    for (x, y) in [((0, 160), (0, 64)), ((100, 300), (90, 130)), ((480, 640), (300, 448))] {
        let (a, b) = (mean(&on, x, y), mean(&all, x, y));
        assert!((a - b).abs() <= 3.0, "open ground at {x:?} {y:?}: On {a:.1}, All {b:.1}");
    }
    // A second load reads the bake back, and draws the same.
    let (said, again) = shot("again", "all");
    assert!(said.contains("terrain shadows: free09's hills read from the cache, "), "the bake was not cached:\n{said}");
    assert!((mean(&again, lee.0, lee.1) - lee_all).abs() <= 1.0);
}
