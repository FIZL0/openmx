//! P384: the first-person camera, the disc's own out of `appstart\fppreset.ini`.
//!
//! The camera looks along the bike's forward axis from the rider's Head node, the look swung into a
//! turn by the yaw rate and lagged, the up rolled by a share of the bike's roll, the eye kept over the
//! ground; the numbers are the file's. The camera button walks onto it after the bike's last cycling
//! preset and off it onto the first.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use chase::{FirstPersonCamera, FpAdjust};
use game_sim::camera::{CurveTable, FirstPerson, FpPreset, Nothing, Onboard, Preset, Race, Surface};
use game_sim::ride::Ride;
use ps2_float::{F, Vf};

const DT: F = F(1.0 / 30.0);

/// The disc's `appstart\fppreset.ini` out of free06's second archive, as the shell finds it; `None`
/// without the disc.
fn disc_file() -> Option<Vec<u8>> {
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
    let ini = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("fppreset.ini"))?;
    res.read(&bytes, ini).ok()
}

/// The file as the disc ships it, in every track's archive alike (journal P384): the camera without
/// the disc.
const STAND_IN: &str = "[General]\nCameraUnrollFilter = 0.01\nObjectRollFilter = 0.01\nYawRateFilter = 0.1\n\
[1Player]\nFOV = 85.0\nFOVWideScreen = 90.0\nOffsetFromBodyPoint = 0.0, 4.5, 0.0\nLookFilter = 0.05, 0.19, 0.05\n\
LookSwing = 0.66\nMaxRoll = 45.0\nObjectRollScale = 0.5\nCameraUnrollScale = 1.0\n\
[2hPlayer]\nFOV = 80.0\n[2vPlayer]\nFOV = 60.0\n[3Player]\nFOV = 80.0\n";

fn file() -> Vec<u8> {
    disc_file().unwrap_or_else(|| {
        eprintln!("no disc: the stand-in fppreset.ini");
        STAND_IN.as_bytes().to_vec()
    })
}

fn close(a: f32, b: f32, by: f32) -> bool {
    (a - b).abs() <= by
}

/// A stock bike standing at `at`, facing +z.
fn bike(at: [f32; 3]) -> Ride {
    let curve = CurveTable::build();
    let p = game_sim::stock::phy(&curve);
    Ride::new(&p, game_sim::stock::MOUNTS, Vf::new(at[0], at[1], at[2], 1.0), Vf::new(0.0, 0.0, 1.0, 0.0))
}

fn camera() -> FirstPersonCamera {
    let mut fp = FirstPersonCamera::default();
    fp.ensure(Some(&file()), 0, false, true);
    fp
}

#[test]
fn fppreset_reads_the_discs_numbers() {
    let Some(bytes) = disc_file() else {
        eprintln!("no disc: skipped");
        return;
    };
    let p = FpPreset::read_bytes(&bytes);
    // The game's own number reader, which lands a hair off the nearest float now and then.
    let same = |got: &[f32], want: &[f32]| got.len() == want.len() && got.iter().zip(want).all(|(g, w)| close(*g, *w, 1e-6));
    assert!(same(&[p.camera_unroll_filter.0, p.object_roll_filter.0, p.yaw_rate_filter.0], &[0.01, 0.01, 0.1]));
    let one = &p.views[0];
    assert!(same(&[one.fov.0, one.fov_wide.0], &[85.0, 90.0]));
    assert!(same(&[one.offset.x(), one.offset.y(), one.offset.z()], &[0.0, 4.5, 0.0]));
    assert!(same(&[one.look_filter.x(), one.look_filter.y(), one.look_filter.z()], &[0.05, 0.19, 0.05]));
    assert!(same(&[one.look_swing.0, one.object_roll_scale.0, one.camera_unroll_scale.0], &[0.66, 0.5, 1.0]));
    assert!(close(one.max_roll.0, 45f32.to_radians(), 1e-6));
    assert!(same(&p.views.map(|v| v.fov.0), &[85.0, 80.0, 60.0, 80.0]));
}

#[test]
fn the_eye_follows_the_riders_head_node() {
    let mut fp = camera();
    assert!(fp.on, "--camera \"First Person\" starts on it");
    let curve = CurveTable::build();
    let ride = bike([10.0, 2.0, 30.0]);
    let at = ride.position();
    // The head swaying over a few ticks, as the posed body moves it.
    for (k, head) in [[0.0, 5.1, 0.4], [0.2, 5.0, 0.5], [-0.3, 4.8, 0.2]].into_iter().enumerate() {
        let head = Vf::new(at.x() + head[0], at.y() + head[1], at.z() + head[2], 1.0);
        let (view, up) = fp.frame(&ride, Some(head), FpAdjust::ORIGINAL, DT, &curve, &mut Nothing).expect("on");
        assert_eq!([view.position.x(), view.position.y(), view.position.z()], [head.x(), head.y(), head.z()], "tick {k}: the eye is the head");
        let look = [view.look_at.x() - head.x(), view.look_at.y() - head.y(), view.look_at.z() - head.z()];
        assert!(close(look[0], 0.0, 1e-3) && close(look[1], 0.0, 1e-3) && close(look[2], 1.0, 1e-3), "tick {k}: along the bike, {look:?}");
        assert!(close(up.y(), 1.0, 1e-3), "a level bike, a level camera: {up:?}");
        assert_eq!(view.fov.0, 85.0, "the file's 1Player FOV");
    }
    // No posed body: the eye where a still head sits, OffsetFromBodyPoint over the bike's point.
    let (view, _) = fp.frame(&ride, None, FpAdjust::ORIGINAL, DT, &curve, &mut Nothing).unwrap();
    assert!(close(view.position.y(), at.y() + 4.5, 1e-4) && close(view.position.z(), at.z(), 1e-4), "{:?}", view.position);
    // MXA's adjust: no bob pins the eye there whatever the head does, and the trim moves it.
    let head = Vf::new(at.x() + 0.3, at.y() + 5.0, at.z() + 0.4, 1.0);
    let adjust = FpAdjust { bob: 0.0, up: 0.5, forward: 1.0, fov: 5.0 };
    let (view, _) = fp.frame(&ride, Some(head), adjust, DT, &curve, &mut Nothing).unwrap();
    assert!(close(view.position.x(), at.x(), 1e-4), "{:?}", view.position);
    assert!(close(view.position.y(), at.y() + 5.0, 1e-4) && close(view.position.z(), at.z() + 1.0, 1e-4), "{:?}", view.position);
    assert_eq!(view.fov.0, 90.0);
    // The Camera page's sliders (P389) hand the same four numbers over in the same order; the menu's
    // side is tests/menu.rs's `menu_sliders_set_fp_adjust_and_save_it`.
    assert_eq!(FpAdjust::from((0.0, 0.5, 1.0, 5.0)), adjust);
    assert_eq!(FpAdjust::from((1.0, 0.0, 0.0, 0.0)), FpAdjust::ORIGINAL);
    // Off the screen, it draws nothing.
    fp.on = false;
    assert!(fp.frame(&ride, Some(head), FpAdjust::ORIGINAL, DT, &curve, &mut Nothing).is_none());
}

#[test]
fn the_look_swings_into_a_turn_with_the_files_lags() {
    let mut fp = camera();
    let curve = CurveTable::build();
    let mut ride = bike([0.0, 2.0, 0.0]);
    let head = Some(Vf::new(0.0, 7.0, 0.0, 1.0));
    fp.frame(&ride, head, FpAdjust::ORIGINAL, DT, &curve, &mut Nothing);
    ride.chassis.world_ang_vel = Vf::new(0.0, 1.0, 0.0, 0.0);
    let turned = |fp: &mut FirstPersonCamera, ride: &Ride| {
        let (v, _) = fp.frame(ride, head, FpAdjust::ORIGINAL, DT, &curve, &mut Nothing).unwrap();
        (v.look_at.x() - v.position.x()).atan2(v.look_at.z() - v.position.z()).abs()
    };
    // A yaw rate of one turns the look by LookSwing × 0.11 radians, lagged in on YawRateFilter and
    // LookFilter, not snapped.
    let target = 0.66 * 0.11;
    let first = turned(&mut fp, &ride);
    assert!(first > 0.0 && first < target * 0.5, "lagged: {first} of {target}");
    let mut last = first;
    for _ in 0..120 {
        last = turned(&mut fp, &ride);
    }
    assert!(close(last, target, 2e-3), "settles on {target}: {last}");
}

#[test]
fn the_camera_rolls_by_the_files_share_of_the_bikes_roll() {
    let curve = CurveTable::build();
    let mut cam = FirstPerson::new(FpPreset::read_bytes(&file()), 0, false);
    let r = 0.4f32;
    let on = Onboard {
        eye: Vf::new(0.0, 5.0, 0.0, 1.0),
        forward: Vf::new(0.0, 0.0, 1.0, 0.0),
        up: Vf::new(r.sin(), r.cos(), 0.0, 0.0),
        roll: FirstPerson::roll_of(Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(r.sin(), r.cos(), 0.0, 0.0)),
        ..Onboard::default()
    };
    assert!(close(on.roll.0.abs(), r, 1e-4), "the bike's roll: {}", on.roll.0);
    let mut up = Vf::ZERO;
    for _ in 0..30 {
        up = cam.update(DT, &on, &curve, &mut Nothing).1;
    }
    // ObjectRollScale 0.5: half the bike's roll, the same way.
    assert!(close(up.x(), (0.5 * r).sin(), 2e-3) && close(up.y(), (0.5 * r).cos(), 2e-3), "{up:?}");
    // Tumbling, no roll.
    let (_, up) = cam.update(DT, &Onboard { tumbling: true, ..on }, &curve, &mut Nothing);
    assert!(close(up.x(), 0.0, 1e-3), "{up:?}");
}

/// The tilt of the level horizon ahead of a view, in radians as it is drawn into a 4:3 frame:
/// positive where its +x end, off to the side, stands higher in the picture.
fn horizon_tilt(camera: &game_render::world::Camera) -> f32 {
    let (w, h) = (640, 480);
    let vp = camera.view_projection(w, h);
    let forward = camera.at.sub(camera.eye).norm();
    let ahead = camera.eye.add(game_render::v3(forward.x, 0.0, forward.z).norm().scale(1000.0));
    let side = game_render::v3(forward.z, 0.0, -forward.x).norm().scale(100.0);
    let pixel = |p: game_render::V3| {
        let c = vp.clip(p);
        [c[0] / c[3] * w as f32 / 2.0, c[1] / c[3] * h as f32 / 2.0]
    };
    let (a, b) = (pixel(ahead.sub(side)), pixel(ahead.add(side)));
    // The +x end's height over the other, along the horizon's run across the frame.
    let (run, rise) = (b[0] - a[0], b[1] - a[1]);
    let rise = if side.x >= 0.0 { rise } else { -rise };
    rise.atan2(run.abs())
}

#[test]
fn a_rolled_first_person_up_tilts_the_drawn_horizon() {
    let curve = CurveTable::build();
    let level = Vf::new(0.0, 0.0, 1.0, 0.0);
    // The camera as the shell draws it (main.rs): the view's eye and look, its own up.
    let drawn = |r: f32| {
        let mut cam = FirstPerson::new(FpPreset::read_bytes(&file()), 0, false);
        let lean = Vf::new(r.sin(), r.cos(), 0.0, 0.0);
        let on = Onboard { eye: Vf::new(0.0, 5.0, 0.0, 1.0), forward: level, up: lean, roll: FirstPerson::roll_of(level, lean), ..Onboard::default() };
        let (mut view, mut up) = cam.update(DT, &on, &curve, &mut Nothing);
        for _ in 0..30 {
            (view, up) = cam.update(DT, &on, &curve, &mut Nothing);
        }
        let v = |p: Vf| game_render::v3(p.x(), p.y(), p.z());
        game_render::world::Camera { eye: v(view.position), at: v(view.look_at), fov: 1.0, near: 0.1, far: 4000.0, up: Some(v(up)) }
    };
    let r = 0.4f32;
    // A level bike, a level horizon, as the world's up draws it.
    let flat = drawn(0.0);
    assert!(horizon_tilt(&flat).abs() < 1e-4, "{}", horizon_tilt(&flat));
    assert!(horizon_tilt(&game_render::world::Camera { up: None, ..flat }).abs() < 1e-4);
    // Leaning towards +x, the camera rolls by ObjectRollScale (0.5) of it: the horizon's +x end
    // stands higher in the picture by that angle, and the other lean tilts it the other way.
    let leaning = drawn(r);
    assert!(close(horizon_tilt(&leaning), 0.5 * r, 3e-3), "{} of {}", horizon_tilt(&leaning), 0.5 * r);
    assert!(close(horizon_tilt(&drawn(-r)), -0.5 * r, 3e-3));
    // Without its up, the same view draws level: what the screen showed before V137.
    assert!(horizon_tilt(&game_render::world::Camera { up: None, ..leaning }).abs() < 1e-4);
}

/// Ground 10 ft up everywhere.
struct Raised;
impl Surface for Raised {
    fn on_surface(&mut self, p: Vf) -> Vf {
        Vf::new(p.x(), 10.0, p.z(), p.w())
    }
}

#[test]
fn the_eye_stays_over_the_ground() {
    let curve = CurveTable::build();
    let mut cam = FirstPerson::new(FpPreset::read_bytes(&file()), 0, false);
    let on = Onboard { eye: Vf::new(0.0, 5.0, 0.0, 1.0), forward: Vf::new(0.0, 0.0, 1.0, 0.0), up: Vf::new(0.0, 1.0, 0.0, 0.0), ..Onboard::default() };
    let (view, _) = cam.update(DT, &on, &curve, &mut Raised);
    assert_eq!(view.position.y(), 11.5);
}

#[test]
fn the_camera_button_walks_onto_it_after_the_last_preset() {
    let preset = |name: &str, cycle: bool| Preset { name: name.into(), cycle_thru_this_preset: cycle, ..Preset::default() };
    let mut race = Race::new(vec![preset("A", true), preset("B", true), preset("Back", false)], F(5.0));
    let mut fp = FirstPersonCamera::default();
    fp.ensure(Some(&file()), 0, false, false);
    assert!(!fp.on);
    assert!(!fp.next(&mut race.cam));
    assert_eq!(race.cam.preset, 1);
    assert!(fp.next(&mut race.cam), "after B, the first-person camera");
    assert_eq!(race.cam.preset, 1, "the follow camera stays on B underneath");
    assert!(!fp.next(&mut race.cam));
    assert_eq!(race.cam.preset, 0, "and on round to A");
    // Without the file there is none to walk onto.
    let mut none = FirstPersonCamera::default();
    none.ensure(None, 0, false, false);
    none.next(&mut race.cam);
    none.next(&mut race.cam);
    assert_eq!((none.on, race.cam.preset), (false, 0));
}

#[test]
fn a_split_screen_reads_its_own_section() {
    let mut fp = FirstPersonCamera::default();
    fp.ensure(Some(&file()), 2, false, true);
    assert_eq!(fp.cam.unwrap().fov().0, 60.0, "2vPlayer");
    fp.ensure(Some(&file()), 0, true, true);
    assert_eq!(fp.cam.unwrap().fov().0, 90.0, "1Player's FOVWideScreen");
    // The look lags take the new section's LookFilter on a switch, as the original's viewport select.
    let mut preset = FpPreset::read_bytes(&file());
    preset.views[1].look_filter = Vf::new(0.3, 0.4, 0.5, 0.0);
    let mut cam = FirstPerson::new(preset, 0, false);
    cam.set_viewport(1);
    assert_eq!(cam.look.map(|l| l.tau.0), [0.3, 0.4, 0.5]);
}

#[test]
fn a_bike_with_one_preset_still_walks_onto_it() {
    let one = Preset { name: "A".into(), cycle_thru_this_preset: true, ..Preset::default() };
    let mut race = Race::new(vec![one], F(5.0));
    let mut fp = FirstPersonCamera::default();
    fp.ensure(Some(&file()), 0, false, false);
    assert!(fp.next(&mut race.cam));
    assert!(!fp.next(&mut race.cam));
    assert_eq!(race.cam.preset, 0);
}
