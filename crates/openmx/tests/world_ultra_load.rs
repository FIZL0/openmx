//! P283: world_ultra ('Every game: one world', 77 maps) loaded too slowly and held too much for a
//! Steam Deck. On the desktop its first ride frame came 29.9 s after launch at a 2671 MB peak: the
//! wrapped world's seam joins read a far edge's ground and the near edge's in turn, sample by
//! sample, and the open world dropped and re-read the tiles under each (24 s), and every map's
//! ground pages were read and sent to the GPU (1.3 GB, and as much again uploading them).
//!
//! Now each seam edge is walked once, and only the maps near the riders hold their pages: the rest
//! are drawn from a 16-pixel page each until a rider comes within the world's reach, when the stream
//! reads the map's pages again on a thread of their own and writes them a few megabytes a frame
//! (`world paint:` lines), letting them go past the reach. The ground's first pieces go to the GPU
//! as each is built, not all held till the last is. 4.4 s and a 1910 MB peak on the same desktop.
//!
//! What is proved, on the user's world_ultra: the first frame comes under a set time and peak memory
//! (the shell's `load:` line, VmHWM) with the far maps on their far pages and the far ground not
//! built; and a sweep across the world streams maps in and lets them go with no frame writing more
//! than the budget and no update over a frame's share. The rides skip without the user's mods
//! (`P283_MODS`, or `~/repos/openmx/mods`), the disc or a GPU.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The CPU time (s) the shell may spend up to the first frame, all its threads' and the shell's own
/// thread's, and the peak memory (MB) world_ultra must come under: before P283 29.9 s of wall clock
/// and 2671 MB on the desktop, after 4.4 s and ~1910 MB. CPU time, not the wall clock (K23): with
/// the other slots' gates at load 44-69 the wall clock read 18.3 to 21.8 s (and 34.7 s at load 76)
/// while the CPU time held at 48-52 s, 10-12 s of it the shell's thread; the bounds are ~1.6 times
/// that, and the 24 s of seam reads P283 took out were the shell's thread's.
const FIRST_FRAME_CPU_S: f32 = 80.0;
const FIRST_FRAME_THREAD_S: f32 = 20.0;
const PEAK_MB: u64 = 2300;
/// The most CPU time a paint update may take of a 60 Hz frame, in ms (K23: the wall clock's 8.69
/// ms under load was the thread waiting for a core).
const UPDATE_MS: f32 = 8.0;

fn mods() -> Option<PathBuf> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let mods = std::env::var_os("P283_MODS").map(PathBuf::from).unwrap_or_else(|| Path::new(&std::env::var("HOME").unwrap()).join("repos/openmx/mods"));
    if !mods.join("tracks/world_ultra/mod.json").exists() {
        eprintln!("skipped: no world_ultra in {}", mods.display());
        return None;
    }
    Some(mods)
}

/// The shell headless on world_ultra for `frames` ticks: what it said.
fn ride(mods: &Path, frames: usize, args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--mods", mods.to_str().unwrap(), "--track", "world_ultra", "--timings", "--size", "640x360"])
        .args(["--frames", &frames.to_string()])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    said
}

fn line<'a>(said: &'a str, start: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(start)).unwrap_or_else(|| panic!("no {start:?} line:\n{said}"))
}

/// The numbers in `line`, in order.
fn numbers(line: &str) -> Vec<f32> {
    line.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter_map(|w| w.trim_matches('.').parse().ok()).collect()
}

fn on_gpu(said: &str) -> bool {
    let gpu = said.lines().any(|l| l == "renderer: gpu");
    if !gpu {
        eprintln!("skipped: no GPU");
    }
    gpu
}

#[test]
fn world_ultras_first_frame_is_quick_and_small_with_the_far_maps_not_built() {
    let Some(mods) = mods() else { return };
    let said = ride(&mods, 30, &[]);
    if !on_gpu(&said) {
        return;
    }
    let load = line(&said, "load: the ride's first frame");
    eprintln!("{load}");
    let n = numbers(load);
    assert!(load.contains(" on this thread)"), "no cpu time: {load}");
    let (peak, cpu, thread) = (n[1] as u64, n[3], n[4]);
    assert!(cpu < FIRST_FRAME_CPU_S, "the first frame took {cpu} s of cpu, over {FIRST_FRAME_CPU_S} s: {load}");
    assert!(thread < FIRST_FRAME_THREAD_S, "the first frame took {thread} s of the shell's thread, over {FIRST_FRAME_THREAD_S} s: {load}");
    assert!(peak < PEAK_MB, "the load peaked at {peak} MB, over {PEAK_MB} MB");
    // The far maps on their far pages: a few near the start hold their own.
    let paint = line(&said, "world paint: ");
    let held = said.lines().find(|l| l.contains("maps on their own pages at the start")).unwrap_or_else(|| panic!("no paint stream:\n{said}"));
    let held = numbers(held)[0] as usize;
    let maps = numbers(paint)[1] as usize;
    assert!(held > 0 && held * 4 < maps, "{held} of {maps} maps hold their pages at the start");
    // And the far ground not built.
    let ground = said.lines().find(|l| l.contains("pieces built before the ride")).unwrap_or_else(|| panic!("no ground:\n{said}"));
    let ground = numbers(ground);
    let (built, pieces) = (ground[0] as usize, ground[1] as usize);
    assert!(built * 10 < pieces, "{built} of {pieces} ground pieces built before the ride");
    // And past 2500 ft the ground built coarse (P286): 15,423 chunks at full detail, ~3100 so.
    let line = said.lines().find(|l| l.contains("pieces built before the ride")).unwrap();
    let coarse = line.split("; ").last().and_then(|w| numbers(w).first().copied()).unwrap_or(0.0) as usize;
    assert!(line.contains("coarse past") && coarse * 2 > built, "{line}");
    let chunks = numbers(line_with(&said, "held at most"))[0] as usize;
    assert!(chunks < 6000, "{chunks} ground chunks held at the start");
}

/// The line that has `what` in it.
fn line_with<'a>(said: &'a str, what: &str) -> &'a str {
    said.lines().find(|l| l.contains(what)).unwrap_or_else(|| panic!("no '{what}':\n{said}"))
}

#[test]
fn riding_across_world_ultra_streams_its_maps_in_within_a_frames_budget() {
    let Some(mods) = mods() else { return };
    // From the start along x across the mxvatv maps into the mxu ones: 54,000 feet, ten maps' edges.
    let said = ride(&mods, 900, &["--world-sweep", "56000,16000,110000,16000"]);
    if !on_gpu(&said) {
        return;
    }
    let ride = line(&said, "world paint: ").to_string();
    let stats = said.lines().find(|l| l.contains("maps shown and")).unwrap_or_else(|| panic!("no paint stream:\n{said}"));
    eprintln!("{stats}");
    let n = numbers(stats);
    let (shown, let_go, most_mb, slowest_ms) = (n[0] as usize, n[1] as usize, n[2], n[5]);
    assert!(stats.ends_with("ms of cpu"), "no cpu time: {stats}");
    assert!(shown >= 5 && let_go >= 5, "the sweep showed {shown} maps and let {let_go} go ({ride})");
    // A frame writes 4 MB and finishes the layer it is on: a 256-pixel page and its levels.
    assert!(most_mb <= 4.0 + 0.4, "a frame wrote {most_mb} MB of pages");
    assert!(slowest_ms < UPDATE_MS, "a paint update took {slowest_ms} ms of cpu: {stats}");
    // Never more maps held than the start's few and their neighbours.
    let end = numbers(said.lines().find(|l| l.contains("maps on their own pages at the end")).unwrap())[0] as usize;
    assert!(end * 4 < 77, "{end} maps held their pages at the end");
    // The ground behind let go to coarse and then out (P286): 20,521 chunks at most at full detail.
    let chunks = numbers(line_with(&said, "held at most"))[0] as usize;
    assert!(chunks < 12_000, "{chunks} ground chunks held at once in the sweep");
}
