//! P157: the other-camera command held brings up the original's thrill camera — R2 under the
//! original's layout, Cross under the new one (P78) — while the rider is going faster than 3 ft/s as
//! it goes down, and letting go hands the screen back to the follow camera, or to the look-back if
//! that is still held: the camera manager's press and release handlers (`research/oracle.txt`, P78
//! and P157).
//!
//! Each tick goes the shell's way (`src/main.rs`): the pad polled, the ride stepped, the look-back
//! ([`chase::LookBack`]) with the thrill camera's up taking it off the screen, then the other camera
//! ([`chase::OtherCamera`]) on the follow camera's target, the follow camera's frame, and the thrill
//! camera's view over it while it is up.
//!
//! No disc: the bike is the simulation crate's stand-in and the camera `Race Cam 1`'s numbers.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;
#[allow(dead_code)]
#[path = "../src/aspect.rs"]
mod aspect;
#[allow(dead_code)]
#[path = "../src/split.rs"]
mod split;
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;

use std::sync::Arc;

use game_sim::camera::{DEG_TO_RAD, Obstacle, Obstacles, Preset, Race, Ridden, Surface, Switches, Target, Thrill, View};
use game_sim::collide::{Bundle, Record};
use game_sim::input::{CONTROL_MAP, Control, Report};
use game_sim::phy::Phy;
use game_sim::rider::Tables;
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};

const REVERSE_SPEED: f32 = 5.0;
const NEAR_CLIP: f32 = 2.4;
const DT: F = F(1.0 / 60.0);

fn preset(name: &str, look_back: bool) -> Preset {
    Preset {
        name: name.into(),
        look_filters: Vf::new(0.2, 0.2, 0.2, 0.0),
        pos_filters: Vf::new(0.3, 0.3, 0.3, 0.0),
        offset_from_body_point: Vf::new(0.0, 2.0, 0.0, 0.0),
        angle_y: F(180.0 * DEG_TO_RAD),
        angle_x: F(65.0 * DEG_TO_RAD),
        following_distance: F(13.0),
        fov: F(85.0),
        filter_this_preset: true,
        cycle_thru_this_preset: !look_back,
        is_look_back_cam: look_back,
        look_ahead_distance: F(1.0),
        min_following_distance: F(5.0),
        max_following_distance: F(40.0),
        min_fov: F(30.0),
        max_fov: F(100.0),
        ..Preset::default()
    }
}

fn camera() -> Race {
    Race::new(vec![preset("Race Cam 1", false), preset("Race Cam 2", false), preset("Look Back", true)], F(REVERSE_SPEED))
}

/// `tests/ride.rs`'s wreck limits: without a `[Wrecks]` section the stand-in bike never gets going.
const RIDE_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

fn sim() -> Sim {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(format!("{}{RIDE_LIMITS}", stock::BIKE).as_bytes(), &tables.ride.curve);
    Sim::ride(0, tables, phy, stock::SPAWN, stock::FACING, Set::default())
}

/// A pad's report with `held` down, full pressure on an analogue control.
fn report(held: &[Control]) -> Report {
    let mut r = Report::idle();
    let mut buttons = 0u16;
    for c in held {
        buttons |= c.mask();
    }
    let low = !buttons;
    r.0[2] = (low >> 8) as u8;
    r.0[3] = low as u8;
    for b in CONTROL_MAP {
        if b.is_analogue() && buttons & b.mask != 0 {
            r.0[b.pressure as usize] = 0xff;
        }
    }
    r
}

/// The two layouts, as the player's pad has them: the gas, and the other camera.
#[derive(Clone, Copy)]
enum Layout {
    /// The original's: Cross the gas, R2 the other camera.
    Original,
    /// The new one: R2 the gas, Cross the other camera, swapped back on their way into the ride
    /// ([`pad::Scheme::ride`]).
    New,
}

impl Layout {
    fn gas(self) -> Control {
        match self {
            Layout::Original => Control::Cross,
            Layout::New => Control::R2,
        }
    }
    fn other_camera(self) -> Control {
        match self {
            Layout::Original => Control::R2,
            Layout::New => Control::Cross,
        }
    }
    fn look_back(self) -> Control {
        match self {
            Layout::Original => Control::L2,
            Layout::New => Control::Square,
        }
    }
    fn ride(self, r: Report) -> Report {
        match self {
            Layout::Original => pad::Scheme::Original.ride(r),
            Layout::New => pad::Scheme::New.ride(r),
        }
    }
}

/// A level floor at height 0, for the ride and the camera alike.
struct Floor;

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

impl Surface for Floor {
    fn on_surface(&mut self, p: Vf) -> Vf {
        Vf::new(p.x(), 0.0, p.z(), p.w())
    }
}

impl Obstacles for Floor {
    fn nearest(&mut self, _start: Vf, _end: Vf) -> Option<Obstacle> {
        None
    }
}

/// A wall across the world at `x = 10`, facing back towards `x` below it.
struct Wall;

impl Surface for Wall {
    fn on_surface(&mut self, p: Vf) -> Vf {
        Vf::new(p.x(), 0.0, p.z(), p.w())
    }
}

impl Obstacles for Wall {
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle> {
        if start.x() < 10.0 && end.x() > 10.0 {
            let t = (10.0 - start.x()) / (end.x() - start.x());
            let at = |a: f32, b: f32| a + (b - a) * t;
            Some(Obstacle {
                point: Vf::new(10.0, at(start.y(), end.y()), at(start.z(), end.z()), 1.0),
                normal: Vf::new(-1.0, 0.0, 0.0, 0.0),
            })
        } else {
            None
        }
    }
}

/// One seat: its ride, its follow camera, its look-back and its other camera, as `main.rs` keeps them.
struct Seat {
    sim: Sim,
    cam: Race,
    back: chase::LookBack,
    other: chase::OtherCamera,
    /// Whether the follow camera has the screen (a wreck's camera does not take the press).
    on_screen: bool,
    layout: Layout,
}

impl Seat {
    fn new(layout: Layout) -> Seat {
        let mut sim = sim();
        for _ in 0..60 {
            sim.frame(DT, &[Report::idle(), Report::missing()], &mut Floor);
        }
        Seat { sim, cam: camera(), back: chase::LookBack::default(), other: chase::OtherCamera::default(), on_screen: true, layout }
    }

    /// One tick, the shell's way, with the gas held when `gas` and `held` down besides. Hands back
    /// the view drawn and the follow camera's own.
    fn tick(&mut self, gas: bool, held: &[Control]) -> (View, View) {
        let mut down = held.to_vec();
        if gas {
            down.push(self.layout.gas());
        }
        let r = self.layout.ride(report(&down));
        let controls = self.sim.poll(DT, &[r, Report::missing()]);
        self.sim.step(DT, &controls, &mut Floor);
        let pad = self.sim.input.pad(0);
        let table = self.sim.table();
        self.back.frame(&mut self.cam.cam, pad, table, self.on_screen && !self.other.up);
        let rider = chase::bike(&self.sim.rider.ride);
        self.other.frame(&mut self.cam.cam, &mut self.back, pad, table, self.on_screen, &rider, F(NEAR_CLIP), &mut Floor);
        let follow = self.cam.update(DT, &rider, F(NEAR_CLIP), &self.sim.tables.ride.curve, Switches::default(), &mut Floor);
        (self.other.view(&rider).unwrap_or(follow), follow)
    }

    fn speed(&self) -> f32 {
        let v = chase::bike(&self.sim.rider.ride).target.vel;
        (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt()
    }

    /// Ride on the gas until the bike is going faster than `speed`.
    fn get_going(&mut self, speed: f32) {
        for _ in 0..600 {
            if self.speed() > speed {
                return;
            }
            self.tick(true, &[]);
        }
        panic!("the bike never got past {speed} ft/s");
    }
}

fn same(a: &View, b: &View) -> bool {
    let bits = |v: &View| {
        let (p, l) = (v.position, v.look_at);
        [p.x(), p.y(), p.z(), l.x(), l.y(), l.z(), v.fov.0].map(f32::to_bits)
    };
    bits(a) == bits(b)
}

#[test]
fn the_other_camera_held_above_3_is_the_thrill_camera_under_either_layout() {
    for layout in [Layout::Original, Layout::New] {
        let mut one = Seat::new(layout);
        one.get_going(20.0);
        let r = one.layout.other_camera();
        let (first, follow) = one.tick(true, &[r]);
        assert!(one.other.up, "the press at speed brought the thrill camera up");
        assert!(!same(&first, &follow), "the thrill camera has the screen, not the follow camera");
        let spot = first.position;
        let mut last_fov = first.fov.0;
        for k in 0..90 {
            let (seen, _) = one.tick(true, &[r]);
            // The eye stays on its spot; the look-at is 3 ft over the rider's target point.
            assert_eq!(seen.position.to_bits(), spot.to_bits(), "tick {k}: the eye moved");
            let p = chase::bike(&one.sim.rider.ride).target.body_point;
            assert_eq!([seen.look_at.x(), seen.look_at.z()], [p.x(), p.z()], "tick {k}");
            assert_eq!(seen.look_at.y(), (F(p.y()) + 3.0).0, "tick {k}");
            assert!((18.0..=75.0).contains(&seen.fov.0), "tick {k}: fov {}", seen.fov.0);
            // Riding away from it, past it, the lens closes in.
            if k > 60 {
                assert!(seen.fov.0 <= last_fov, "tick {k}: the zoom opened riding away");
            }
            last_fov = seen.fov.0;
        }
        // Let go: the follow camera's view again, the follow camera having run on underneath.
        let (seen, follow) = one.tick(true, &[]);
        assert!(!one.other.up);
        assert!(same(&seen, &follow), "let go, the follow camera has the screen");
        // Pressed again it picks a new spot, off where the rider is now.
        let (again, _) = one.tick(true, &[r]);
        assert!(one.other.up);
        assert_ne!(again.position.to_bits(), spot.to_bits(), "a second press picks a new spot");
    }
}

#[test]
fn at_3_or_slower_the_press_does_nothing_until_pressed_again() {
    let mut one = Seat::new(Layout::Original);
    assert!(one.speed() <= 3.0, "the bike is standing: {}", one.speed());
    let (seen, follow) = one.tick(false, &[Control::R2]);
    assert!(!one.other.up, "a press standing still");
    assert!(same(&seen, &follow));
    // Held while the bike gets going, it stays the follow camera: the manager tests the speed on the
    // press only.
    for _ in 0..300 {
        let (seen, follow) = one.tick(true, &[Control::R2]);
        assert!(!one.other.up);
        assert!(same(&seen, &follow));
    }
    assert!(one.speed() > 3.0);
    one.tick(true, &[]);
    one.tick(true, &[Control::R2]);
    assert!(one.other.up, "pressed again at speed");
    // Under the original's layout only R2 is the other camera; Cross is its gas.
    let mut two = Seat::new(Layout::Original);
    two.get_going(20.0);
    for c in [Control::L2, Control::Square, Control::Cross, Control::R1, Control::Circle] {
        two.tick(true, &[c]);
        assert!(!two.other.up, "{c:?}");
        two.tick(true, &[]);
    }
}

#[test]
fn a_wreck_cameras_screen_takes_no_press() {
    let mut one = Seat::new(Layout::Original);
    one.get_going(20.0);
    one.on_screen = false;
    one.tick(true, &[Control::R2]);
    assert!(!one.other.up);
}

#[test]
fn let_go_with_the_look_back_held_is_the_look_back() {
    for layout in [Layout::Original, Layout::New] {
        let mut one = Seat::new(layout);
        one.get_going(20.0);
        let (r, l) = (layout.other_camera(), layout.look_back());
        one.tick(true, &[r]);
        assert!(one.other.up);
        // The look-back pressed under the thrill camera: nothing, the race camera on the player's
        // preset.
        one.tick(true, &[r, l]);
        assert!(one.other.up);
        assert_eq!(one.cam.cam.preset, 0);
        // The other camera let go with the look-back still down: the look-back.
        one.tick(true, &[l]);
        assert!(!one.other.up);
        assert_eq!(one.cam.cam.preset, 2, "the look-back came up as the thrill camera went");
        // And let go of that, the player's own preset.
        one.tick(true, &[]);
        assert_eq!(one.cam.cam.preset, 0);
        // The look-back first, then the thrill camera over it: let go of both, the player's preset.
        one.tick(true, &[l]);
        assert_eq!(one.cam.cam.preset, 2);
        one.tick(true, &[l, r]);
        assert!(one.other.up);
        one.tick(true, &[r]);
        one.tick(true, &[]);
        assert!(!one.other.up);
        assert_eq!(one.cam.cam.preset, 0);
    }
}

fn rider(point: Vf, vel: Vf, airborne: bool) -> Ridden {
    Ridden {
        target: Target { body_point: point, forward: Vf::new(0.0, 0.0, 1.0, 1.0), up: Vf::new(0.0, 1.0, 0.0, 1.0), vel, lean: F::ZERO },
        airborne,
        speed: F(0.0),
    }
}

fn near(a: Vf, b: [f32; 3]) -> bool {
    (a.x() - b[0]).abs() < 1e-3 && (a.y() - b[1]).abs() < 1e-3 && (a.z() - b[2]).abs() < 1e-3
}

#[test]
fn the_spot_is_the_originals() {
    let eye = Vf::new(0.0, 8.0, -13.0, 1.0);
    // On the ground at 40 ft/s along +z: a lead of 1.5 out, a fifth back, 0.45 to the side, and
    // six feet over the floor (the clearance and a foot, over half the seat's height).
    let mut t = Thrill::default();
    let on = rider(Vf::new(0.0, 3.0, 0.0, 1.0), Vf::new(0.0, 0.0, 40.0, 1.0), false);
    let spot = t.place(&on, eye, F(NEAR_CLIP), F::MAX, &mut Floor);
    assert!(near(spot, [18.0, 6.0, 52.0]), "{spot:?}");
    // The zoom off it: 55 ft flat to the look-at.
    let v = t.frame(&on).unwrap();
    let away = (18.0f32 * 18.0 + 52.0 * 52.0).sqrt();
    assert!((v.fov.0 - (18.0 + 57.0 * (200.0 - away) / 180.0)).abs() < 1e-3, "{}", v.fov.0);
    // Slower than 30 ft/s, the velocity is stretched to 30 first.
    let slow = rider(Vf::new(0.0, 3.0, 0.0, 1.0), Vf::new(0.0, 0.0, 10.0, 1.0), false);
    let spot = t.place(&slow, eye, F(NEAR_CLIP), F::MAX, &mut Floor);
    assert!(near(spot, [13.5, 6.0, 39.0]), "{spot:?}");
    // A wall in the way keeps the spot five feet off it.
    let spot = t.place(&on, eye, F(NEAR_CLIP), F::MAX, &mut Wall);
    assert!((spot.x() - 5.0).abs() < 1e-3, "{spot:?}");
    // Past 200 ft the lens holds at 18.
    let far = rider(Vf::new(0.0, 3.0, 300.0, 1.0), Vf::new(0.0, 0.0, 40.0, 1.0), false);
    t.place(&on, eye, F(NEAR_CLIP), F::MAX, &mut Floor);
    assert_eq!(t.frame(&far).unwrap().fov.0, 18.0);
    assert!(t.expired);
    // Up off a jump, the spot comes off the takeoff, not off where the rider is now.
    let mut t = Thrill::default();
    t.watch(&on);
    let off = rider(Vf::new(0.0, 4.0, 10.0, 1.0), Vf::new(0.0, 20.0, 40.0, 1.0), true);
    t.watch(&off);
    let up = rider(Vf::new(0.0, 20.0, 40.0, 1.0), Vf::new(0.0, 5.0, 40.0, 1.0), true);
    t.watch(&up);
    assert_eq!(t.takeoff, Some((off.target.body_point, off.target.vel)));
    let spot = t.place(&up, eye, F(NEAR_CLIP), F::MAX, &mut Floor);
    let lead: f32 = 20.0 * 0.0310559 * 1.5;
    let top: f32 = 4.0 + 20.0 * lead * 0.1 - 1.0;
    assert!(near(spot, [18.0, top.max(6.0), 10.0 + 40.0 * lead - 8.0]), "{spot:?}");
    // The manager's speed test.
    assert!(!Thrill::fast_enough(Vf::new(0.0, 0.0, 3.0, 1.0)));
    assert!(Thrill::fast_enough(Vf::new(0.0, 0.0, 3.01, 1.0)));
}
