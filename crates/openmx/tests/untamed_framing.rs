//! MD256: the Camera page's framing on the Untamed quad camera (MD251). The player's fov, distance
//! and height are laid over both ends of each slow/fast preset as P82 lays them over MX Unleashed's,
//! in the same order as the stand-ins' camera's list, and a change eases the field of view over.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use chase::{FovEase, Framing, Own, seat_presets, untamed_framed, untamed_reframe, untamed_seat_presets};
use game_sim::camera::{Nothing, Ridden, Target, UntamedFollow, UntamedPreset, View};
use ps2_float::{F, Vf};

/// A slow/fast file in Untamed's layout: a race camera and a look-back one, the eye in front.
const PB: &str = "[4x3/1p/1]\nName = Race\nSlowAngleX = 12\nFastAngleX = 18\nSlowFollowDistance = 14\n\
FastFollowDistance = 22\nSlowFov = 60\nFastFov = 70\nSlowSpeed = 10\nFastSpeed = 60\n\
[4x3/1p/2]\nName = Back\nAngleX = 10\nSlowFollowDistance = -10\nFastFollowDistance = -10\nFOV = 0\nIsLookBackCam = 1\n";

fn presets() -> Vec<UntamedPreset> {
    chase::untamed_presets(Some(PB.as_bytes()))
}

fn standing() -> Ridden {
    Ridden {
        target: Target {
            body_point: Vf::new(100.0, 50.0, 200.0, 1.0),
            forward: Vf::new(0.0, 0.0, 1.0, 1.0),
            up: Vf::new(0.0, 1.0, 0.0, 1.0),
            vel: Vf::new(0.0, 0.0, 0.0, 1.0),
            lean: F::ZERO,
        },
        airborne: false,
        speed: F::ZERO,
    }
}

/// The snapped first frame on preset `at`: the goal, unfiltered.
fn shot(presets: Vec<UntamedPreset>, at: usize) -> View {
    let mut cam = UntamedFollow::new(presets);
    cam.set_preset(at);
    cam.snap = true;
    cam.update(F(1.0 / 60.0), &standing(), &mut Nothing)
}

/// The eye's legs off the target point (3 ft up the up row): behind it, and above it.
fn legs(v: &View) -> (f32, f32) {
    let r = standing().target.body_point;
    (v.position.z() - r.z(), v.position.y() - (r.y() + game_sim::camera::UNTAMED_TARGET_HEIGHT))
}

#[test]
fn the_original_notch_is_the_files_presets() {
    let disc = presets();
    assert_eq!(disc.len(), 2);
    let laid = untamed_seat_presets(&disc, Framing::ORIGINAL, &[], false);
    assert_eq!(laid, disc);
    for at in 0..2 {
        let (a, b) = (shot(disc.clone(), at), shot(laid.clone(), at));
        assert_eq!(a.position.x().to_bits(), b.position.x().to_bits());
        assert_eq!(a.fov.0.to_bits(), b.fov.0.to_bits());
    }
}

#[test]
fn distance_height_and_fov_reach_both_ends() {
    let disc = presets();
    let f = Framing { fov: 5.0, distance: 1.5, height: 2.0 };
    let laid = untamed_seat_presets(&disc, f, &[], false);
    let (was, now) = (shot(disc.clone(), 0), shot(laid.clone(), 0));
    let ((wb, wa), (nb, na)) = (legs(&was), legs(&now));
    // Behind the quad (the eye at angle_y 180 sits at −z).
    assert!(wb < 0.0 && nb < 0.0);
    assert!((nb / wb - 1.5).abs() < 1e-4, "behind {wb} -> {nb}");
    assert!((na / wa - 2.0).abs() < 1e-4, "above {wa} -> {na}");
    assert!((now.fov.0 - (was.fov.0 + 5.0)).abs() < 1e-4);
    // The fast end too.
    let fast = untamed_framed(&disc[0], f).fast;
    let d = disc[0].fast.follow_distance.0;
    let a = disc[0].fast.angle_x.0;
    assert!((fast.follow_distance.0 * fast.angle_x.0.cos() - d * a.cos() * 1.5).abs() < 1e-3);
    assert!((fast.follow_distance.0 * fast.angle_x.0.sin() - d * a.sin() * 2.0).abs() < 1e-3);
    assert_eq!(fast.fov.0, 75.0);
}

#[test]
fn the_look_back_keeps_the_eye_in_front() {
    let disc = presets();
    let f = Framing { fov: 3.0, distance: 1.2, height: 1.0 };
    let back = &untamed_seat_presets(&disc, f, &[], false)[1];
    // Still a look-back distance (the springs run its way round), 1.2 times as far in front.
    assert!(back.slow.follow_distance.0 < 0.0);
    let (wb, _) = legs(&shot(disc.clone(), 1));
    let (nb, _) = legs(&shot(untamed_seat_presets(&disc, f, &[], false), 1));
    assert!(wb > 0.0 && (nb / wb - 1.2).abs() < 1e-4, "in front {wb} -> {nb}");
    // A file's fov of none is drawn at 77, and moves from there.
    assert_eq!(back.slow.fov.0, game_sim::camera::UNTAMED_NO_FOV + 3.0);
}

#[test]
fn a_seats_own_presets_line_up_with_the_stand_ins() {
    let disc = presets();
    let stand_ins: Vec<_> = disc.iter().map(UntamedPreset::stand_in).collect();
    let mine = Framing { fov: 0.0, distance: 2.0, height: 1.0 };
    let own = [Own { base: "Race", name: "Mine", framing: mine }, Own { base: "Back", name: "Back", framing: mine }];
    let laid = untamed_seat_presets(&disc, Framing::ORIGINAL, &own, true);
    let race = seat_presets(&stand_ins, Framing::ORIGINAL, &own, true);
    let cycle = |v: Vec<(String, bool, bool)>| v;
    assert_eq!(
        cycle(laid.iter().map(|p| (p.name.clone(), p.cycle_thru_this_preset, p.is_look_back_cam)).collect()),
        cycle(race.iter().map(|p| (p.name.clone(), p.cycle_thru_this_preset, p.is_look_back_cam)).collect()),
    );
    assert_eq!(laid.len(), 3);
    // The player's own copy of Race, twice as far back; the look-back on its own numbers.
    assert_eq!(laid[2].name, "Mine");
    assert_eq!(laid[2].slow, untamed_framed(&disc[0], mine).slow);
    assert!((laid[2].slow.follow_distance.0 - 28.0).abs() > 0.1, "the legs scale apart, not the distance");
    assert!((laid[1].slow.follow_distance.0 + 20.0).abs() < 1.0);
}

#[test]
fn a_framing_change_eases_the_field_of_view() {
    let disc = presets();
    let rider = standing();
    let mut cam = UntamedFollow::new(disc.clone());
    let mut ease = FovEase::default();
    // Not run yet: it snaps, nothing to ease from.
    untamed_reframe(&mut cam, untamed_seat_presets(&disc, Framing { fov: 10.0, distance: 1.0, height: 1.0 }, &[], false), 0, &mut ease);
    assert_eq!(ease.offset, 0.0);
    let dt = F(1.0 / 60.0);
    let first = cam.update(dt, &rider, &mut Nothing);
    assert_eq!(first.fov.0, 70.0);
    // Running: the view shown starts where it was and eases onto the new framing's.
    untamed_reframe(&mut cam, disc.clone(), 0, &mut ease);
    assert_eq!(cam.current, 0);
    assert!(!cam.snap);
    let next = cam.update(dt, &rider, &mut Nothing);
    assert_eq!(next.fov.0, 60.0);
    let shown = ease.ease(next.fov, dt.0);
    assert!(shown.0 > 69.0 && shown.0 < 70.0, "{shown:?}");
    let mut last = shown;
    for _ in 0..120 {
        last = ease.ease(cam.update(dt, &rider, &mut Nothing).fov, dt.0);
    }
    assert_eq!(last.0, 60.0);
}
