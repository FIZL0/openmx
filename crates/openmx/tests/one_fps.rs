//! P47: the shell ran at about a frame a second. The cause was drawing, not the simulation: V25's
//! splat paint (eight bilinear passes and a light map a ground pixel) and the sky, the props and the
//! menu's full-screen stretch, all on one core at the window's own size. These hold the fix:
//!
//! - a headless ride at the shell's default frame keeps to a budget a frame, and the stages that
//!   used to be the cost are each under their share of it;
//! - after the first frame nothing is read off the disc, out of an archive or from any file, and no
//!   texture is decoded: the load is the load, and a frame is only a frame.
//!
//! Both read the shell's own `--timings` lines, so they ride the binary as a user would.

use std::path::Path;
use std::process::Command;

/// Ride the shell headless with `--timings` and these arguments, or `None` with no disc to ride.
fn ride(args: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free06", "--timings"])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// The session's cost of `stage` a frame, in ms, off its `session ms a frame: …` line.
fn session_ms(said: &str, stage: &str) -> f64 {
    let line = said
        .lines()
        .find(|l| l.starts_with("session ms a frame:"))
        .unwrap_or_else(|| panic!("no session line:\n{said}"));
    let words: Vec<&str> = line.split(|c: char| c == ' ' || c == ',').filter(|w| !w.is_empty()).collect();
    let at = words.iter().position(|w| *w == stage).unwrap_or_else(|| panic!("no {stage} in {line}"));
    words[at + 1].parse().unwrap_or_else(|_| panic!("{stage} is not a number in {line}"))
}

/// Before P47 a 640x448 frame cost 129 ms, 111 of them the ground; now about 17 on this machine.
/// The budget is four 60 Hz frames: room for a loaded machine and a test build, and well short of
/// what the slow shell cost.
#[test]
fn a_headless_ride_at_the_default_frame_keeps_to_its_budget() {
    let Some(said) = ride(&["--frames", "120"]) else { return };
    assert!(said.contains("headless, drawing 640x448"), "not the default frame:\n{said}");
    assert!(said.contains("over 120 frames"), "the ride did not run its frames:\n{said}");
    let ms = |stage| session_ms(&said, stage);
    let (all, ground, sky, scene) = (ms("all"), ms("ground"), ms("sky"), ms("scene"));
    println!("a frame: {all:.1} ms, ground {ground:.1}, sky {sky:.1}, scene {scene:.1}");
    // The ground has to have been drawn for its time to mean anything.
    assert!(ground > 0.0, "no ground drawn:\n{said}");
    assert!(all < 66.7, "a frame cost {all:.1} ms, more than four 60 Hz frames:\n{said}");
    assert!(ground < 50.0, "the ground cost {ground:.1} ms a frame (it was 111 before P47):\n{said}");
    assert!(sky < 8.0 && scene < 20.0, "the sky {sky:.1} ms, the props {scene:.1} ms:\n{said}");
}

/// Four players, so four views' worth of every per-frame stage: every texture, model and file was
/// read before the first frame and nothing after it. The bytes are every read
/// the process made, of anything — a headless ride has no window socket and no pad to read.
#[test]
fn nothing_is_read_or_decoded_after_the_first_frame() {
    let Some(said) = ride(&["--players", "4", "--frames", "60"]) else { return };
    assert!(said.contains("over 60 frames"), "the ride did not run its frames:\n{said}");
    let line = said
        .lines()
        .find(|l| l.starts_with("after the first frame:"))
        .unwrap_or_else(|| panic!("no line on what came after the first frame:\n{said}"));
    assert_eq!(
        line, "after the first frame: 0 disc reads, 0 archive reads, 0 texture decodes, 0 bytes read",
        "{said}"
    );
}
