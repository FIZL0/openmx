//! P82: the player's camera framing, a view laid over the original's presets.
//!
//! The Camera page's Original notch hands the camera the disc's presets untouched, so the follow
//! camera at that notch is the original's to the bit, frame for frame. The shipped default sits
//! further back, higher and wider, by the shares it says. The wreck camera's distances scale with
//! the bike's, and at the Original notch are the original's constants.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use chase::{Framing, WreckCamera, framed};
use game_sim::camera::{CurveTable, DEG_TO_RAD, Nothing, Preset, Race, Ridden, Switches, Target, View};
use ps2_float::{F, Vf};

/// The shell's camera numbers (`main.rs`), as `tests/biplane_camera.rs` repeats them.
const REVERSE_SPEED: f32 = 5.0;
const SPEED_REFERENCE: f32 = 117.332_8;
const NEAR_CLIP: f32 = 2.4;

/// The shipped default, as the Camera page's `CameraView::DEFAULT` frames the bike: the menu's
/// numbers, which `tests/menu.rs` holds the page to.
const DEFAULT: Framing = Framing { fov: 4.0, distance: 1.15, height: 1.10 };

/// The disc's free06 follow cameras, the way the shell reads them; `None` without the disc.
fn disc_presets() -> Option<Vec<Preset>> {
    use game_sim::archive::{Psr, Res};
    use std::io::{Read, Seek, SeekFrom};
    let iso = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("iso"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso"),
    };
    let mut file = std::fs::File::open(iso.join("MX.PSR")).ok()?;
    let mut front = vec![0u8; 64 << 10];
    file.read_exact(&mut front).ok()?;
    let psr = Psr::parse(&front).ok()?;
    let e = psr.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("free06_2.res"))?.clone();
    file.seek(SeekFrom::Start(e.at)).ok()?;
    let mut bytes = vec![0u8; e.size as usize];
    file.read_exact(&mut bytes).ok()?;
    let res = Res::parse(&bytes).ok()?;
    let ini = res.entries.iter().find(|e| {
        let n = e.name.to_ascii_lowercase();
        n == "cpresets.ini" || n.ends_with("\\cpresets.ini")
    })?;
    let presets = Preset::read_bytes(&res.read(&bytes, ini).ok()?, "");
    (!presets.is_empty()).then_some(presets)
}

/// `Race Cam 1`'s numbers as `research/oracle.txt` reads them off the file (AngleY 180, AngleX 65,
/// FollowingDistance 13, MinFollowingDistance 5, FOV 85), with lags and limits of this file's own:
/// the camera without the disc.
fn stand_in() -> Vec<Preset> {
    vec![Preset {
        name: "Race Cam 1".into(),
        look_filters: Vf::new(0.2, 0.2, 0.2, 0.0),
        pos_filters: Vf::new(0.3, 0.3, 0.3, 0.0),
        offset_from_body_point: Vf::new(0.0, 2.0, 0.0, 0.0),
        angle_y: F(180.0 * DEG_TO_RAD),
        angle_x: F(65.0 * DEG_TO_RAD),
        following_distance: F(13.0),
        fov: F(85.0),
        filter_this_preset: true,
        cycle_thru_this_preset: true,
        look_ahead_distance: F(1.0),
        min_following_distance: F(5.0),
        max_following_distance: F(40.0),
        min_fov: F(30.0),
        max_fov: F(100.0),
        ..Preset::default()
    }]
}

fn presets() -> Vec<Preset> {
    disc_presets().unwrap_or_else(|| {
        eprintln!("no disc: the stand-in Race Cam 1");
        stand_in()
    })
}

/// The presets with the low-speed droop off: the camera's pitch as it is at speed. Below the
/// preset's cutoff the original scales the pitch down (by half on Race Cam 1 standing still), which
/// a framing leaves alone, so the shares are exact only at the pitch the rider sees at speed.
fn at_speed(mut presets: Vec<Preset>) -> Vec<Preset> {
    for p in &mut presets {
        p.low_speed_angle_x_vel_cutoff = F::ZERO;
    }
    presets
}

/// A rider standing still at `at`, facing +z.
fn standing(at: Vf) -> Ridden {
    Ridden {
        target: Target {
            body_point: at,
            forward: Vf::new(0.0, 0.0, 1.0, 1.0),
            up: Vf::new(0.0, 1.0, 0.0, 1.0),
            vel: Vf::new(0.0, 0.0, 0.0, 1.0),
            lean: F::ZERO,
        },
        airborne: false,
        speed: F::ZERO,
    }
}

/// `frames` frames of the shell's race camera on Race Cam 1 over `presets`, behind a rider standing
/// at `at`, snapped on the first as the shell snaps it.
fn film(presets: Vec<Preset>, at: Vf, frames: usize) -> Vec<View> {
    let curve = CurveTable::build();
    let mut cam = Race::new(presets, F(REVERSE_SPEED));
    let start = Preset::find(&cam.cam.presets, "Race Cam 1").expect("Race Cam 1");
    cam.cam.set_preset(start);
    cam.cam.snap = true;
    cam.cam.speed_reference = F(SPEED_REFERENCE);
    let rider = standing(at);
    let switches = Switches { no_look_ahead: false, short_viewport: false };
    (0..frames).map(|_| cam.update(F(1.0 / 60.0), &rider, F(NEAR_CLIP), &curve, switches, &mut Nothing)).collect()
}

fn bits(v: &View) -> [u32; 7] {
    [
        v.position.x().to_bits(),
        v.position.y().to_bits(),
        v.position.z().to_bits(),
        v.look_at.x().to_bits(),
        v.look_at.y().to_bits(),
        v.look_at.z().to_bits(),
        v.fov.0.to_bits(),
    ]
}

/// The Original notch is the disc's presets to the bit, and the camera over them films the same
/// frames to the bit as the camera over the file's own.
#[test]
fn the_original_notch_is_the_originals_camera_bit_for_bit() {
    let original = presets();
    let notch = framed(&original, Framing::ORIGINAL);
    assert_eq!(format!("{original:?}"), format!("{notch:?}"), "the Original notch moved a preset");
    let at = Vf::new(2884.0, 338.0, 2750.0, 1.0);
    let a = film(original, at, 120);
    let b = film(notch, at, 120);
    for (i, (a, b)) in a.iter().zip(&b).enumerate() {
        assert_eq!(bits(a), bits(b), "frame {i}");
    }
}

/// The shipped default sits further back, higher and wider than the original: behind the standing
/// rider by the distance share, above the target point by the height share, and the field of view
/// the offset wider.
#[test]
fn the_default_sits_further_back_higher_and_wider() {
    let original = at_speed(presets());
    let at = Vf::new(0.0, 0.0, 0.0, 1.0);
    let o = film(original.clone(), at, 600);
    let d = film(framed(&original, DEFAULT), at, 600);
    let (o, d) = (o.last().unwrap(), d.last().unwrap());
    // The legs from the rider's body point: behind is along -z (the rider faces +z), up is y. The
    // target point sits the preset's offset above the body point, which neither framing moves.
    let race = original.iter().find(|p| p.name == "Race Cam 1").unwrap();
    let lift = race.offset_from_body_point.y();
    let legs = |v: &View| (-(v.position.z() - at.z()), v.position.y() - at.y() - lift);
    let ((ob, oh), (db, dh)) = (legs(o), legs(d));
    assert!(ob > 0.0 && oh > 0.0, "the original's eye is not behind and above: {ob}, {oh}");
    assert!((db / ob - DEFAULT.distance).abs() < 0.01, "behind {db} against the original's {ob}");
    assert!((dh / oh - DEFAULT.height).abs() < 0.01, "above {dh} against the original's {oh}");
    assert!((d.fov.0 - o.fov.0 - DEFAULT.fov).abs() < 1e-3, "fov {} against the original's {}", d.fov.0, o.fov.0);
    assert!(db > ob && dh > oh && d.fov.0 > o.fov.0);
}

/// The two sliders are apart: the distance moves the eye back and leaves its height, the height lifts
/// it and leaves how far back it is.
#[test]
fn distance_and_height_move_the_eye_apart() {
    let original = at_speed(presets());
    let at = Vf::new(0.0, 0.0, 0.0, 1.0);
    let eye = |f: Framing| *film(framed(&original, f), at, 600).last().unwrap();
    let o = eye(Framing::ORIGINAL);
    let back = eye(Framing { distance: 1.5, ..Framing::ORIGINAL });
    let up = eye(Framing { height: 1.5, ..Framing::ORIGINAL });
    assert!((back.position.y() - o.position.y()).abs() < 0.01, "the distance moved the height");
    assert!(back.position.z() < o.position.z() - 1.0, "the distance did not move the eye back");
    assert!((up.position.z() - o.position.z()).abs() < 0.01, "the height moved the distance");
    assert!(up.position.y() > o.position.y() + 1.0, "the height did not lift the eye");
}

/// The wreck camera at the Original notch keeps the original's distances and field-of-view limits,
/// and under the default its distances scale with the bike's and its limits move with the offset.
#[test]
fn the_wreck_camera_scales_with_the_bike() {
    let mut wreck = WreckCamera::new(false);
    wreck.frame(Framing::ORIGINAL);
    assert_eq!(
        [wreck.min_distance(), wreck.max_distance(), wreck.min_fov(), wreck.max_fov()].map(f32::to_bits),
        [13.0f32, 100.0, 20.0, 82.0].map(f32::to_bits)
    );
    wreck.frame(DEFAULT);
    assert_eq!(wreck.min_distance(), 13.0 * DEFAULT.distance);
    assert_eq!(wreck.max_distance(), 100.0 * DEFAULT.distance);
    assert_eq!(wreck.max_fov(), 82.0 + DEFAULT.fov);
}

// --- P119: every Camera page value steady, a change eased --------------------------------------------

use chase::{FovEase, reframe};

/// The shell's tick.
const TICK: f32 = 1.0 / 30.0;

/// A camera file of free06's archive by its name, raw; `None` without the disc.
fn disc_file(ends: &str) -> Option<Vec<u8>> {
    use game_sim::archive::{Psr, Res};
    use std::io::{Read, Seek, SeekFrom};
    let iso = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("iso"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso"),
    };
    let mut file = std::fs::File::open(iso.join("MX.PSR")).ok()?;
    let mut front = vec![0u8; 64 << 10];
    file.read_exact(&mut front).ok()?;
    let psr = Psr::parse(&front).ok()?;
    let e = psr.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("free06_2.res"))?.clone();
    file.seek(SeekFrom::Start(e.at)).ok()?;
    let mut bytes = vec![0u8; e.size as usize];
    file.read_exact(&mut bytes).ok()?;
    let res = Res::parse(&bytes).ok()?;
    let ini = res.entries.iter().find(|e| {
        let n = e.name.to_ascii_lowercase();
        n == ends || n.ends_with(&format!("\\{ends}")) || n.ends_with(&format!("/{ends}"))
    })?;
    res.read(&bytes, ini).ok()
}

/// The follow cameras a player can be on: the bike's and the plane's, each a full screen's and the
/// stacked split's copies (`Preset{n}h`), as `(what, presets, the preset the shell starts on)`. The
/// stand-in Race Cam 1 for each without the disc.
fn cameras() -> Vec<(String, Vec<Preset>, usize)> {
    let mut out = Vec::new();
    for (file, start) in [("cpresets.ini", "Race Cam 1"), ("biplanecpresets.ini", "Flight Cam 1")] {
        for suffix in ["", "h"] {
            let presets = disc_file(file).map(|b| Preset::read_bytes(&b, suffix)).filter(|p| !p.is_empty());
            let presets = presets.unwrap_or_else(stand_in);
            let at = Preset::find(&presets, start).unwrap_or(0);
            out.push((format!("{file}{suffix}"), presets, at));
        }
    }
    if disc_file("cpresets.ini").is_none() {
        eprintln!("no disc: the stand-in Race Cam 1 for every camera");
    }
    out
}

/// A rider at speed `ft_s` on a weave over flat ground at tick `t`: along +z, the heading swinging
/// a quarter-radian either way every four seconds and the lean with it.
fn riding(t: usize, ft_s: f32) -> Ridden {
    let s = t as f32 * TICK;
    let heading = 0.25 * (s * std::f32::consts::TAU / 4.0).sin();
    let forward = Vf::new(heading.sin(), 0.0, heading.cos(), 1.0);
    // The heading's integral along the weave, near enough: the body point only has to move on.
    let at = Vf::new(10.0 * (1.0 - (s * std::f32::consts::TAU / 4.0).cos()), 0.0, ft_s * s, 1.0);
    Ridden {
        target: Target {
            body_point: at,
            forward,
            up: Vf::new(0.0, 1.0, 0.0, 1.0),
            vel: Vf::new(forward.x() * ft_s, 0.0, forward.z() * ft_s, 1.0),
            lean: F(0.3 * (s * std::f32::consts::TAU / 4.0).cos()),
        },
        airborne: false,
        speed: F(ft_s),
    }
}

/// The shell's camera on `presets`' preset `at`, snapped on its first frame.
fn shell_camera(presets: Vec<Preset>, at: usize) -> Race {
    let mut cam = Race::new(presets, F(REVERSE_SPEED));
    cam.cam.set_preset(at);
    cam.cam.snap = true;
    cam.cam.speed_reference = F(SPEED_REFERENCE);
    cam
}

/// One tick of the shell's camera at tick `t`: the update, and the view's field of view eased.
fn tick(cam: &mut Race, ease: &mut FovEase, curve: &CurveTable, t: usize, short: bool) -> View {
    let switches = Switches { no_look_ahead: false, short_viewport: short };
    let mut v = cam.update(F(TICK), &riding(t, 40.0), F(NEAR_CLIP), curve, switches, &mut Nothing);
    v.fov = ease.ease(v.fov, TICK);
    v
}

/// The view's up vector as a renderer looking from the eye at the look-at with the world's up builds
/// it, and the view's length: upright is a positive `y` and a field of view inside half a turn.
fn up_y(v: &View) -> (f32, f32) {
    let f = [v.look_at.x() - v.position.x(), v.look_at.y() - v.position.y(), v.look_at.z() - v.position.z()];
    let l = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
    let fy = f[1] / l;
    // up = Y - (Y.f) f, normalised: its y is 1 - fy^2 over its length.
    let u = [-fy * f[0] / l, 1.0 - fy * fy, -fy * f[2] / l];
    let ul = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
    (if ul > 0.0 { u[1] / ul } else { 0.0 }, l)
}

/// Every value the Camera page can give, one slider at a time from the Original notch and from the
/// shipped default: each slider's every step, both ends among them, as the shell's framings.
fn page_values() -> Vec<(String, Framing, Framing)> {
    let mut out = Vec::new();
    // (fov, distance, height, plane distance, plane height), as `menu::CameraView`.
    let original = [0, 100, 100, 100, 100];
    let default = [4, 115, 110, 120, 115];
    for base in [original, default] {
        for slider in 0..5 {
            let (lo, hi, step) = if slider == 0 { (-10, 20, 1) } else { (50, 200, 5) };
            let mut v = lo;
            while v <= hi {
                let mut n = base;
                n[slider] = v;
                let f = |d: i32, h: i32| Framing { fov: n[0] as f32, distance: d as f32 / 100.0, height: h as f32 / 100.0 };
                out.push((format!("{n:?}"), f(n[1], n[2]), f(n[3], n[4])));
                v += step;
            }
        }
    }
    out
}

/// The most the eye's reach to the look-at may move in a tick, as a share of the last: the lags
/// chasing a jump from one end of a slider to the other move it about a fifth.
const REACH_SWING: f32 = 0.25;
/// The most the field of view may move in a tick, as a share of the last.
const FOV_SWING: f32 = 0.05;

/// Every check one ridden frame makes of the camera on `framed` presets: upright, its distance and
/// field of view inside the preset's limits, and against the last frame no swing past a share.
fn steady(what: &str, cam: &Race, v: &View, last: Option<&View>, frame: usize) {
    let p = cam.cam.preset();
    let (up, len) = up_y(v);
    assert!(up > 0.05, "{what} frame {frame}: the view is not upright (up y {up})");
    let d = cam.cam.distance.0;
    let floor = p.min_following_distance.0.max(4.5 * NEAR_CLIP);
    assert!(d >= floor - 1e-3 && d <= p.max_following_distance.0 + 1e-3, "{what} frame {frame}: distance {d} out of {floor}..{}", p.max_following_distance.0);
    let fov = v.fov.0;
    assert!(fov > 0.0 && fov < 180.0, "{what} frame {frame}: field of view {fov}");
    assert!(fov >= p.min_fov.0 - 1e-3 && fov <= p.max_fov.0 + 1e-3, "{what} frame {frame}: field of view {fov} out of {}..{}", p.min_fov.0, p.max_fov.0);
    if let Some(l) = last {
        let (_, was) = up_y(l);
        assert!((len - was).abs() <= REACH_SWING * was, "{what} frame {frame}: the eye's reach swung from {was} to {len}");
        assert!((fov - l.fov.0).abs() <= FOV_SWING * l.fov.0, "{what} frame {frame}: the field of view swung from {} to {fov}", l.fov.0);
    }
}

/// P119: every value the Camera page gives — each slider's every step and both ends, from the
/// Original notch and from the default — rides a second at speed on a steady, upright camera, on
/// the bike's and the plane's presets, a full screen's and the split's.
#[test]
fn every_camera_page_value_rides_steady() {
    let curve = CurveTable::build();
    let cams = cameras();
    let values = page_values();
    assert_eq!(values.len(), 2 * (31 + 4 * 31));
    for (file, presets, at) in &cams {
        let plane = file.contains("biplane");
        for (name, bike, flight) in &values {
            let framing = if plane { *flight } else { *bike };
            let mut cam = shell_camera(framed(presets, framing), *at);
            let mut ease = FovEase::default();
            let mut last = None;
            for t in 0..60 {
                let v = tick(&mut cam, &mut ease, &curve, t, file.ends_with('h'));
                if t >= 30 {
                    steady(&format!("{file} at {name}"), &cam, &v, last.as_ref(), t);
                }
                last = Some(v);
            }
        }
    }
}

/// What the camera's own lag on the field of view does with a target that moves (P119's haywire):
/// its time constant is zero, its blend zero over zero, which the PS2's divide makes the largest
/// float — the field of view goes to the largest float, then to zero, and on.
#[test]
fn the_cameras_own_fov_lag_cannot_follow_a_change() {
    let curve = CurveTable::build();
    let presets = presets();
    let start = Preset::find(&presets, "Race Cam 1").expect("Race Cam 1");
    let mut cam = shell_camera(presets.clone(), start);
    let mut ease = FovEase::default();
    for t in 0..30 {
        tick(&mut cam, &mut ease, &curve, t, false);
    }
    assert_eq!(cam.cam.fov_filter.tau.0, 0.0, "the fov lag has a time constant now: P119's ease may be moot");
    // The reload as P82 shipped it: the presets swapped, the preset set again, and nothing else.
    cam.cam.presets = framed(&presets, Framing { fov: 1.0, ..Framing::ORIGINAL });
    cam.cam.set_preset(start);
    let a = tick(&mut cam, &mut ease, &curve, 30, false).fov.0;
    let b = tick(&mut cam, &mut ease, &curve, 31, false).fov.0;
    assert_eq!((a, b), (f32::MAX, 0.0), "the old reload's field of view");
}

/// P119: a change mid-ride, taken the way the shell takes one (one [`reframe`] for each change), eases
/// over: the camera stays upright and inside its limits every frame, nothing swings past a share
/// from one frame to the next, and a second later it is where a camera started on the new framing
/// is. Every slider walked a step a tick to its ends and back, then Default and Original as the
/// page's two rows jump to them — on the bike and the plane, a full screen and the split.
#[test]
fn a_change_mid_ride_eases_over() {
    let curve = CurveTable::build();
    for (file, presets, at) in cameras() {
        let plane = file.contains("biplane");
        let short = file.ends_with('h');
        // The walk: [fov, distance, height] of this machine's sliders, a tick each.
        let mut walk: Vec<[i32; 3]> = Vec::new();
        let mut v = [0, 100, 100];
        for (s, (lo, hi, step)) in [(-10, 20, 1), (50, 200, 5), (50, 200, 5)].into_iter().enumerate() {
            let home = v[s];
            while v[s] < hi { v[s] += step; walk.push(v); }
            while v[s] > lo { v[s] -= step; walk.push(v); }
            while v[s] < home { v[s] += step; walk.push(v); }
        }
        let default = if plane { [4, 120, 115] } else { [4, 115, 110] };
        walk.extend([default, [0, 100, 100], [20, 200, 50], [-10, 50, 200], [0, 100, 100]]);
        let to = |v: [i32; 3]| Framing { fov: v[0] as f32, distance: v[1] as f32 / 100.0, height: v[2] as f32 / 100.0 };

        let mut cam = shell_camera(presets.clone(), at);
        let mut ease = FovEase::default();
        let mut last: Option<View> = None;
        let mut t = 0;
        let mut ride = |cam: &mut Race, ease: &mut FovEase, t: &mut usize, last: &mut Option<View>, what: &str| {
            let v = tick(cam, ease, &curve, *t, short);
            if *t >= 30 {
                steady(what, cam, &v, last.as_ref(), *t);
            }
            *last = Some(v);
            *t += 1;
        };
        for _ in 0..30 {
            ride(&mut cam, &mut ease, &mut t, &mut last, &file);
        }
        // A tick a step, then two ticks on each of the jumps at the end.
        for (k, step) in walk.iter().enumerate() {
            reframe(&mut cam, &framed(&presets, to(*step)), &mut ease);
            let what = format!("{file} stepping to {step:?}");
            ride(&mut cam, &mut ease, &mut t, &mut last, &what);
            if k + 5 >= walk.len() {
                ride(&mut cam, &mut ease, &mut t, &mut last, &what);
            }
        }
        // Settled a second on, on a framing away from the original: the same view as a camera that
        // started on it, and an extra reload with nothing changed moves nothing.
        let end = [20, 200, 50];
        reframe(&mut cam, &framed(&presets, to(end)), &mut ease);
        for _ in 0..60 {
            ride(&mut cam, &mut ease, &mut t, &mut last, &format!("{file} settling"));
        }
        assert_eq!(ease.offset, 0.0, "{file}: the ease has not settled");
        let mut fresh = shell_camera(framed(&presets, to(end)), at);
        let mut none = FovEase::default();
        let mut want = None;
        for u in t - 60..t {
            want = Some(tick(&mut fresh, &mut none, &curve, u, short));
        }
        let (got, want) = (last.unwrap(), want.unwrap());
        assert_eq!(got.fov.0, want.fov.0, "{file}: the field of view settled elsewhere");
        let gap = ((got.position.x() - want.position.x()).powi(2)
            + (got.position.y() - want.position.y()).powi(2)
            + (got.position.z() - want.position.z()).powi(2))
        .sqrt();
        assert!(gap < 0.05, "{file}: the eye settled {gap} ft from a camera started on the framing");
        let before = bits(&tick(&mut Race { cam: cam.cam.clone() }, &mut ease.clone(), &curve, t, short));
        reframe(&mut cam, &framed(&presets, to(end)), &mut ease);
        let again = bits(&tick(&mut cam, &mut ease, &curve, t, short));
        assert_eq!(before, again, "{file}: a reload with nothing changed moved the camera");
    }
}
