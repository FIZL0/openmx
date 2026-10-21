//! V29: the biplane's camera, against the original's own behind the human's biplane flight.
//!
//! The plane's camera is the race camera the bike has, built by the same constructor and handed
//! `AppStart\BiplaneCPresets.ini` instead of the track's presets (`research/oracle.txt`, V29). Its
//! frame was ported and proved by S24; what is the plane's own is the block it reads, which the plane
//! fills once a tick. `watch/biplane-camera.toml` captured that block, the camera's state and the
//! viewport it wrote on every tick of `biplane.p2m2`; this replays the captured block through the
//! port's camera and compares the eye and the view's direction with the original's.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use std::collections::HashMap;
use std::path::PathBuf;

use game_sim::camera::{CurveTable, Preset, Race, Ridden, Surface, Switches, Target};
use ps2_float::{F, Vf};

/// The shell's constants for the plane's camera (`main.rs`): the race camera's reverse threshold,
/// the speed reference, the near clip.
const REVERSE_SPEED: f32 = 5.0;
const SPEED_REFERENCE: f32 = 117.332_8;
const NEAR_CLIP: f32 = 2.4;

fn context() -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context"),
    }
}

/// The plane's camera file off the disc, `None` without it.
fn plane_presets() -> Option<Vec<Preset>> {
    use game_sim::archive::{Psr, Res};
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(context().join("iso/MX.PSR")).ok()?;
    let mut front = vec![0u8; 64 << 10];
    file.read_exact(&mut front).ok()?;
    let psr = Psr::parse(&front).ok()?;
    let e = psr.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("free06_2.res"))?.clone();
    file.seek(SeekFrom::Start(e.at)).ok()?;
    let mut bytes = vec![0u8; e.size as usize];
    file.read_exact(&mut bytes).ok()?;
    let res = Res::parse(&bytes).ok()?;
    let ini = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("biplanecpresets.ini"))?;
    let presets = Preset::read_bytes(&res.read(&bytes, ini).ok()?, "");
    (!presets.is_empty()).then_some(presets)
}

/// The capture's rows, by column name. `None` without it.
fn capture() -> Option<Vec<HashMap<String, String>>> {
    let text = std::fs::read_to_string(context().join("capture/v29/biplane-camera.csv")).ok()?;
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(
        lines
            .map(|l| head.iter().cloned().zip(l.split(',').map(str::to_string)).collect())
            .collect(),
    )
}

fn num(r: &HashMap<String, String>, k: &str) -> f32 {
    r[k].parse().unwrap()
}

fn quad(r: &HashMap<String, String>, n: &str) -> Vf {
    Vf::new(num(r, &format!("{n}.x")), num(r, &format!("{n}.y")), num(r, &format!("{n}.z")), num(r, &format!("{n}.w")))
}

/// The airborne flag is a word: the recorder wrote its bits as a float.
fn flag(r: &HashMap<String, String>) -> bool {
    num(r, "tgt_flag_speed.x") != 0.0
}

fn dist(a: Vf, b: Vf) -> f32 {
    ((a.x() - b.x()).powi(2) + (a.y() - b.y()).powi(2) + (a.z() - b.z()).powi(2)).sqrt()
}

/// The ground under the probe: nowhere near. The capture's ground lift is zero on every row.
struct Below;

impl Surface for Below {
    fn on_surface(&mut self, point: Vf) -> Vf {
        Vf::new(point.x(), -1.0e6, point.z(), point.w())
    }
}

/// Row `i`'s target block, as the capture read it.
fn block(r: &HashMap<String, String>) -> Ridden {
    let tail = quad(r, "tgt_tail");
    Ridden {
        target: Target {
            body_point: quad(r, "tgt_point"),
            forward: quad(r, "tgt_fwd"),
            up: quad(r, "tgt_up"),
            vel: quad(r, "tgt_vel"),
            lean: F(tail.y()),
        },
        airborne: flag(r),
        speed: F(num(r, "tgt_flag_speed.y")),
    }
}

/// The angle between two directions, radians.
fn angle(a: Vf, b: Vf) -> f32 {
    let dot = a.x() * b.x() + a.y() * b.y() + a.z() * b.z();
    let len = |v: Vf| (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt();
    (dot / (len(a) * len(b))).clamp(-1.0, 1.0).acos()
}

/// Ticks the camera's lags take to forget a block the capture read wrong.
const SETTLE: usize = 120;

/// The capture's block, replayed through the port's camera on Flight Cam 1, puts the eye where the
/// original's viewport has it and points it the same way, through the climb, the loop, the crash and
/// both of the plane's resets.
///
/// The capture is polled, not synced, and tears (`frame-counter-trigger-tears-captures`): some rows
/// read the viewport a tick late, which the comparison takes as either of the port's last two eyes;
/// some read the plane's block a tick early, which feeds the port a wrong block, so the rows a lag
/// would still remember it by are not compared. The rest are held to a hundredth of a foot.
#[test]
fn the_plane_camera_films_the_flight_as_the_original() {
    let (Some(presets), Some(rows)) = (plane_presets(), capture()) else {
        eprintln!("skipped: no disc or no context/capture/v29/biplane-camera.csv");
        return;
    };
    let curve = CurveTable::build();
    let mut cam = Race::new(presets, F(REVERSE_SPEED));
    cam.cam.set_preset(Preset::find(&cam.cam.presets, "Flight Cam 1").unwrap());
    // The state was saved with the camera already behind the plane, at rest: a snap is where it is.
    cam.cam.snap = true;
    cam.cam.speed_reference = F(SPEED_REFERENCE);
    let (mut compared, mut resets, mut worst, mut turned) = (0, 0, (0.0f32, 0usize), 0.0f32);
    let mut last: Option<Ridden> = None;
    let mut before = Vf::new(0.0, 0.0, 0.0, 1.0);
    let mut settled_at = SETTLE / 4;
    for (i, r) in rows.iter().enumerate() {
        let rider = block(r);
        let point = rider.target.body_point;
        if let Some(p) = &last {
            let was = p.target.body_point;
            // The plane's reset puts it back at its spawn, and snaps its camera there.
            if dist(was, point) > 50.0 {
                cam.cam.snap = true;
                resets += 1;
            }
            // A block read before the plane wrote it: last tick's point, while it is moving.
            if was == point && p.speed.0 > 1.0 {
                settled_at = i + SETTLE;
            }
        }
        let view = cam.update(
            F(num(r, "dt")),
            &rider,
            F(NEAR_CLIP),
            &curve,
            Switches { no_look_ahead: false, short_viewport: false },
            &mut Below,
        );
        last = Some(rider);
        let eye = quad(r, "vp_eye");
        let off = dist(view.position, eye).min(dist(before, eye));
        let late = dist(before, eye) < dist(view.position, eye);
        before = view.position;
        if i < settled_at {
            continue;
        }
        compared += 1;
        if off > worst.0 {
            worst = (off, i);
        }
        if !late {
            let dir = view.look_at.sub(view.position);
            turned = turned.max(angle(dir, quad(r, "vp_dir")));
        }
    }
    assert_eq!(resets, 2, "the crash's respawn and the reset after it");
    assert!(compared > 1400, "{compared} of {} rows compared", rows.len());
    assert!(worst.0 < 0.01, "the eye is {} ft off at row {}", worst.0, worst.1);
    assert!(turned < 1.0e-3, "the view is turned {turned} rad off");
}

/// The plane's block is its flight's as the original's writer fills it: the centre of mass where the
/// last substep began (not the drawn origin, nor where the body ended), the measure's rows, the
/// body's velocity, the measure's world yaw rate for the lean, "no wheel down", and the airspeed
/// floored at zero; every `w` 1.
#[test]
fn the_plane_block_is_the_flights() {
    let mut f = game_sim::flight::Flight::default();
    f.body.last_position = Vf::new(10.0, 20.0, 30.0, 1.0);
    f.body.position = Vf::new(11.0, 21.0, 31.0, 1.0);
    f.body.velocity = Vf::new(30.0, -3.0, 0.5, 0.0);
    f.measure.forward = Vf::new(0.0, 0.6, -0.8, 0.0);
    f.measure.up = Vf::new(0.0, 0.8, 0.6, 0.0);
    f.measure.world_rates = Vf::new(0.1, 0.25, -0.3, 0.0);
    f.measure.grounded = false;
    f.measure.airspeed = F(42.0);
    let b = chase::flown(&f);
    assert_eq!(b.target.body_point, Vf::new(10.0, 20.0, 30.0, 1.0));
    assert_eq!(b.target.forward, Vf::new(0.0, 0.6, -0.8, 1.0));
    assert_eq!(b.target.up, Vf::new(0.0, 0.8, 0.6, 1.0));
    assert_eq!(b.target.vel, Vf::new(30.0, -3.0, 0.5, 1.0));
    assert_eq!(b.target.lean, F(0.25));
    assert!(b.airborne);
    assert_eq!(b.speed, F(42.0));

    f.measure.grounded = true;
    f.measure.airspeed = F(-2.0);
    let b = chase::flown(&f);
    assert!(!b.airborne);
    assert_eq!(b.speed, F::ZERO);
}
