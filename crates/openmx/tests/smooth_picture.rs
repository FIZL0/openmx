//! P120: after a shootoff, or riding fast, the player looked as if they were teleporting back and
//! forth between two points. The picture is drawn between the last two ticks (P52, `pace::Blend`),
//! and the thrown rider was blended in the space of the wrecked bike they are kept relative to: lying
//! still beside a bike tumbling through the air, they were swung round it between ticks — up to
//! seventy feet a frame off free09's edge — and sprang back on the next tick.
//!
//! These ride the binary headless, as a user would, with `--trace-picture` printing every drawn
//! frame's first player — the bike and the middle of the rider as drawn, the camera, and both of
//! their spots at the two ticks the frame is between:
//!
//! - full throttle down free09's long straight, drawn at 60, 144 and 45 pictures a second: every
//!   frame's bike and camera are further along the way than the frame before, never back;
//! - off free09's south edge into the shootoff and on to its put-back: every frame draws the bike and
//!   the thrown rider on the line between their two ticks' spots, and the put-back's jump is drawn
//!   once, where it lands, with no frame between the wreck and the put-back's spot.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The farthest a move between two ticks goes before the picture calls it a jump (`pace::SNAP`).
const SNAP: f32 = 30.0;

/// Full throttle and nothing else, at the 30 Hz cap: Cross down, at full pressure.
fn throttle_tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape full throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d088889 0x{report} -\n");
    }
    text
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p120-smooth-picture");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// Ride the throttle tape headless on free09 from `at` (`X,Z,DEG`) for `ticks` ticks, drawn at
/// `fps`, or `None` with no disc to ride.
fn ride(fps: u32, at: &str, ticks: usize) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let tape = scratch("throttle.tape");
    std::fs::write(&tape, throttle_tape(ticks + 30)).expect("the tape is written");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--track", "free09", "--trace-at", at, "--trace-picture"])
        .args(["--draw-fps", &fps.to_string(), "--frames", &ticks.to_string()])
        .args(["--play", tape.to_str().expect("a path")])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

type P = [f32; 3];

/// One drawn frame, off its `picture:` line.
#[derive(Debug)]
struct Frame {
    tick: u64,
    blend: f32,
    cut: bool,
    down: bool,
    bike: (P, P, P),
    eye: P,
    rider: Option<(P, P, P)>,
}

fn frames(said: &str) -> Vec<Frame> {
    said.lines()
        .filter_map(|l| l.strip_prefix("picture: "))
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let n = |i: usize| -> f32 { w[i].parse().unwrap_or_else(|_| panic!("word {i} of {l}")) };
            let p = |i: usize| -> P { [n(i), n(i + 1), n(i + 2)] };
            let words = (w[0], w[2], w[4], w[6], w[8], w[9], w[13], w[17], w[21], w[29]);
            assert_eq!(words, ("tick", "blend", "cut", "down", "bike", "from", "to", "drawn", "eye", "rider"), "{l}");
            let rider = (w[30] == "from").then(|| (p(31), p(35), p(39)));
            Frame { tick: n(1) as u64, blend: n(3), cut: w[5] == "1", down: w[7] == "1", bike: (p(10), p(14), p(18)), eye: p(22), rider }
        })
        .collect()
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: P, b: P) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len(a: P) -> f32 {
    dot(a, a).sqrt()
}

/// How far `p` is from the line between `a` and `b`, and how far along it, 0 at `a` and 1 at `b`.
fn off_line(p: P, a: P, b: P) -> (f32, f32) {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    if l2 < 1e-9 {
        return (len(sub(p, a)), 0.0);
    }
    let t = dot(sub(p, a), ab) / l2;
    let near = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    (len(sub(p, near)), t)
}

#[test]
fn a_fast_ride_moves_forward_every_frame() {
    for fps in [60, 144, 45] {
        // Five seconds down the straight north of x 4620: past 80 mph, the bike's top speed.
        let Some(said) = ride(fps, "4620,2320,0", 150) else { return };
        let shown = frames(&said);
        let fast = shown.iter().filter(|f| len(sub(f.bike.1, f.bike.0)) > 3.0).count();
        assert!(fast > 30 * fps as usize / 30, "{fps} fps: only {fast} frames drawn at speed, of {}", shown.len());
        let (mut worst_bike, mut worst_eye) = (f32::MAX, f32::MAX);
        for (a, b) in shown.iter().zip(&shown[1..]) {
            assert!(!b.cut, "{fps} fps: a ride down a straight cut at tick {}", b.tick);
            assert!((0.0..=1.0).contains(&b.blend), "{fps} fps: blend {} at tick {}", b.blend, b.tick);
            // The way the bike is going, between the two ticks this frame is drawn between.
            let way = sub(b.bike.1, b.bike.0);
            if len(way) < 0.5 {
                continue;
            }
            let way = way.map(|x| x / len(way));
            let (bike, eye) = (dot(sub(b.bike.2, a.bike.2), way), dot(sub(b.eye, a.eye), way));
            (worst_bike, worst_eye) = (worst_bike.min(bike), worst_eye.min(eye));
            assert!(bike >= -1e-3, "{fps} fps: the bike stepped back {bike} ft at tick {} ({a:?} then {b:?})", b.tick);
            assert!(eye >= -1e-3, "{fps} fps: the camera stepped back {eye} ft at tick {} ({a:?} then {b:?})", b.tick);
        }
        println!("{fps} fps: {} frames, {fast} at speed; least step forward {worst_bike:.3} ft (bike), {worst_eye:.3} ft (camera)", shown.len());
    }
}

#[test]
fn a_shootoff_and_its_put_back_draw_once() {
    // South off free09's edge at z 0 (the map's shootoff margin is 150 ft): the throw, the wreck on
    // landing and the wreck clock's put-back, inside 600 ticks.
    let Some(said) = ride(60, "4620,500,180", 600) else { return };
    let shown = frames(&said);
    let down = shown.iter().filter(|f| f.down).count();
    assert!(down > 300, "the rider was down for {down} frames of {}: no shootoff", shown.len());
    let thrown = shown.iter().filter(|f| f.down && f.rider.is_some()).count();
    assert!(thrown > 300, "the rider was drawn thrown for {thrown} frames");

    let (mut worst_rider, mut worst_bike, mut jumps) = (0.0f32, 0.0f32, 0);
    for f in &shown {
        let (from, to, drawn) = f.bike;
        if len(sub(to, from)) > SNAP {
            // A jump: drawn once, where it lands.
            jumps += 1;
            assert!(f.cut, "a {} ft jump at tick {} was blended: {f:?}", len(sub(to, from)), f.tick);
        }
        if f.cut {
            assert_eq!(drawn, to, "a cut at tick {} not drawn where it lands", f.tick);
            continue;
        }
        let (off, along) = off_line(drawn, from, to);
        worst_bike = worst_bike.max(off);
        assert!(off < 0.01 && (-1e-3..=1.001).contains(&along), "the bike was drawn {off} ft off its line, {along} along it, at tick {}", f.tick);
        if let Some((from, to, drawn)) = f.rider {
            let (off, along) = off_line(drawn, from, to);
            worst_rider = worst_rider.max(off);
            assert!(off < 0.05, "the rider was drawn {off} ft off their line at tick {}, blend {} ({f:?})", f.tick, f.blend);
            // Along the line within f32's reach at these coordinates: the rider is drawn through the
            // drawn bike's inverse and back, a few thousandths of a foot out at four thousand feet.
            let way = len(sub(to, from));
            assert!((-0.02..=way + 0.02).contains(&(along * way)), "the rider was drawn {along} of the way ({way} ft) at tick {}, blend {}", f.tick, f.blend);
        }
    }
    assert!(jumps > 0, "no put-back was drawn in {} frames", shown.len());
    println!("{} frames, {down} down, {jumps} drawn at a jump; worst off the line {worst_bike:.4} ft (bike), {worst_rider:.4} ft (rider)", shown.len());
}
