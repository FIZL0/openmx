//! P270: "when poofing it should reset your character without animation, right now it teleports you
//! smoothly and causes visual issues especially when tricking". A put-back (the L1+R1 chord, a wreck's
//! reset) cuts the picture to the new spot (S74), but the rider then sprang over from the trick's pose
//! (the riding ragdoll's spring, S90), the bike's clip faded back from the trick's over a third of a
//! second with the grips and the rider held to them, and the camera chased the bike for a second from
//! where it had been in the air.
//!
//! These ride the binary headless, as a user would, with `--trace-picture` printing every drawn
//! frame's first player: the bike, the camera and the middle of the rider as drawn, and each of the
//! rider's drawn nodes in the bike's frame (`picture pose:`). Full throttle on free06 off its first
//! jump, Circle and the stick up from tick 250 for a trick, and the chord in the middle of it:
//!
//! - the frame before the poof draws the trick (the pose far from riding), the poof's frame cuts, and
//!   from that frame on every node is within [`POSE_NEAR`] of the pose the rider settles to, with the
//!   rider's middle at the new spot and not the old;
//! - the camera's eye jumps with the bike on that frame and stays put after it.
//!
//! Once with the riding ragdoll drawn (the default) and once with the clip pose
//! (`OPENMX_RIDING_RAGDOLL=0`).

use std::path::{Path, PathBuf};
use std::process::Command;

/// The tick the trick is asked from, and the tick the chord goes down: mid-trick, off free06's first
/// jump at full throttle.
const TRICK: usize = 250;
const POOF: usize = 275;
const TICKS: usize = 300;

/// The farthest any drawn node is from its settled spot on the bike, from the poof's own frame on, in
/// feet. Measured 0.21 (ragdoll) and 0.35 (clip) after the fix, where the riding body follows the
/// rate-limited lean back to upright as it does on any ride; before it the pose was 5.7 ft off on the
/// cut, swung 3.4-4.5 ft back towards the trick under the spring, and slid 1.6 ft with the bike's clip.
const POSE_NEAR: f32 = 0.6;

/// The most the eye moves in the [`SETTLE`] frames after the poof's, in feet. Measured 0.1 after the
/// fix; before it the eye chased the bike down from 25 ft away.
const EYE_STILL: f32 = 1.0;

/// How many ticks after the poof the rider has settled, which the pose is measured against.
const SETTLE: u64 = 20;

/// Full throttle (Cross) to tick 248, Circle and the left stick up over `TRICK..POOF`, and L1+R1 for
/// three ticks from `POOF`, at the 30 Hz cap.
fn poof_tape() -> String {
    let mut text = format!("# game-sim pad tape: {} frames\nformat 1\ntape poof mid-trick\n", TICKS + 40);
    for f in 0..TICKS + 40 {
        let (mut b2, mut ly) = (0xffu8, 0x7fu8);
        // Pressures: right left up down triangle circle cross square l1 r1 l2 r2.
        let mut pressure = [0u8; 12];
        if f < 248 {
            b2 &= !0x40;
            pressure[6] = 0xff;
        }
        if (TRICK..POOF).contains(&f) {
            b2 &= !0x20;
            pressure[5] = 0xff;
            ly = 0x00;
        }
        if (POOF..POOF + 3).contains(&f) {
            b2 &= !0x0c;
            pressure[8] = 0xff;
            pressure[9] = 0xff;
        }
        let pressures: String = pressure.iter().map(|p| format!("{p:02x}")).collect();
        let report = format!("0079ff{b2:02x}7f7f7f{ly:02x}{pressures}{}", "00".repeat(12));
        text += &format!("frame 0x3d088889 0x{report} -\n");
    }
    text
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p270-poof-cut");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Ride the poof tape headless on free06, the riding ragdoll drawn or not, or `None` with no disc.
fn ride(ragdoll: bool) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    // One tape a test: the two run side by side, and a file being rewritten is no tape to play.
    let tape = scratch(if ragdoll { "poof-ragdoll.tape" } else { "poof-clip.tape" });
    std::fs::write(&tape, poof_tape()).expect("the tape is written");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .env("OPENMX_RIDING_RAGDOLL", if ragdoll { "1" } else { "0" })
        .args(["--headless", "--steady-clock", "--no-sound", "--track", "free06", "--trace-picture"])
        .args(["--draw-fps", "30", "--frames", &TICKS.to_string()])
        .args(["--play", tape.to_str().expect("a path")])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

type P = [f32; 3];

/// One drawn frame, off its `picture:` line and the `picture pose:` line after it.
#[derive(Debug)]
struct Frame {
    tick: u64,
    cut: bool,
    bike: P,
    eye: P,
    rider: Option<P>,
    pose: Vec<P>,
}

fn frames(said: &str) -> Vec<Frame> {
    let mut out: Vec<Frame> = Vec::new();
    for l in said.lines() {
        if let Some(l) = l.strip_prefix("picture: ") {
            let w: Vec<&str> = l.split_whitespace().collect();
            let n = |i: usize| -> f32 { w[i].parse().unwrap_or_else(|_| panic!("word {i} of {l}")) };
            let p = |i: usize| -> P { [n(i), n(i + 1), n(i + 2)] };
            let words = (w[0], w[4], w[8], w[17], w[21], w[29]);
            assert_eq!(words, ("tick", "cut", "bike", "drawn", "eye", "rider"), "{l}");
            // `rider none` before the body has been posed.
            let rider = (w[30] == "from").then(|| p(39));
            out.push(Frame { tick: n(1) as u64, cut: w[5] == "1", bike: p(18), eye: p(22), rider, pose: Vec::new() });
        } else if let Some(l) = l.strip_prefix("picture pose: ") {
            let w: Vec<&str> = l.split_whitespace().collect();
            assert_eq!((w[0], w[4]), ("tick", "nodes"), "{l}");
            let tick: u64 = w[1].parse().expect("a tick");
            let v: Vec<f32> = w[6..].iter().map(|x| x.parse().expect("a coordinate")).collect();
            let frame = out.last_mut().filter(|f| f.tick == tick).expect("a pose line follows its picture line");
            frame.pose = v.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
        }
    }
    out
}

fn dist(a: P, b: P) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The farthest any node of `a` is from its own in `b`.
fn off(a: &[P], b: &[P]) -> f32 {
    assert_eq!(a.len(), b.len(), "the drawn body changed its node count");
    a.iter().zip(b).map(|(a, b)| dist(*a, *b)).fold(0.0, f32::max)
}

fn poof_draws_the_riding_pose_at_once(ragdoll: bool) {
    let Some(said) = ride(ragdoll) else { return };
    let shown = frames(&said);
    let at = shown
        .iter()
        .position(|f| f.cut && f.tick > TRICK as u64)
        .unwrap_or_else(|| panic!("no frame cut after the chord (ragdoll {ragdoll})"));
    let (before, poof) = (&shown[at - 1], &shown[at]);
    let settled = shown
        .iter()
        .find(|f| f.tick == poof.tick + SETTLE)
        .unwrap_or_else(|| panic!("no frame {SETTLE} ticks after the poof at {}", poof.tick));
    assert!(!before.pose.is_empty() && !poof.pose.is_empty() && !settled.pose.is_empty(), "frames with no rider drawn");
    // The put-back moved the bike, and the frame before it drew the trick, not the riding pose.
    assert!(dist(before.bike, poof.bike) > 20.0, "the poof only moved the bike {} ft", dist(before.bike, poof.bike));
    let trick = off(&before.pose, &settled.pose);
    assert!(trick > 2.0, "the frame before the poof (tick {}) was only {trick} ft off riding: no trick showing", before.tick);
    // The rider is drawn at the new spot on the poof's own frame, on the bike and nowhere near the old.
    let (Some(rider), Some(was)) = (poof.rider, before.rider) else { panic!("no rider drawn around the poof") };
    assert!(dist(rider, poof.bike) < 6.0, "tick {}: the rider is {} ft off the bike", poof.tick, dist(rider, poof.bike));
    assert!(dist(rider, was) > 20.0, "tick {}: the rider is drawn near the old spot", poof.tick);
    // And on the riding pose from that frame on: no spring, slide or fade back from the trick.
    for f in shown.iter().filter(|f| (poof.tick..=settled.tick).contains(&f.tick)) {
        let d = off(&f.pose, &settled.pose);
        assert!(d < POSE_NEAR, "ragdoll {ragdoll}: tick {} (poof at {}) draws a node {d} ft off the riding pose", f.tick, poof.tick);
    }
    // The camera cut with it: its eye jumped on the poof's frame and does not chase after.
    assert!(dist(before.eye, poof.eye) > 10.0, "the eye only moved {} ft on the poof", dist(before.eye, poof.eye));
    for f in shown.iter().filter(|f| (poof.tick..=settled.tick).contains(&f.tick)) {
        let d = dist(f.eye, poof.eye);
        assert!(d < EYE_STILL, "tick {}: the eye chased {d} ft from where the poof put it (tick {})", f.tick, poof.tick);
    }
}

#[test]
fn a_poof_mid_trick_draws_the_riding_ragdoll_at_once() {
    poof_draws_the_riding_pose_at_once(true);
}

#[test]
fn a_poof_mid_trick_draws_the_riding_clip_at_once() {
    poof_draws_the_riding_pose_at_once(false);
}
