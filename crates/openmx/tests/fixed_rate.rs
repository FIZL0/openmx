//! P52: the ride went into slow motion when the picture was slow. The shell stepped the game once a
//! drawn frame on the host's clock, and the engine caps a frame at 1/30 s, so at 5 fps drawn the world
//! ran at a sixth of real time. Now the ticks keep the original's 30 a second on the wall clock and
//! read the clock a steady PS2 would, whatever the picture does (`pace`).
//!
//! These ride the binary headless, as a user would, at a forced 5 and 60 pictures a second:
//!
//! - the same tape of full throttle, for the same three seconds, is the same number of ticks and ends
//!   on exactly the same state, and that state is somewhere else than the start line;
//! - an untouched ride's engine hands the simulation the same `dt` tick for tick at both rates —
//!   the tape `--record` writes holds it — climbing from the load's 0.033 to the 30 Hz cap and
//!   staying there.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The ticks due in the first three seconds: one every 1001/30,000 s, from 0.
const TICKS: usize = 90;

/// Ride headless at `fps` pictures a second for three seconds with these arguments, or `None` with
/// no disc to ride.
fn ride(fps: u32, args: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--seconds", "3", "--draw-fps", &fps.to_string()])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// The line starting `prefix`.
fn line<'a>(said: &'a str, prefix: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no `{prefix}` line:\n{said}"))
}

/// How long the ride took, how many ticks it ran and where it ended, off its last line.
fn ended(said: &str) -> (f32, usize, [f32; 3]) {
    let l = line(said, "ridden for ");
    let words: Vec<&str> = l.split([' ', ',']).filter(|w| !w.is_empty()).collect();
    let n = |i: usize| -> f32 { words[i].trim_end_matches('s').parse().unwrap_or_else(|_| panic!("{l}")) };
    (n(2), n(4) as usize, [n(8), n(9), n(10)])
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p52-fixed-rate");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Full throttle and nothing else, at the engine's capped `dt`: Cross down, at full pressure.
fn throttle_tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape full throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text
}

#[test]
fn a_ride_drawn_at_5_fps_is_the_ride_drawn_at_60() {
    let tape = scratch("throttle.tape");
    std::fs::write(&tape, throttle_tape(4 * TICKS)).expect("the tape is written");
    let tape = tape.to_str().expect("a path");
    let Some(slow) = ride(5, &["--play", tape]) else { return };
    let fast = ride(60, &["--play", tape]).expect("the disc was there a moment ago");
    let idle = ride(60, &[]).expect("the disc was there a moment ago");

    let (slow_s, slow_ticks, slow_at) = ended(&slow);
    let (fast_s, fast_ticks, fast_at) = ended(&fast);
    let (_, _, idle_at) = ended(&idle);
    println!("5 fps: {slow_s}s, {slow_ticks} ticks, at {slow_at:?}; 60 fps: {fast_s}s, {fast_ticks} ticks, at {fast_at:?}");
    // The same wall time, and the ticks it owes: before P52, 5 fps rode 15 of them.
    for s in [slow_s, fast_s] {
        assert!((3.0..3.6).contains(&s), "a three-second ride took {s}s:\n{slow}\n{fast}");
    }
    assert_eq!((slow_ticks, fast_ticks), (TICKS, TICKS), "\n{slow}\n{fast}");
    // Exactly the same state, not nearly.
    assert_eq!(line(&slow, "ride hash"), line(&fast, "ride hash"));
    assert_eq!(slow_at, fast_at);
    // And the throttle took the bike somewhere: three seconds on the gas is well clear of where
    // three seconds untouched leaves it.
    let moved = (0..3).map(|i| (slow_at[i] - idle_at[i]).powi(2)).sum::<f32>().sqrt();
    assert!(moved > 20.0, "full throttle ended {moved} ft from an untouched ride:\n{slow}\n{idle}");
}

#[test]
fn the_engines_dt_is_the_same_every_tick_at_any_drawn_rate() {
    let (slow_tape, fast_tape) = (scratch("idle-5.tape"), scratch("idle-60.tape"));
    let Some(slow) = ride(5, &["--record", slow_tape.to_str().expect("a path")]) else { return };
    let fast = ride(60, &["--record", fast_tape.to_str().expect("a path")]).expect("the disc was there a moment ago");
    assert_eq!(line(&slow, "ride hash"), line(&fast, "ride hash"));

    let dts = |path: &Path| -> Vec<f32> {
        let text = std::fs::read_to_string(path).expect("the tape was written");
        text.lines()
            .filter_map(|l| l.strip_prefix("frame 0x"))
            .map(|l| f32::from_bits(u32::from_str_radix(&l[..8], 16).expect("a dt")))
            .collect()
    };
    let (slow, fast) = (dts(&slow_tape), dts(&fast_tape));
    assert_eq!(slow.len(), TICKS);
    assert_eq!(slow.iter().map(|d| d.to_bits()).collect::<Vec<_>>(), fast.iter().map(|d| d.to_bits()).collect::<Vec<_>>());
    // From the load's 0.033 up to the cap within the first second, and on it after.
    assert!((slow[0] - 0.033).abs() < 1e-6, "the first tick's dt is {}", slow[0]);
    assert!(slow.windows(2).all(|w| w[0] <= w[1]), "{slow:?}");
    let capped = slow.iter().position(|&d| d == 0.03333).expect("the ride reaches the 30 Hz cap");
    assert!(capped < 30, "{slow:?}");
    assert!(slow[capped..].iter().all(|&d| d == 0.03333), "{slow:?}");
}
