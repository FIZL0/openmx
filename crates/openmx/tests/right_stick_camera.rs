//! P114: the right stick turns the camera, L3 held zooms it, and clicking the stick (R3) puts it
//! back.
//!
//! Each seat rides its own [`game_sim::Sim`] and has its own race camera, as `src/main.rs` keeps them,
//! and each tick goes the shell's way: the seat's reports polled, the ride stepped, then
//! [`chase::look_around`] on that seat's pad and table, then the camera's frame. The look-around is
//! the original's (`Race::nudge` over `Follow::nudge`, oracle.txt's `camera_nudge`): 0.05 rad of yaw
//! a tick at full stick, 0.3 ft of zoom a tick with the zoom command held. The reset is the port's.
//!
//! P78: the look-back command held puts the camera on its look-back preset until it is let go, as
//! the original's camera manager does on L2 (the new layout's Square, which reaches the ride as L2:
//! `src/pad.rs`'s own tests).
//!
//! No disc: the bike is the simulation crate's stand-in and the camera `Race Cam 1`'s numbers.

#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(dead_code)]
#[path = "../src/chase.rs"]
mod chase;

use std::sync::Arc;

use game_sim::camera::{DEG_TO_RAD, Nothing, Preset, Race, Switches, View};
use game_sim::input::{CONTROL_MAP, Control, Report};
use game_sim::phy::Phy;
use game_sim::rider::Tables;
use game_sim::trick::Set;
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};

/// The shell's camera numbers (`main.rs`), as `tests/camera_view.rs` repeats them.
const REVERSE_SPEED: f32 = 5.0;
const NEAR_CLIP: f32 = 2.4;
const DT: F = F(1.0 / 60.0);

/// The report's right stick bytes ([`Report::set_axis`]'s order).
const RIGHT_X: usize = 0;
const RIGHT_Y: usize = 1;

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

/// `tests/ride.rs`'s wreck limits, appended to the stand-in bike as that file does: without a
/// `[Wrecks]` section this bike never gets going.
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

/// A pad's report: `held` down (full pressure on an analogue control), the right stick at `right`,
/// and the gas held so the bike is moving.
fn report(held: &[Control], right: (u8, u8)) -> Report {
    let mut r = Report::idle();
    let mut buttons = Control::Cross.mask();
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
    r.set_axis(RIGHT_X, right.0);
    r.set_axis(RIGHT_Y, right.1);
    r
}

const CENTRE: (u8, u8) = (0x7f, 0x7f);
/// The stick full right: the poll inverts the right x, so this reads −1.
const FULL_RIGHT: (u8, u8) = (0xff, 0x7f);
/// The stick full down: +1.
const FULL_DOWN: (u8, u8) = (0x7f, 0xff);

/// A level floor at height 0, as `tests/ride.rs`'s world is between its ramps.
struct Floor;

impl World for Floor {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// One seat: its own ride, its own camera, and a camera beside it that never sees the stick.
struct Seat {
    sim: Sim,
    cam: Race,
    plain: Race,
    world: Floor,
    /// The look-back command on `cam` (P78), as `main.rs` keeps one beside each follow camera.
    back: chase::LookBack,
    /// Whether the follow camera has the screen (a wreck's camera does not take the press).
    on_screen: bool,
}

impl Seat {
    fn new() -> Seat {
        // A second on an idle pad first, the state a session begins in (`tests/ride.rs`).
        let (mut sim, mut world) = (sim(), Floor);
        for _ in 0..60 {
            sim.frame(DT, &[Report::idle(), Report::missing()], &mut world);
        }
        Seat { sim, cam: camera(), plain: camera(), world, back: chase::LookBack::default(), on_screen: true }
    }

    /// One tick, the shell's way. R3's press is the shell's reset (`pad::Scheme::resets_camera`,
    /// under either layout); `look` false skips the look-around, as a shell without P114 would.
    fn tick(&mut self, r: Report, look: bool) -> (View, View) {
        let controls = self.sim.poll(DT, &[r, Report::missing()]);
        self.sim.step(DT, &controls, &mut self.world);
        let pad = self.sim.input.pad(0);
        self.back.frame(&mut self.cam.cam, pad, self.sim.table(), self.on_screen);
        if look {
            chase::look_around(&mut self.cam, pad, self.sim.table(), pad.pressed(Control::R3));
        }
        let rider = chase::bike(&self.sim.rider.ride);
        let curve = &self.sim.tables.ride.curve;
        let s = Switches::default();
        let near = F(NEAR_CLIP);
        (
            self.cam.update(DT, &rider, near, curve, s, &mut Nothing),
            self.plain.update(DT, &rider, near, curve, s, &mut Nothing),
        )
    }
}

fn bits(v: &View) -> [u32; 9] {
    let (p, l) = (v.position, v.look_at);
    [p.x(), p.y(), p.z(), l.x(), l.y(), l.z(), v.fov.0, 0.0, 0.0].map(f32::to_bits)
}

/// The original's yaw step, `k` ticks of it at full right: `user_yaw - (-1) * 0.05`, wrapped.
fn yawed(k: usize) -> F {
    let mut want = F::ZERO;
    for _ in 0..k {
        want = want - F(-1.0) * 0.05;
    }
    want
}

#[test]
fn a_held_right_stick_turns_1ps_camera_and_leaves_2ps_alone() {
    let (mut one, mut two) = (Seat::new(), Seat::new());
    let angle_y = one.cam.cam.preset().angle_y;
    for k in 1..=40 {
        let (seen, plain) = one.tick(report(&[], FULL_RIGHT), true);
        let (seen2, plain2) = two.tick(report(&[], CENTRE), true);
        // The original's rate, every tick: 0.05 rad of the player's yaw, and the camera's yaw the
        // preset's less it.
        assert_eq!(one.cam.cam.user_yaw.0.to_bits(), yawed(k).0.to_bits(), "tick {k}: 1P's yaw nudge");
        assert_eq!(one.cam.cam.yaw.0.to_bits(), (angle_y - yawed(k)).0.to_bits(), "tick {k}: 1P's yaw");
        assert_eq!(one.cam.cam.user_pitch, F::ZERO, "tick {k}: a level stick does not pitch");
        assert_ne!(bits(&seen), bits(&plain), "tick {k}: 1P's view did not turn");
        // 2P's stick is centred: its camera is the camera that never saw a stick, to the bit.
        assert_eq!(bits(&seen2), bits(&plain2), "tick {k}: 2P's view moved");
        assert_eq!(two.cam.cam.user_yaw, F::ZERO);
    }
}

#[test]
fn with_l3_held_it_zooms_instead() {
    let mut one = Seat::new();
    let mut want = F::ZERO;
    for k in 1..=10 {
        one.tick(report(&[Control::L3], FULL_DOWN), true);
        want = want + F(1.0) * 0.3;
        let c = &one.cam.cam;
        assert_eq!(c.user_zoom.0.to_bits(), want.0.to_bits(), "tick {k}: the zoom nudge");
        assert_eq!((c.user_yaw, c.user_pitch, c.user_fov), (F::ZERO, F::ZERO, F::ZERO), "tick {k}");
        assert!(c.distance > one.plain.cam.distance, "tick {k}: the camera did not move out");
    }
    // R1 is the widen modifier, and the race camera lets go of it before it looks: held, the stick
    // looks round as if nothing were held.
    let mut one = Seat::new();
    one.tick(report(&[Control::R1], FULL_DOWN), true);
    let c = &one.cam.cam;
    assert_eq!((c.user_zoom, c.user_fov), (F::ZERO, F::ZERO));
    assert_eq!(c.user_pitch.0.to_bits(), (F::ZERO + F(1.0) * -0.05).0.to_bits(), "R1 held: the stick pitches");
}

#[test]
fn an_r3_click_puts_the_camera_back() {
    let mut one = Seat::new();
    for _ in 0..20 {
        one.tick(report(&[], FULL_RIGHT), true);
    }
    for _ in 0..10 {
        one.tick(report(&[Control::L3], FULL_DOWN), true);
    }
    let c = &one.cam.cam;
    assert_ne!((c.yaw, c.distance), (one.plain.cam.yaw, one.plain.cam.distance));
    // The click, the stick let go: the preset's angles and distance on that very tick, the ones the
    // camera that never saw the stick has.
    one.tick(report(&[Control::R3], CENTRE), true);
    one.tick(report(&[], CENTRE), true);
    let (c, p) = (&one.cam.cam, &one.plain.cam);
    assert_eq!([c.user_zoom, c.user_yaw, c.user_pitch, c.user_fov], [F::ZERO; 4]);
    assert_eq!(
        [c.yaw, c.pitch, c.distance].map(|f| f.0.to_bits()),
        [p.yaw, p.pitch, p.distance].map(|f| f.0.to_bits()),
        "R3 did not put the camera back on its preset"
    );
}

#[test]
fn the_nudges_keep_the_originals_rules_through_a_preset_change() {
    let mut cam = camera();
    for _ in 0..5 {
        cam.nudge(F(-1.0), F::ZERO, false);
    }
    let turned = cam.cam.user_yaw;
    // Through the look-back and out again, the nudges come back.
    cam.cam.set_preset(2);
    cam.cam.set_preset(0);
    assert_eq!(cam.cam.user_yaw, turned, "the look-back lost the nudges");
    // The camera button, between two ordinary presets, throws them away.
    cam.cam.next_preset();
    assert_eq!(cam.cam.preset().name, "Race Cam 2");
    assert_eq!(cam.cam.user_yaw, F::ZERO, "a preset change kept the nudges");
}

#[test]
fn the_rides_ticks_are_identical_with_and_without_the_stick() {
    // The same reports into two rides, one with the camera reading them and one without: the state
    // hash (the pad's own state in it) is the same every tick. And a third ride on a centred stick
    // and no L3 or R3: the bike goes where it goes on the other two, because nothing in the ride
    // reads the right stick or the two clicks.
    let (mut stick, mut still, mut centred) = (Seat::new(), Seat::new(), Seat::new());
    let place = |s: &Seat| {
        let m = s.sim.rider.ride.transform();
        m.0.map(|row| [row.x(), row.y(), row.z(), row.w()].map(f32::to_bits))
    };
    for k in 0..300usize {
        // The stick round in circles, L3 held for a stretch, R3 clicked now and then.
        let right = [FULL_RIGHT, FULL_DOWN, (0x00, 0x7f), (0x7f, 0x00)][(k / 20) % 4];
        let mut held = Vec::new();
        if (100..160).contains(&k) {
            held.push(Control::L3);
        }
        if k % 50 == 49 {
            held.push(Control::R3);
        }
        stick.tick(report(&held, right), true);
        still.tick(report(&held, right), false);
        centred.tick(report(&[], CENTRE), false);
        assert_eq!(stick.sim.hash(), still.sim.hash(), "tick {k}: the camera reached the ride");
        assert_eq!(place(&stick), place(&centred), "tick {k}: the right stick reached the bike");
    }
    let speed = stick.sim.rider.ride.chassis.speed;
    assert!(speed > 10.0, "the bike never got going: {speed:?}");
}

/// P78: L2 held is the look-back preset, and letting go goes back to the player's own preset, the
/// nudges kept through both as the original's preset switch keeps them; a press while a wreck's
/// camera has the screen, or on a camera whose file has no look-back preset, does nothing.
#[test]
fn the_look_back_command_holds_the_look_back_preset() {
    let mut one = Seat::new();
    // The player's own preset is the second one, walked to, and the stick has turned it.
    one.cam.cam.next_preset();
    for _ in 0..10 {
        one.tick(report(&[], FULL_RIGHT), true);
    }
    let turned = one.cam.cam.user_yaw;
    assert_ne!(turned, F::ZERO);
    assert_eq!(one.cam.cam.look_back_preset(), Some(2));
    for k in 0..20 {
        one.tick(report(&[Control::L2], CENTRE), true);
        assert_eq!(one.cam.cam.preset, 2, "tick {k}: the look-back is up while L2 is held");
        assert!(one.cam.cam.preset().is_look_back_cam);
    }
    // The nudges were parked, not thrown away: let go, the player's preset with its turn.
    one.tick(report(&[], CENTRE), true);
    assert_eq!(one.cam.cam.preset, 1, "letting go goes back to the player's preset");
    assert_eq!(one.cam.cam.user_yaw.0.to_bits(), turned.0.to_bits(), "the turn came back with it");
    // Held again from the first preset, it comes back to the first.
    one.cam.cam.next_preset();
    assert_eq!(one.cam.cam.preset, 0);
    one.tick(report(&[Control::L2], CENTRE), true);
    assert_eq!(one.cam.cam.preset, 2);
    one.tick(report(&[], CENTRE), true);
    assert_eq!(one.cam.cam.preset, 0);
    // A wreck's camera on the screen takes no press.
    one.on_screen = false;
    one.tick(report(&[Control::L2], CENTRE), true);
    assert_eq!(one.cam.cam.preset, 0, "a press under the wreck camera");
    one.tick(report(&[], CENTRE), true);
    one.on_screen = true;
    // Nothing but L2 is the look-back under the shipped table.
    for c in [Control::R2, Control::Square, Control::Select, Control::L1] {
        one.tick(report(&[c], CENTRE), true);
        assert_ne!(one.cam.cam.preset, 2, "{c:?}");
        one.tick(report(&[], CENTRE), true);
    }
    // A file with no look-back preset has nothing to look back on.
    let mut two = Seat::new();
    two.cam = Race::new(vec![preset("Race Cam 1", false), preset("Race Cam 2", false)], F(REVERSE_SPEED));
    two.tick(report(&[Control::L2], CENTRE), true);
    assert_eq!(two.cam.cam.preset, 0);
    assert_eq!(two.cam.cam.look_back_preset(), None);
}
