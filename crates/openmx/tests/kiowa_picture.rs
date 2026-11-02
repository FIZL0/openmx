//! P216b: the flown Kiowa is drawn as the plane is. The shell rides free09 headless on a
//! hand-written tape that switches into the helicopter and holds Cross (the collective's climb),
//! and says under `--trace-picture` what each drawn frame did: where the helicopter was drawn
//! between the two ticks it falls between (`picture heli:`), and what it put in the seat (`pilot:`,
//! the rider's own body as the plane's). Both renderers draw it and its pilot.

use std::path::Path;
use std::process::Command;

/// The root of the pilot's node tree in the Kiowa's space (`game_render::kiowa::PILOT_SEAT`).
const SEAT: [f32; 3] = [-0.107, 3.522, 0.937];

/// Cross held for `frames` ticks, switching into the helicopter at tick 10.
fn tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the helicopter\n");
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x0079ffbf7f7f7f7f000000000000ff{} -\n", "00".repeat(17));
    }
    text + "switch 10 helicopter\n"
}

/// The shell on free09, headless, climbing in the helicopter on `renderer`: what it said, or `None`
/// with no disc to ride.
fn ride(renderer: &str) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p216b-kiowa-picture");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let play = dir.join(format!("climb-{renderer}.tape"));
    std::fs::write(&play, tape(40)).expect("the tape is written");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free09", "--size", "320x224", "--overlay", "none", "--steady-clock"])
        .args(["--renderer", renderer, "--trace-picture", "--frames", "30", "--play"])
        .arg(&play)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

/// The three numbers after `key` in `line`.
fn after(line: &str, key: &str) -> [f32; 3] {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == key).unwrap_or_else(|| panic!("no {key}: {line}"));
    let n = |j: usize| -> f32 { w[i + j].parse().unwrap_or_else(|_| panic!("not a number after {key}: {line}")) };
    [n(1), n(2), n(3)]
}

/// One word after `key` in `line`.
fn word<'a>(line: &'a str, key: &str) -> &'a str {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == key).unwrap_or_else(|| panic!("no {key}: {line}"));
    w[i + 1]
}

#[test]
fn the_kiowa_slides_between_ticks_with_its_pilot_in_the_seat() {
    for renderer in ["gpu", "software"] {
        let Some(said) = ride(renderer) else { return };
        let frames: Vec<&str> = said.lines().filter(|l| l.starts_with("picture heli: ")).collect();
        assert!(frames.len() >= 10, "{renderer}: the helicopter was hardly drawn:\n{said}");

        // The tick it is taken it is cut, drawn where it is; after, it is drawn between the tick
        // before and the tick, on the climb, never back.
        assert_eq!(word(frames[0], "cut"), "1", "{renderer}: {}", frames[0]);
        let mut slid = 0;
        for line in &frames[1..] {
            let (from, to, drawn) = (after(line, "from"), after(line, "to"), after(line, "drawn"));
            if word(line, "cut") == "1" {
                assert_eq!(drawn, to, "{renderer}: a cut is drawn at the tick: {line}");
                continue;
            }
            let blend: f32 = word(line, "blend").parse().unwrap();
            for i in 0..3 {
                let want = from[i] + (to[i] - from[i]) * blend;
                assert!((drawn[i] - want).abs() < 0.01, "{renderer}: not drawn at the blend: {line}");
            }
            if (to[1] - from[1]).abs() > 1e-3 && blend < 0.99 {
                slid += 1;
            }
        }
        assert!(slid > 0, "{renderer}: never drawn between two ticks:\n{}", frames.join("\n"));

        // Its pilot: the rider's body, rooted in the seat, in the cabin.
        let pilots: Vec<&str> = said.lines().filter(|l| l.starts_with("pilot: seat 0 ")).collect();
        assert!(pilots.len() >= frames.len(), "{renderer}: frames without a pilot:\n{said}");
        for line in pilots {
            assert_eq!(word(line, "0"), renderer, "{line}");
            let (root, middle) = (after(line, "root"), after(line, "middle"));
            let count: usize = word(line, "of").parse().unwrap();
            assert!(count > 0, "{renderer}: nothing of the pilot was drawn: {line}");
            for i in 0..3 {
                assert!((root[i] - SEAT[i]).abs() < 1e-3, "{renderer}: the pilot's root is not in the seat: {line}");
            }
            let off = [middle[0] - SEAT[0], middle[1] - SEAT[1], middle[2] - SEAT[2]];
            let far = (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]).sqrt();
            assert!(middle[0].abs() < 0.5 && far < 3.0, "{renderer}: the pilot's body is {far:.2} ft from the seat: {line}");
        }
    }
}
