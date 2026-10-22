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
