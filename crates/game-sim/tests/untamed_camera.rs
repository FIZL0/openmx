//! MD251: MX vs. ATV Untamed's follow camera for the quad, against MD147's `spawn.p2s` (T7): the
//! quad at rest on Normal Camera, read out of the RAM image (`context/untamed/md147/spawn.ee.bin`,
//! camera `0xf96e60`, its target block `0x1be5110`, its filter `0xf96b50`).

use game_sim::camera::{Obstacle, Obstacles, Ridden, Surface, Target, UntamedFollow, UntamedPreset};
use game_sim::phy::Ini;
use ps2_float::{F, Vf};

/// `Cam\FC_ATVDefault.pb`'s first two presets, as the BPB reads.
const FILE: &str = "\
[4x3/1p/1]
Name=Normal Camera
AngleY=180.0
SlowAngleX=18.0
SlowFollowDistance=17.0
SlowLookAheadDistance=10.0
SlowFov=75.0
SlowSpeed=2.0
FastAngleX=15.0
FastFollowDistance=19
FastLookAheadDistance=40.0
FastFov=85.0
FastSpeed=75.0
OnGroundFollowTightness=14
OnGroundLookAheadTightness=35
InAirFollowTightness=10
InAirLookAheadTightness=30
AngleXAffectedByHills=0.3499999940395355
CycleThruThisPreset=1
FilterThisPreset=1
IsLookBackCam=0
CameraSwing=0.10000000149011612
LookSwing=0.6000000238418579
OffsetFromBodyPoint=0.0,0.35,0.0
[4x3/1p/2]
Name=Look Back Camera
AngleY=0.0
SlowAngleX=5.0
SlowFollowDistance=-10.0
SlowFov=85.0
SlowSpeed=5.0
FastAngleX=10.0
FastFollowDistance=-10.0
FastLookAheadDistance=-30.0
FastFov=85.0
FastSpeed=75.0
CycleThruThisPreset=0
FilterThisPreset=0
IsLookBackCam=1
OffsetFromBodyPoint=0.0,4.0,0.0
";

fn v(b: [u32; 4]) -> Vf {
    Vf(b.map(f32::from_bits))
}

/// The target block at spawn: position, forward and up rows, velocity; on the ground.
fn spawn() -> Ridden {
    Ridden {
        target: Target {
            body_point: v([0x43c5d1d8, 0xbe52b178, 0x431fccf3, 0x3f800000]),
            forward: v([0xb5e0ec96, 0x3cb7e7ed, 0x3f7fef7b, 0x00000000]),
            up: v([0x36eb8c97, 0x3f7fef7b, 0xbcb7e7ec, 0x00000000]),
            vel: v([0xb4483a9a, 0xba8ae6fa, 0x3de049e2, 0x3f800000]),
            lean: F::ZERO,
        },
        airborne: false,
        speed: F::ZERO,
    }
}

fn near(got: Vf, want: Vf, tol: f32) -> bool {
    (0..3).all(|i| (got[i] - want[i]).abs() <= tol)
}

struct Flat(f32);

impl Surface for Flat {
    fn on_surface(&mut self, p: Vf) -> Vf {
        Vf::new(p.x(), self.0, p.z(), p.w())
    }
}

impl Obstacles for Flat {
    fn nearest(&mut self, _start: Vf, _end: Vf) -> Option<Obstacle> {
        None
    }
}

/// Flat ground far down and one wall: the plane `z = at`, its face towards `+z` when `facing` is 1
/// and `-z` when −1. As [`Obstacles::nearest`] asks, a line that leaves from behind the face meets
/// nothing.
struct Wall {
    at: f32,
    facing: f32,
}

impl Surface for Wall {
    fn on_surface(&mut self, p: Vf) -> Vf {
        Vf::new(p.x(), -100.0, p.z(), p.w())
    }
}

impl Obstacles for Wall {
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        let (s, e) = ((start.z() - self.at) * self.facing, (end.z() - self.at) * self.facing);
        if s < 0.0 || e >= 0.0 {
            return None;
        }
        let t = s / (s - e);
        let point = Vf::new(
            start.x() + (end.x() - start.x()) * t,
            start.y() + (end.y() - start.y()) * t,
            self.at,
            1.0,
        );
        Some(Obstacle { point, normal: Vf::new(0.0, 0.0, self.facing, 0.0) })
    }
}

/// The spawn quad turned square to `+z` and still.
fn square() -> Ridden {
    let mut rider = spawn();
    rider.target.forward = Vf::new(0.0, 0.0, 1.0, 0.0);
    rider.target.up = Vf::new(0.0, 1.0, 0.0, 0.0);
    rider.target.vel = Vf::new(0.0, 0.0, 0.0, 1.0);
    rider
}

#[test]
fn the_reader_takes_the_slow_fast_pairs_and_defaults_the_rest() {
    let ini = Ini::parse(FILE);
    assert!(UntamedPreset::is_slow_fast(&ini));
    let p = UntamedPreset::read(&ini);
    assert_eq!(p.len(), 2);
    // The text reader squeezes the spaces out, as the game's does.
    assert_eq!(p[0].name, "NormalCamera");
    assert_eq!((p[0].slow.follow_distance, p[0].fast.follow_distance), (F(17.0), F(19.0)));
    assert_eq!((p[0].slow.speed, p[0].fast.speed), (F(2.0), F(75.0)));
    assert_eq!(p[0].on_ground_look_ahead_tightness, F(35.0));
    // The file spells the swing `CameraSwing`; the reader asks for `Swing`.
    assert_eq!(p[0].swing, F::ZERO);
    // Keys the second preset leaves out take the reader's defaults.
    assert_eq!(p[1].slow.look_ahead_distance, F::ZERO);
    assert_eq!(p[1].on_ground_follow_tightness, F(16.0));
    assert_eq!(p[1].in_air_look_ahead_tightness, F(20.0));
    assert!(p[1].is_look_back_cam && !p[1].filter_this_preset && !p[1].cycle_thru_this_preset);
}

#[test]
fn the_blend_runs_from_slow_to_fast_by_speed() {
    let mut p = UntamedPreset::read(&Ini::parse(FILE)).remove(0);
    p.blend_at(F(1.0));
    assert_eq!((p.blend.follow_distance, p.blend.fov), (F(17.0), F(75.0)));
    p.blend_at(F(80.0));
    assert_eq!((p.blend.follow_distance, p.blend.fov), (F(19.0), F(85.0)));
    p.blend_at(F(38.5));
    assert!((p.blend.follow_distance.0 - 18.0).abs() < 1e-5);
    assert!((p.blend.look_ahead_distance.0 - 25.0).abs() < 1e-4);
}

#[test]
fn at_spawn_the_goal_is_the_originals() {
    let mut cam = UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE)));
    let goal = cam.goal(&spawn());
    // The filter's origin, look goal and eye goal in the image.
    let target = v([0x43c5d1d8, 0x40492d78, 0x431fb9b2, 0]);
    let look = v([0x43c5d1d7, 0x404e3518, 0x4329b99d, 0x3f800000]);
    let eye = v([0x43c5d1d8, 0x41044a72, 0x430f8426, 0x3f800000]);
    assert!(near(goal.target, target, 1e-4), "target {:?}, the original's {target:?}", goal.target);
    assert!(near(goal.look, look, 1e-4), "look {:?}, the original's {look:?}", goal.look);
    assert!(near(goal.eye, eye, 1e-4), "eye {:?}, the original's {eye:?}", goal.eye);
    assert_eq!(goal.fov, F(75.0));
    assert_eq!((goal.follow_tightness, goal.look_tightness), (F(14.0), F(35.0)));
    assert!(goal.eye_orbits && goal.filter);
}

#[test]
fn the_springs_snap_first_and_then_settle_on_a_still_goal() {
    let mut cam = UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE)));
    let mut rider = spawn();
    rider.target.vel = Vf::new(0.0, 0.0, 0.0, 1.0);
    let first = cam.update(F(1.0 / 30.0), &rider, &mut Flat(-100.0));
    let goal = cam.goal(&rider);
    assert_eq!((first.position, first.look_at), (goal.eye, goal.look));
    for _ in 0..30 {
        let view = cam.update(F(1.0 / 30.0), &rider, &mut Flat(-100.0));
        assert!(near(view.position, goal.eye, 1e-3) && near(view.look_at, goal.look, 1e-3), "{view:?}");
    }
}

#[test]
fn the_eye_trails_a_moving_quad_and_catches_up() {
    let mut cam = UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE)));
    let mut rider = spawn();
    rider.target.forward = Vf::new(0.0, 0.0, 1.0, 0.0);
    rider.target.up = Vf::new(0.0, 1.0, 0.0, 0.0);
    rider.target.vel = Vf::new(0.0, 0.0, 30.0, 0.0);
    let dt = F(1.0 / 30.0);
    let mut gaps = Vec::new();
    for _ in 0..120 {
        let view = cam.update(dt, &rider, &mut Flat(-100.0));
        let goal = cam.goal(&rider);
        gaps.push((view.position[2] - goal.eye[2]).abs());
        rider.target.body_point[2] += 1.0;
    }
    // Behind the goal while the quad pulls away, then settled at a steady lag.
    assert!(gaps[5] > 0.01, "{gaps:?}");
    assert!((gaps[119] - gaps[118]).abs() < 1e-3, "{gaps:?}");
    assert!(gaps[119] < 2.0, "{gaps:?}");
}

#[test]
fn the_eye_keeps_over_the_ground() {
    let mut cam = UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE)));
    // A snap draws the goal as handed in: the filter's output words are the goal's, unswept.
    let first = cam.update(F(1.0 / 30.0), &spawn(), &mut Flat(7.0));
    assert_eq!(first.position, cam.goal(&spawn()).eye);
    assert!(first.position.y() < 9.5);
    // But the sweep floored the eye and its goal, so the next frame starts 2.5 ft over the ground.
    assert_eq!((cam.eye.pos.y(), cam.eye.goal.y()), (9.5, 9.5));
    let next = cam.update(F(1.0 / 30.0), &spawn(), &mut Flat(7.0));
    assert!((next.position.y() - 9.5).abs() < 1e-2, "{next:?}");
}

#[test]
fn a_wall_behind_the_quad_puts_the_eye_two_feet_in_front_of_it() {
    let mut cam = UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE)));
    let rider = square();
    let goal = cam.goal(&rider);
    let at = rider.target.body_point.z() - 8.0;
    assert!(goal.eye.z() < at, "the goal starts behind the wall");
    let mut wall = Wall { at, facing: 1.0 };
    let dt = F(1.0 / 30.0);
    cam.update(dt, &rider, &mut wall);
    // The line from the target point to the goal meets the wall's face: the goal goes 2 ft out in
    // front of it, along its normal, keeping its height and side.
    assert!((cam.eye.goal.z() - (at + 2.0)).abs() < 1e-4, "{:?}", cam.eye.goal);
    assert_eq!((cam.eye.goal.x(), cam.eye.goal.y()), (goal.eye.x(), goal.eye.y()));
    let mut view = cam.update(dt, &rider, &mut wall);
    for _ in 0..60 {
        view = cam.update(dt, &rider, &mut wall);
    }
    assert!((view.position.z() - (at + 2.0)).abs() < 1e-2, "{view:?}");
    assert!((view.position.y() - goal.eye.y()).abs() < 1e-2, "{view:?}");
}

#[test]
fn backing_into_a_wall_the_goal_stays_in_front_and_the_eye_settles_there() {
    let mut cam = UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE)));
    let mut rider = square();
    rider.target.vel = Vf::new(0.0, 0.0, -10.0, 0.0);
    let at = rider.target.body_point.z() - 30.0;
    let mut wall = Wall { at, facing: 1.0 };
    let dt = F(1.0 / 30.0);
    // The sweeps move the goal, not the eye: the goal never goes past the face (once it would, it
    // is put 2 ft in front), where it would be 6 ft behind it by the end. The eye's spring, carried
    // along at the quad's speed, lags out past the goal and may run past the face while the quad
    // backs towards it.
    for tick in 0..60 {
        cam.update(dt, &rider, &mut wall);
        assert!(cam.eye.goal.z() > at, "tick {tick}: {:?}, the wall at {at}", cam.eye.goal);
        rider.target.body_point[2] -= 1.0 / 3.0;
    }
    assert!(cam.goal(&rider).eye.z() < at - 5.0);
    // Stopped 10 ft from it, the eye comes back and settles 2 ft in front.
    rider.target.vel = Vf::new(0.0, 0.0, 0.0, 1.0);
    let mut view = cam.update(dt, &rider, &mut wall);
    for _ in 0..60 {
        view = cam.update(dt, &rider, &mut wall);
    }
    assert!((view.position.z() - (at + 2.0)).abs() < 1e-2, "{view:?}, the wall at {at}");
}

#[test]
fn the_second_sweep_holds_the_goal_off_a_prop_between_it_and_the_eye() {
    let mut cam = UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE)));
    let mut wall = Wall { at: 0.0, facing: 1.0 };
    // The eye 10 ft in front of the face, its goal half a foot in front: the line out past the goal
    // meets the face, and the goal goes to 2 ft in front.
    cam.eye.pos = Vf::new(3.0, 4.0, 10.0, 1.0);
    cam.eye.goal = Vf::new(3.0, 4.0, 0.5, 1.0);
    assert!(cam.sweep_from_eye(&mut wall));
    assert_eq!(cam.eye.goal, Vf::new(3.0, 4.0, 2.0, 1.0));
    // A goal 1 ft or more in front is left, and so is an eye within 0.1 ft of its goal.
    cam.eye.goal = Vf::new(3.0, 4.0, 1.5, 1.0);
    cam.eye.pos = Vf::new(3.0, 4.0, 1.55, 1.0);
    assert!(!cam.sweep_from_eye(&mut wall));
    cam.eye.pos = Vf::new(3.0, 4.0, 10.0, 1.0);
    assert!(!cam.sweep_from_eye(&mut wall));
    assert_eq!(cam.eye.goal, Vf::new(3.0, 4.0, 1.5, 1.0));
}
