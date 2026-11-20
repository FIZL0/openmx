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

// --- R51: backed into Rocksbury's south wall, against the original --------------------------------

/// One row of R51's `back.csv` (`context/capture/r51`, research/r51/camera.toml): the column name to
/// its text.
struct Row(std::collections::BTreeMap<String, String>);

impl Row {
    /// A raw column's little-endian words.
    fn words(&self, k: &str) -> Vec<u32> {
        let h = &self.0[k];
        (0..h.len() / 8)
            .map(|n| u32::from_le_bytes(std::array::from_fn(|b| u8::from_str_radix(&h[8 * n + 2 * b..8 * n + 2 * b + 2], 16).unwrap())))
            .collect()
    }

    /// The quadword at byte `off` of raw column `k`.
    fn q(&self, k: &str, off: usize) -> Vf {
        let w = self.words(k);
        v([w[off / 4], w[off / 4 + 1], w[off / 4 + 2], w[off / 4 + 3]])
    }

    /// The target block as the goal reads it: body point `+0x10`, forward `+0x30`, up `+0x40`,
    /// velocity `+0x50`, and `+0xb0` set in the air.
    fn ridden(&self) -> Ridden {
        let vel = self.q("target", 0x50);
        Ridden {
            target: Target {
                body_point: self.q("target", 0x10),
                forward: self.q("target", 0x30),
                up: self.q("target", 0x40),
                vel,
                lean: F::ZERO,
            },
            airborne: self.words("target")[0xb0 / 4] != 0,
            speed: F(vel.x() * vel.x() + vel.y() * vel.y() + vel.z() * vel.z()).sqrt(),
        }
    }

    /// The filter's spring at byte `off`: goal, position, velocity, last target point.
    fn spring(&self, off: usize) -> game_sim::camera::Spring {
        game_sim::camera::Spring {
            goal: self.q("filter", off),
            pos: self.q("filter", off + 0x10),
            vel: self.q("filter", off + 0x20),
            prev: self.q("filter", off + 0x30),
        }
    }
}

fn back_rows() -> Option<Vec<Row>> {
    let kit = ps2_oracle::Kit::load().map_err(|w| ps2_oracle::skip(&w)).ok()?;
    let path = kit.ctx("capture/r51").join("back.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        ps2_oracle::skip(&format!("{} is not there (R51's)", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(str::to_string).collect();
    Some(lines.map(|l| Row(head.iter().cloned().zip(l.split(',').map(str::to_string)).collect())).collect())
}

/// Rocksbury's solids (`SX_A.scn`'s collision files), placed as `untamed_race_quad.rs` places them.
fn rocksbury() -> Option<game_sim::world::Props> {
    let kit = ps2_oracle::Kit::load().map_err(|w| ps2_oracle::skip(&w)).ok()?;
    let dir = kit.ctx("untamed/sx_a");
    let Ok(bytes) = std::fs::read(dir.join("env/sx_a/SX_A.scn.bpb")) else {
        ps2_oracle::skip(&format!("{} has no SX_A.scn.bpb (SX_A.res extracted, MD168b)", dir.display()));
        return None;
    };
    let ini = game_sim::bpb::ini(&bytes).unwrap_or_else(|e| panic!("SX_A.scn: {e}"));
    let mut text = String::new();
    let mut last = None;
    for (section, key, value) in ini.entries() {
        if last != Some(section) {
            text += &format!("[{section}]\n");
            last = Some(section);
        }
        text += &format!("{key}={value}\n");
    }
    let scene = game_sim::track::Scene::parse(text.as_bytes()).unwrap_or_else(|e| panic!("SX_A.scn: {e}"));
    let files: Vec<std::path::PathBuf> =
        std::fs::read_dir(dir.join("col")).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    let (props, missed) = game_sim::world::Props::load(&scene, |name| {
        let p = files
            .iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(name)))
            .ok_or("not in SX_A.res")?;
        std::fs::read(p).map_err(|e| e.to_string())
    });
    assert_eq!(missed, ["FP01A_E.col: not in SX_A.res"], "Rocksbury's solids not placed");
    Some(props)
}

/// The stadium as the camera's filter meets it. Its walls are props, not the world's triangles: so
/// the goal sweep's first ask in a frame (the world's, whose loop ends on a miss) meets nothing and
/// every later ask is the props', unless `world_too`, when the props answer the world's ask as well
/// (as a [`game_sim::world::Place`] does). The ground is far down.
struct Stadium<'a> {
    props: &'a mut game_sim::world::Props,
    asks: usize,
    world_too: bool,
}

impl Surface for Stadium<'_> {
    fn on_surface(&mut self, p: Vf) -> Vf {
        Vf::new(p.x(), -100.0, p.z(), p.w())
    }
}

impl Obstacles for Stadium<'_> {
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        self.asks += 1;
        if self.asks == 1 && !self.world_too {
            return None;
        }
        self.props.nearest(start, end)
    }
}

/// R51: the quad backed into Rocksbury's south wall from MD147's spawn (research/r51/back.pad), the
/// original's filter read every tick (`back.csv`). Each row's frame is run from the row before's
/// springs on the row's target block, with Rocksbury's solids as the scene, and the eye's swept goal
/// and position are the original's. The wall pushes the goal on 380 of the 653 rows run. On the
/// first, nothing is on the target point's line yet and the eye's sweep puts the goal 2 ft out; on
/// the other 379 the goal sweep's props hit puts it 2.5 ft out from the face that line meets (two
/// faces of the wall, about 6 and 1.4 degrees off the z axis), and the eye, on the straight spring,
/// runs up to 5.68 ft behind the face before it comes back. Asked as world triangles too, the same
/// wall would stop the goal 2 ft out on all 379.
///
/// A row whose filter is not one frame on from the row before's (the reader caught the target block
/// or the filter a frame early or late; told by the springs' last target point) is left out.
#[test]
fn r51_the_eye_backed_into_rocksburys_wall_is_the_originals() {
    let Some(rows) = back_rows() else { return };
    let Some(mut props) = rocksbury() else { return };
    let origin = |r: &Row| UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE))).goal(&r.ridden()).target;
    let same = |a: Vf, b: Vf| (0..3).all(|k| a[k] == b[k]);
    let (mut run, mut pressed, mut by_eye, mut behind, mut two_ft) = (0, 0, 0, 0.0f32, 0);
    for i in 2..rows.len() - 1 {
        let (was, now) = (&rows[i - 1], &rows[i]);
        let Some(j) = (i - 1..=i + 1).find(|&j| same(origin(&rows[j]), now.q("filter", 0xa0))) else { continue };
        if !same(origin(&rows[j - 1]), was.q("filter", 0xa0)) {
            continue;
        }
        let frame = |world_too: bool, props: &mut game_sim::world::Props| {
            let mut cam = UntamedFollow::new(UntamedPreset::read(&Ini::parse(FILE)));
            cam.snap = false;
            cam.look = was.spring(0x30);
            cam.eye = was.spring(0x70);
            let rider = rows[j].ridden();
            let unswept = cam.goal(&rider).eye;
            cam.update(F(1.0 / 30.0), &rider, &mut Stadium { props, asks: 0, world_too });
            (cam, unswept)
        };
        let (cam, unswept) = frame(false, &mut props);
        let (goal, eye) = (now.q("filter", 0x70), now.q("filter", 0x80));
        assert!(near(cam.eye.goal, goal, 1e-5), "row {i}: goal {:?}, the original's {goal:?}", cam.eye.goal);
        assert!(near(cam.eye.pos, eye, 1e-4), "row {i}: eye {:?}, the original's {eye:?}", cam.eye.pos);
        run += 1;
        if unswept == goal {
            continue;
        }
        let off = |p: Vf, face: Obstacle| (0..3).map(|k| (p[k] - face.point[k]) * face.normal[k]).sum::<f32>();
        let Some((_, face)) = props.along(origin(&rows[j]), unswept) else {
            // Nothing on the target point's line: the eye's sweep, from where the eye was to as far
            // past the goal again, found the goal under a foot in front of the wall.
            let from = was.q("filter", 0x80);
            let past = Vf::new(2.0 * unswept.x() - from.x(), 2.0 * unswept.y() - from.y(), 2.0 * unswept.z() - from.z(), 1.0);
            let (_, face) = props.along(from, past).expect("the goal moved with no wall on either line");
            assert!(off(unswept, face) < 1.0 && (off(goal, face) - 2.0).abs() < 1e-3, "row {i}");
            by_eye += 1;
            continue;
        };
        assert!((off(goal, face) - 2.5).abs() < 1e-3, "row {i}: goal {} ft off the wall", off(goal, face));
        pressed += 1;
        behind = behind.max(-off(eye, face));
        let (merged, _) = frame(true, &mut props);
        two_ft += usize::from((off(merged.eye.goal, face) - 2.0).abs() < 1e-3);
    }
    assert_eq!((run, pressed, by_eye, two_ft), (653, 379, 1, 379));
    assert!((behind - 5.68).abs() < 0.01, "the eye ran {behind} ft behind the wall");
}
