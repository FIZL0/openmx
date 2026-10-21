//! V31: the wreck camera, as the shell films a wreck through `src/chase.rs`.
//!
//! Three things the user asked for: while the rider is down the camera's target is the rider's body,
//! not the bike; it sits at the decompilation's distance; and it hands the screen back to the follow
//! camera, on the bike, once the rider is back on. The numbers are the original's (journal
//! `wreck-camera`, `research/oracle.txt` V31): 13 ft at the nearest, one and a half of that as the
//! spread of a lone target, and the distance that fits a spread in the field of view less 20°.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use chase::{WreckCamera, WreckTargets};
use game_sim::camera::{CurveTable, Preset, Race, Ridden, Surface, Switches, Target};
use game_sim::ride::Crash;
use game_sim::trick::Wreck;
use ps2_float::{F, Vf};

/// The shell's frame.
const DT: f32 = 1.0 / 60.0;

/// Flat ground at `y = 0`.
struct Flat;

impl Surface for Flat {
    fn on_surface(&mut self, point: Vf) -> Vf {
        Vf::new(point.x(), 0.0, point.z(), point.w())
    }
}

fn v(x: f32, y: f32, z: f32) -> Vf {
    Vf::new(x, y, z, 1.0)
}

fn dist(a: Vf, b: Vf) -> f32 {
    ((a.x() - b.x()).powi(2) + (a.y() - b.y()).powi(2) + (a.z() - b.z()).powi(2)).sqrt()
}

/// A wreck with the bike's wreck body at `at` and the rider's point at `rider`.
fn crash(rider: Vf, at: Vf) -> Crash {
    let mut bike = game_sim::wreck_bike::WreckBike::default();
    bike.body.velocity = v(0.0, 0.0, 30.0);
    Crash {
        reason: Wreck::THROWN,
        hit: None,
        throw: Default::default(),
        clock: F::ZERO,
        holding: false,
        throwing: F::ZERO,
        bike,
        origin: at,
        rider,
        rider_vel: v(0.0, 0.0, 0.0),
        landed: true,
        ground: F::ZERO,
        resting: false,
        facing: v(0.0, 0.0, 1.0),
    }
}

/// A follow camera on one plain preset, as a track's file would give it, at `fov` degrees.
fn follow_at(fov: f32) -> Race {
    let lags = Vf::new(0.2, 0.2, 0.2, 0.0);
    let preset = Preset {
        name: "Race Cam 1".into(),
        look_filters: lags,
        pos_filters: lags,
        offset_from_body_point: v(0.0, 3.0, 0.0),
        angle_x: F(0.2),
        following_distance: F(15.0),
        fov: F(fov),
        filter_this_preset: true,
        cycle_thru_this_preset: true,
        min_following_distance: F(5.0),
        max_following_distance: F(50.0),
        min_fov: F(40.0),
        max_fov: F(100.0),
        ..Preset::default()
    };
    let mut race = Race::new(vec![preset], F(1.0));
    race.cam.set_preset(0);
    race.cam.snap = true;
    race
}

/// The follow camera at the wreck camera's own 80 degrees.
fn follow() -> Race {
    follow_at(80.0)
}

/// The bike riding along `z` at `at`.
fn ridden(at: Vf) -> Ridden {
    Ridden {
        target: Target {
            body_point: at,
            forward: Vf::new(0.0, 0.0, 1.0, 0.0),
            up: Vf::new(0.0, 1.0, 0.0, 0.0),
            vel: Vf::new(0.0, 0.0, 30.0, 0.0),
            lean: F::ZERO,
        },
        airborne: false,
        speed: F(30.0),
    }
}

/// The wreck camera's target is the rider's body: with no ragdoll, the crash's one rider point, not
/// the wreck body's place. Settled on a rider at rest, the look-at is the rider's point and the eye
/// is the decompilation's distance from it: a lone target (the bike past the 45 ft radius) spreads
/// one and a half minimum distances, 19.5 ft, and 19.5 ft across 60° is 16.89 ft. The rider lies 8 ft
/// above the ground here so that the eye's floor (three near planes, 7.2 ft) does not lift it.
#[test]
fn the_wreck_camera_frames_the_rider_at_the_original_distance() {
    let rider = v(10.0, 8.0, 100.0);
    let bike = v(80.0, 1.0, 140.0);
    let c = crash(rider, bike);
    let targets = chase::wreck_targets(&c, None);
    assert_eq!(targets.rider, rider, "the target is the rider's body");
    assert_eq!(targets.bike, bike);

    let curve = CurveTable::build();
    let mut race = follow();
    let mut cam = WreckCamera::new(false);
    for _ in 0..30 {
        race.update(F(DT), &ridden(v(10.0, 8.0, 90.0)), F(2.4), &curve, Switches::default(), &mut Flat);
    }
    assert!(cam.switch(Some(&targets), c.reason.0, &mut race.cam), "a wreck switches the screen to the wreck camera");
    let mut view = None;
    for _ in 0..60 * 20 {
        view = Some(cam.update(DT, &targets, false, 2.4, &curve, &mut Flat));
    }
    let view = view.unwrap();
    let want = WreckCamera::fitting_distance(13.0 * 1.5, 80.0);
    assert!((want - 16.887).abs() < 0.01, "19.5 ft at 60 degrees is {want}");
    assert!((cam.distance - want).abs() < 0.05, "settled at {} ft, not {want}", cam.distance);
    assert!(dist(view.look_at, rider) < 0.05, "the camera looks at {:?}, not the rider at {rider:?}", view.look_at);
    let off = dist(view.position, rider);
    assert!((off - want).abs() < 0.1, "the eye is {off} ft from the rider, not {want}");
    assert!(dist(view.position, bike) > 50.0, "the eye is with the rider, not the bike");
    // Eight degrees up, and side-on to the way the wreck went (+z), once the aim has settled.
    let up = (view.position.y() - rider.y()) / off;
    assert!((up - 0.1396f32.sin()).abs() < 0.01, "the eye is {up} up, not eight degrees");
    assert!((view.position.z() - rider.z()).abs() < 0.5, "the eye is not side-on: {:?}", view.position);
    assert!((view.fov.0 - 80.0).abs() < 0.01, "a still wreck keeps the base 80 degrees: {}", view.fov.0);
}

/// A bike within the radius pulls the point towards itself and the spread grows the distance: the
/// rider and the bike 30 ft apart is a point a third of the way to the bike's share and a distance
/// that fits 30 ft at 60°.
#[test]
fn a_near_bike_widens_the_shot() {
    let rider = v(0.0, 3.0, 0.0);
    let bike = v(30.0, 3.0, 0.0);
    let targets = WreckTargets { rider, bike, bike_vel: v(0.0, 0.0, 0.0) };
    let curve = CurveTable::build();
    let mut race = follow();
    race.update(F(DT), &ridden(v(0.0, 3.0, -15.0)), F(2.4), &curve, Switches::default(), &mut Flat);
    let mut cam = WreckCamera::new(false);
    cam.switch(Some(&targets), 7, &mut race.cam);
    for _ in 0..60 * 30 {
        cam.update(DT, &targets, false, 2.4, &curve, &mut Flat);
    }
    let (point, _, active) = cam.target(DT, &targets);
    assert_eq!(active, 2);
    let share = (1.0 - 1.0 / 3.0) * (1.0 - 30.0 / 45.0) / 2.0;
    assert!((point[0] - 30.0 * share).abs() < 1e-4, "the point is at x {}, not {}", point[0], 30.0 * share);
    let want = WreckCamera::fitting_distance(30.0, 80.0);
    assert!((cam.distance - want).abs() < 0.2, "{} ft for a 30 ft spread, not {want}", cam.distance);
}

/// Back on, the screen goes back to the follow camera, which snaps onto the bike rather than lagging
/// in from the wreck; and the wreck camera starts each wreck from the follow camera's view.
#[test]
fn back_on_the_follow_camera_has_the_bike() {
    let rider = v(0.0, 3.0, 0.0);
    let bike = v(0.0, 1.0, 10.0);
    let targets = WreckTargets { rider, bike, bike_vel: v(0.0, 0.0, 30.0) };
    let curve = CurveTable::build();
    let mut race = follow();
    let mut cam = WreckCamera::new(false);
    let mut riding = race.update(F(DT), &ridden(v(0.0, 3.0, -5.0)), F(2.4), &curve, Switches::default(), &mut Flat);
    for _ in 0..30 {
        riding = race.update(F(DT), &ridden(v(0.0, 3.0, -5.0)), F(2.4), &curve, Switches::default(), &mut Flat);
    }
    assert!(cam.switch(Some(&targets), 7, &mut race.cam));
    let first = cam.update(DT, &targets, false, 2.4, &curve, &mut Flat);
    assert!(
        dist(first.position, riding.position) < 1.5,
        "the wreck camera starts where the follow camera was: {:?} against {:?}",
        first.position,
        riding.position
    );
    for _ in 0..120 {
        assert!(cam.switch(Some(&targets), 7, &mut race.cam));
        cam.update(DT, &targets, false, 2.4, &curve, &mut Flat);
    }
    // The rider respawns at the start line, far from the wreck.
    let start = v(500.0, 3.0, 500.0);
    assert!(!cam.switch(None, 0, &mut race.cam), "back on, the follow camera has the screen");
    assert!(race.cam.snap, "and it snaps");
    let back = race.update(F(DT), &ridden(start), F(2.4), &curve, Switches::default(), &mut Flat);
    assert!(dist(back.look_at, start) < 10.0, "the follow camera looks at the bike: {:?}", back.look_at);
    assert!(dist(back.position, start) < 30.0, "and is behind it the same frame: {:?}", back.position);
    assert!(!cam.on);
    // The next wreck switches again.
    assert!(cam.switch(Some(&targets), 13, &mut race.cam));
    assert!((cam.eye[0].tau.0 - 0.2).abs() < 1e-6, "a cannonball's eye lags 0.2 s");
    assert!((cam.look[1].tau.0 - 0.23 * 0.75 * 0.9).abs() < 1e-6, "and its look-at's height 0.155 s");
}

/// Per player: two screens' wreck cameras are two cameras, each switched by its own rider.
#[test]
fn each_player_has_their_own_wreck_camera() {
    let curve = CurveTable::build();
    let (mut a, mut b) = (WreckCamera::new(false), WreckCamera::new(false));
    let (mut fa, mut fb) = (follow(), follow());
    let targets = WreckTargets { rider: v(0.0, 3.0, 0.0), bike: v(0.0, 1.0, 10.0), bike_vel: v(0.0, 0.0, 30.0) };
    fb.cam.snap = false;
    assert!(a.switch(Some(&targets), 7, &mut fa.cam));
    assert!(!b.switch(None, 0, &mut fb.cam));
    a.update(DT, &targets, true, 2.4, &curve, &mut Flat);
    assert!(a.on && !b.on);
    assert!(!fb.cam.snap, "the riding player's camera is left alone");
}

/// The field of view moves off the screen's as the wreck began, not a fixed one. From a follow camera
/// at 85 degrees (the original's screen A, live) the offset is walked from 85, not the constructor's 80,
/// and the shot never goes wider than the wreck camera's widest, 82. The field of view's lag is not
/// reseeded: it eases from where it was, as the original's went 79.0, 79.1, 79.3 towards 82.
#[test]
fn the_field_of_view_starts_from_the_screens() {
    let rider = v(10.0, 8.0, 100.0);
    let targets = WreckTargets { rider, bike: v(80.0, 1.0, 140.0), bike_vel: v(0.0, 0.0, 30.0) };
    let curve = CurveTable::build();
    let mut race = follow_at(85.0);
    for _ in 0..30 {
        race.update(F(DT), &ridden(v(10.0, 8.0, 90.0)), F(2.4), &curve, Switches::default(), &mut Flat);
    }
    let mut cam = WreckCamera::new(false);
    assert!(cam.switch(Some(&targets), 7, &mut race.cam));
    assert!((cam.base_fov - 85.0).abs() < 0.01, "the base is the follow camera's {}", cam.base_fov);
    let first = cam.update(DT, &targets, false, 2.4, &curve, &mut Flat).fov.0;
    assert!(first > 80.0 && first < 80.2, "the lag eases from its 80 towards the clamped 82: {first}");
    for _ in 0..60 * 20 {
        let fov = cam.update(DT, &targets, false, 2.4, &curve, &mut Flat).fov.0;
        assert!(fov <= 82.0 + 1e-4, "the field of view went past the widest: {fov}");
    }
}
