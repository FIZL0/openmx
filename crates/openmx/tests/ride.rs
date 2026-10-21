//! P10 — a minute you can ride, through the shell's own pad.
//!
//! Everything this shell is tested on is a part of it: the pad's bytes ([`pad`]'s own unit tests), the
//! fixed update's pacing, the renderer's placement. Nothing says the bike it puts on the screen can be
//! **ridden** — the shell would start, draw, pace itself and record a replay with a bike that never
//! moved an inch, and every test in the workspace would stay green.
//!
//! So this rides one. `tests/ride-minute.pad` is a minute written in the kit's pad notation — the
//! notation `./kit pad script` sends to a real PCSX2 — and this file reads it frame by frame, builds
//! the DualShock 2 reports the shell's own pad would have built out of it, and hands them to
//! [`game_sim::Sim`] exactly the way `src/main.rs`'s fixed update does: `poll` for the controls, then
//! `step` for the frame. Ten legs, 3600 frames, and each leg asserts what a rider would notice:
//!
//!     the launch                 2 s   it goes
//!     flat out                  20 s   it reaches a speed and keeps it
//!     turning right              4 s   it turns the way the stick is pushed
//!     straight after the right   2 s
//!     turning left               4 s   and the other way
//!     straight after the left    2 s
//!     the run-up                 3 s
//!     the jump                   5 s   it leaves the ground and lands on its wheels
//!     the brake                 10 s   it stops, and stays stopped
//!     stopped                    8 s
//!
//! (P16 added an eleventh, a flip off a bigger ramp, and P30 moved it to a ride of its own on the
//! disc's bike — see below.)
//!
//! It is headless: no window, no device, no disc. The bike is [`game_sim::stock::BIKE`], the stand-in
//! the simulation crate ships for exactly this (no number in it is the game's), with the two keys
//! [`ridden_bike`] patches; the world is a floor with a take-off ramp laid on it, twice.
//!
//! **The sibling file.** `crates/game-sim/tests/ride.rs`'s section 7 rides the same ten legs from
//! [`Controls`] built by hand. The two are deliberately not one test: that one asks whether the
//! *simulation* can be ridden, this one asks whether the simulation can be ridden **through a pad**,
//! and between the two sit the report bytes, the input layer's dead zone and rate limits, and
//! `Controls::from_pad`'s binding — three layers a bike can be lost in without either end being wrong.
//! What is duplicated between them is the ramp, the leg list and the measuring helpers, because
//! `openmx` has no library target for them to live in and neither crate may `include_str!` the other's
//! fixtures (`game_sim::stock`'s own head says why).
//!
//! **Every leg of the minute passes here now.** Two of them shipped `#[ignore]`d with the defect
//! named in the test, and both were the simulation's rather than this file's: the steering command
//! never reaching the front wheel, which P11 and P12 between them paid, and the brake never reaching
//! the chassis, which P13 paid. Each was repeated here as well as in the sibling file, because a leg
//! of a minute is not proved by a leg of another minute, and because this is the file whose failure
//! would be a player's complaint.
//!
//! **And one test that is not a leg of the minute**, for the same reason:
//! `it_pulls_away_after_a_frame_off_the_throttle` starts from an idle pad rather than from `down
//! cross` on frame zero, which is the state a session actually begins in. The clutch the idle rule
//! pulls in there was a second copy of the engine's clutch word that nothing walked back out, so
//! the bike in the shell would not move at any throttle; P11 paid it.
//!
//! **And the flip is ridden twice** (P16): once off its script's stick, like every leg of the
//! minute, and once more on the shell's own arrow keys — `src/pad.rs`'s `Pads` and its key ramp,
//! which is why this file compiles that module and its window in by path. The keyboard is the control
//! most of this shell's players have, and a flip the pad can do and the keys cannot is the complaint
//! P16 was opened for. Since P30 the flip is not a leg of the minute: it is `tests/ride-flip.pad`,
//! ridden on the disc's `race125p.phy`, because the stand-in bike's flip was this file's own numbers
//! and stopped meaning anything once the backflip read its stick the way the original does. Without
//! the disc it skips.
//!
//! **And the biplane switch** (P19), at the bottom: the shell's `--vehicle` and its swap key go
//! through `src/vehicle.rs`'s `Garage`, and the three things it must do are asserted on the same pad
//! script — a session that starts in the plane has the plane up and leaves the bike alone, a swap
//! mid-session moves the controller and back, and a default session rides the bike bit for bit as it
//! did before there was a garage. Whether the plane then *flies* is S34's flight, which is not ported;
//! that test is here, ignored, with S34's name on it.
//!
//! **And the chase camera through the flip** (P34): `the_camera_stays_behind_the_backflip` rides the
//! same flip with the shell's race camera on it, handed its target by `src/chase.rs` as the shell
//! hands it, and holds the view behind the bike from the lip to the landing.

use game_sim::collide::{Bundle, Record};
use game_sim::input::{AXIS_CENTRE, CONTROL_MAP, Control, PADS, Report};
use game_sim::phy::Phy;
use game_sim::rider::Tables;
use game_sim::ride::Controls;
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::sync::Arc;

// The shell's own keyboard pad, for the flip's second ride. This crate is a binary, so a test cannot
// `use` it; both files are std and `game_sim` only, and are compiled into this test as they stand.
// Their own unit tests come with them and run here a second time, which is the price of not moving
// either file.
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;
// And the shell's machine switch (P19), for the same reason and in the same way.
#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
// And what the shell hands its chase camera (P34), which reads the garage above.
#[path = "../src/chase.rs"]
mod chase;

/// The minute, in the kit's pad notation. A file rather than a string const so that a human can send
/// the same ride to a real machine with `./kit pad script`.
const MINUTE: &str = include_str!("ride-minute.pad");

/// P16's flip on the disc's bike, in the same notation: one leg, `the flip` (P30).
const FLIP: &str = include_str!("ride-flip.pad");

/// The frame the shell's fixed update runs at, and the one the script's `f` suffix counts.
const DT: F = F(1.0 / 60.0);

/// The legs the script has to come to, in order: the names and their frame counts. The script is the
/// source of both — this is here so that an edit to one and not the other fails instead of quietly
/// asserting about a different ride.
const LEGS: [(&str, usize); 10] = [
    ("the launch", 120),
    ("flat out", 1200),
    ("turning right", 240),
    ("straight after the right", 120),
    ("turning left", 240),
    ("straight after the left", 120),
    ("the run-up", 180),
    ("the jump", 300),
    ("the brake", 600),
    ("stopped", 480),
];

/// How far ahead of the bike the take-off ramp goes, and its shape: a rise over a run, in feet. The
/// same ramp as the sibling file's.
const RAMP: (f32, f32, f32) = (250.0, 0.3, 15.0);

/// The flip's ramp, the same three numbers — the sibling file's `FLIP_RAMP`, which says why it is
/// nearer, steeper and longer than [`RAMP`]. Laid along +z from the origin, where [`ride_the_flip`]
/// starts the bike.
const FLIP_RAMP: (f32, f32, f32) = (108.0, 0.5, 25.0);

/// The top speed the human's recording of the original reaches, in feet a second of the chassis —
/// measured in `crates/game-sim/tests/ride.rs`, which is where the capture is read and this number is
/// checked back against it (75.78 until P28, which was the capture's 0.8x copy of the chassis'
/// velocity read raw). Here it is the scale the legs' own tolerances are written in.
const CAPTURED_TOP_SPEED: f32 = 94.73;

/// The slowest of the original's four standing launches across both recordings, half a second and
/// two seconds in — the sibling file's `ORIGINAL_LAUNCH`, with the measurement written up there (P15,
/// restated in the chassis' units by P28).
const ORIGINAL_LAUNCH: (f32, f32) = (11.0, 42.75);

/// How much of [`ORIGINAL_LAUNCH`] this bike is held to — the sibling file's `LAUNCH_SHARE`, which
/// says why it is not all of it.
const LAUNCH_SHARE: (f32, f32) = (0.85, 0.8);

/// The wreck limits the minute is judged against, appended to the bike: the limits of a bike being
/// *ridden*, so a hard landing and a hard lean are not crashes and coming down on its side still is.
/// The same block as the sibling file's `RIDE_LIMITS`, and plausible numbers rather than the disc's.
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

/// [`stock::BIKE`], made rideable — in memory, because that text is the whole workspace's stand-in and
/// moving a number in it would move other crates' tests.
///
/// Two patches, both explained at length by the sibling file's `ridden_bike`: the `[Joystick]` section
/// has no `SteerScale*`/`SteerSpeed*` keys, so the steering scale loads as zero and the stick is not
/// connected to anything at all; and `Drag = 0.5` holds this bike to about 48 ft/s, which is nothing
/// like the 94.73 the human's recording reaches (0.037 meets it; P28 took it from 0.06, which met
/// the recording's 0.8x copy of the speed), so the minute's speeds would mean nothing. Neither
/// patch is a claim about the disc's file.
///
/// **P12 moved two things, and both are the sibling file's `ridden_bike` catching up.** The steering
/// fade's far end is `0.05` and not the `0.25` this file shipped with: 0.25 asks the tires for five
/// and a half g at the top speed, and a bike asked for grip it has not got slides instead of turning.
/// And `BodyTireForcePosition` is P8's tuning arm here as well — the sibling's third patch, which this
/// file never got because nothing had ever steered this bike and the difference did not show. Read as
/// the drive and tire force's arm, the file's own value is a foot *above* the body origin and two feet
/// behind the axle, which is both a nose-diving launch (P8) and a yaw arm two and a half times the
/// sibling's: on the unpatched bike the stick at 0.05 of the lock spun the minute a full turn and a
/// quarter in four seconds. The mounts are kept off the *unpatched* text, which is what
/// [`ride_the_minute`] builds the rider with and what the sibling file's harness does.
///
/// The two files' bikes have to agree, because `the_pad_script_is_the_minute` is what says the shell
/// rides the same minute `game-sim` does.
fn ridden_bike() -> String {
    let mut out = stock::BIKE.to_string();
    let patches = [
        (
            "XAxisRateLimit = 6\n",
            "XAxisRateLimit = 6\nSteerScale1 = 1.0\nSteerScale2 = 0.05\nSteerSpeed1 = 5\nSteerSpeed2 = 50\n",
        ),
        ("Drag = 0.5", "Drag = 0.037"),
        ("BodyTireForcePosition = 0.0, 1.1, 2.1", "BodyTireForcePosition = 0.0, -0.3, 0.8"),
        ("BodyTireForcePosition = 0.0, 1.1, -2.1", "BodyTireForcePosition = 0.0, -0.3, -0.8"),
    ];
    for (key, with) in patches {
        assert!(out.contains(key), "game_sim::stock::BIKE no longer has `{}`", key.trim_end());
        out = out.replace(key, with);
    }
    format!("{out}{RIDE_LIMITS}")
}

// ---------------------------------------------------------------------------------------------------
// the pad
// ---------------------------------------------------------------------------------------------------

/// The four stick slots of a report, in [`Report::set_axis`]'s own order — `src/pad.rs`'s constants,
/// repeated here because this crate is a binary and a test cannot reach into it.
const REPORT_RIGHT_X: usize = 0;
const REPORT_RIGHT_Y: usize = 1;
const REPORT_LEFT_X: usize = 2;
const REPORT_LEFT_Y: usize = 3;

/// A joystick axis as the stick byte a DualShock 2 reports: centred at `0x7f`, full travel at 0 and
/// 255. `src/pad.rs`'s `stick_byte`, and it has to stay the same function — the port's dead zone and
/// its one inverted axis are measured against this sense.
fn stick_byte(value: i16) -> u8 {
    let centred = (value as i32 * 128) / 32767;
    (AXIS_CENTRE as i32 + centred).clamp(0, 255) as u8
}

/// What the rider is holding at one instant: the buttons that are down and where the two sticks are.
#[derive(Clone, Copy, Default)]
struct Hands {
    buttons: u16,
    left: (i16, i16),
    right: (i16, i16),
}

impl Hands {
    /// These hands as the 32 bytes a pad would have sent — `Pads::report`'s own three steps, in its
    /// order: the button word active low, a full pressure byte for every analogue control held by a
    /// digital source, and the two sticks.
    fn report(&self) -> Report {
        let mut report = Report::idle();
        let low = !self.buttons;
        report.0[2] = (low >> 8) as u8;
        report.0[3] = low as u8;
        for binding in CONTROL_MAP {
            if binding.is_analogue() && self.buttons & binding.mask != 0 {
                report.0[binding.pressure as usize] = 0xff;
            }
        }
        report.set_axis(REPORT_LEFT_X, stick_byte(self.left.0));
        report.set_axis(REPORT_LEFT_Y, stick_byte(self.left.1));
        report.set_axis(REPORT_RIGHT_X, stick_byte(self.right.0));
        report.set_axis(REPORT_RIGHT_Y, stick_byte(self.right.1));
        report
    }
}

/// A button name out of the script, as the game's own control. The kit's own list, less `select`,
/// which it refuses to send.
fn button(name: &str) -> u16 {
    let control = match name {
        "cross" => Control::Cross,
        "circle" => Control::Circle,
        "square" => Control::Square,
        "triangle" => Control::Triangle,
        "start" => Control::Start,
        "l1" => Control::L1,
        "r1" => Control::R1,
        "l2" => Control::L2,
        "r2" => Control::R2,
        "l3" => Control::L3,
        "r3" => Control::R3,
        "up" => Control::Up,
        "down" => Control::Down,
        "left" => Control::Left,
        "right" => Control::Right,
        _ => panic!("the script names a button the pad does not have: `{name}`"),
    };
    control.mask()
}

/// A duration out of the script, in frames. `120f` is 120 frames; a bare number is milliseconds, which
/// is the kit's own default and is rounded to the nearest frame here because a frame is all a
/// simulation has.
fn frames(text: &str) -> usize {
    match text.strip_suffix('f') {
        Some(count) => count.parse().unwrap_or_else(|_| panic!("`{text}` is not a frame count")),
        None => {
            let ms: f32 = text
                .trim_end_matches("ms")
                .parse()
                .unwrap_or_else(|_| panic!("`{text}` is not a duration"));
            (ms * 60.0 / 1000.0).round() as usize
        }
    }
}

/// A stick position out of the script: −1 to 1, with `y` down positive, as the kit sends it.
fn stick(text: &str) -> i16 {
    let value: f32 = text.parse().unwrap_or_else(|_| panic!("`{text}` is not a stick position"));
    (value.clamp(-1.0, 1.0) * 32767.0) as i16
}

/// Read the script into the reports it sends, leg by leg.
///
/// The subset of the notation the minute uses, which is all this reader claims to implement:
/// `down`/`up <button>`, `press <button> <ms>`, `dpad <dir> <ms>`, `stick <l|r> <x> <y>`,
/// `sleep`/`wait <ms>`, `release`, `;` between commands on a line, and `#` comments — of which
/// `# leg: <name>` starts a leg. Anything else is a panic rather than a silent skip: a script this
/// reader half-understands is a ride nobody has checked.
fn script(text: &str) -> Vec<(String, Vec<Report>)> {
    let mut legs: Vec<(String, Vec<Report>)> = Vec::new();
    let mut hands = Hands::default();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("# leg:") {
            legs.push((name.trim().to_string(), Vec::new()));
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        for command in line.split(';') {
            let words: Vec<&str> = command.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            let leg = match legs.last_mut() {
                Some(leg) => &mut leg.1,
                None => panic!("`{command}` comes before the script's first `# leg:` line"),
            };
            let mut hold = |hands: &Hands, count: usize| {
                let report = hands.report();
                leg.extend(std::iter::repeat_n(report, count));
            };
            match words.as_slice() {
                ["down", name] => hands.buttons |= button(name),
                ["up", name] => hands.buttons &= !button(name),
                // A press is a down, a wait and an up — the kit's own reading of it.
                ["press" | "dpad", name, ms] => {
                    hands.buttons |= button(name);
                    hold(&hands, frames(ms));
                    hands.buttons &= !button(name);
                }
                ["stick", side, x, y] => {
                    let at = (stick(x), stick(y));
                    match *side {
                        "l" => hands.left = at,
                        "r" => hands.right = at,
                        _ => panic!("`{side}` is neither stick"),
                    }
                }
                ["sleep" | "wait", ms] => hold(&hands, frames(ms)),
                ["release"] => hands = Hands::default(),
                _ => panic!("the script says `{command}`, which this reader does not understand"),
            }
        }
    }
    legs
}

// ---------------------------------------------------------------------------------------------------
// the world
// ---------------------------------------------------------------------------------------------------

/// Flat ground with one take-off ramp laid across the rider's path: level everywhere except a plane
/// rising at `slope` for `length` feet, `start` feet along `dir` from `origin`.
///
/// The sibling file's `Jump`, repeated for the reason the file's head gives. A jump is the one leg of a
/// minute flat ground cannot produce, and the bike leaves the lip because the ground stops rising under
/// it rather than because anything here tells it to.
#[derive(Clone)]
struct Jump {
    origin: Vf,
    dir: Vf,
    start: f32,
    slope: f32,
    length: f32,
}

impl Jump {
    /// Nowhere near a ramp: what the minute rides over until the run-up puts one in front of it.
    fn nothing() -> Self {
        Jump {
            origin: Vf::default(),
            dir: Vf::new(0.0, 0.0, 1.0, 0.0),
            start: 0.0,
            slope: 0.0,
            length: 0.0,
        }
    }

    fn along(&self, at: Vf) -> f32 {
        (at.x() - self.origin.x()) * self.dir.x() + (at.z() - self.origin.z()) * self.dir.z()
    }
}

impl World for Jump {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let t = self.along(at) - self.start;
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        if self.length <= 0.0 || t < 0.0 || t > self.length {
            return (Vf::new(at.x(), 0.0, at.z(), 1.0), up);
        }
        let n = 1.0 / (1.0 + self.slope * self.slope).sqrt();
        (
            Vf::new(at.x(), t * self.slope, at.z(), 1.0),
            Vf::new(-self.slope * self.dir.x() * n, n, -self.slope * self.dir.z() * n, 0.0),
        )
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------------------------------
// the ride
// ---------------------------------------------------------------------------------------------------

/// What the rider sees out of one frame, plus what the pad chain made of the script that frame: the
/// throttle, the brake and the stick as [`game_sim::ride::Controls::from_pad`] left them.
struct Seat {
    speed: f32,
    heading: f32,
    up_y: f32,
    y: f32,
    on_ground: bool,
    wreck: bool,
    gas: f32,
    brake: f32,
    stick_x: f32,
    /// The stick's y as the rider's hand put it — `Controls::from_pad`'s, before the bike's rate limit.
    stick_y: f32,
    /// The sibling file's four flip fields: the lip as `air.rs`'s `on_a_lip` reads it, sampled at
    /// the end of the frame; the preload shoving; a backflip triggered; and the nose's angle above the
    /// ground in the ramp's plane, the full circle.
    lip: bool,
    preload: bool,
    flip: bool,
    pitch: f32,
}

impl Seat {
    /// The frame `sim` has just stepped, as the rider saw it.
    fn of(sim: &Sim, controls: &Controls, wreck: bool, world: &Jump) -> Seat {
        let ride = &sim.rider.ride;
        let m = ride.transform();
        let f = m.0[2];
        Seat {
            speed: ride.chassis.speed.0,
            heading: f32::atan2(m.0[2].x(), m.0[2].z()),
            up_y: m.0[1].y(),
            y: ride.position().y(),
            on_ground: ride.air.on_ground,
            wreck,
            gas: controls.gas.0,
            brake: controls.brake.0,
            stick_x: controls.stick.0.0,
            stick_y: controls.stick.1.0,
            lip: ride.air.on_ground
                && ride.chassis.vel.y() > 5.0
                && ride.air.contact_vel.z() > 30.0
                && ride.held.brake == F::ZERO,
            preload: ride.air.preload_firing,
            flip: ride.air.flip_active,
            pitch: f32::atan2(f.y(), f.x() * world.dir.x() + f.z() * world.dir.z()),
        }
    }
}

/// The minute, leg by leg, as it was ridden.
struct Minute {
    legs: Vec<(String, Vec<Seat>)>,
}

impl Minute {
    fn leg(&self, name: &str) -> Vec<&Seat> {
        let found = self.legs.iter().find(|(n, _)| n == name);
        found.unwrap_or_else(|| panic!("the minute has no leg called '{name}'")).1.iter().collect()
    }

    /// Two legs' frames end to end, for the one thing that straddles a boundary: the jump.
    fn span(&self, first: &str, last: &str) -> Vec<&Seat> {
        let at = |name: &str| {
            self.legs
                .iter()
                .position(|(n, _)| n == name)
                .unwrap_or_else(|| panic!("the minute has no leg called '{name}'"))
        };
        self.legs[at(first)..=at(last)].iter().flat_map(|(_, seen)| seen).collect()
    }

    fn all(&self) -> Vec<&Seat> {
        self.legs.iter().flat_map(|(_, seen)| seen).collect()
    }
}

fn fastest(seen: &[&Seat]) -> f32 {
    seen.iter().map(|t| t.speed).fold(0.0, f32::max)
}

fn slowest(seen: &[&Seat]) -> f32 {
    seen.iter().map(|t| t.speed).fold(f32::MAX, f32::min)
}

/// The furthest from upright the bike got: the minimum of its transform's own up.
fn flattest(seen: &[&Seat]) -> f32 {
    seen.iter().map(|t| t.up_y).fold(f32::MAX, f32::min)
}

fn highest(seen: &[&Seat]) -> f32 {
    seen.iter().map(|t| t.y).fold(f32::MIN, f32::max)
}

fn airborne(seen: &[&Seat]) -> usize {
    seen.iter().filter(|t| !t.on_ground).count()
}

fn wrecks(seen: &[&Seat]) -> usize {
    seen.iter().filter(|t| t.wreck).count()
}

/// A signed angle brought back into ±π, so a heading that walks past the branch cut does not read as a
/// turn the other way.
fn wrap(a: f32) -> f32 {
    let turn = std::f32::consts::TAU;
    let mut a = a % turn;
    if a > std::f32::consts::PI {
        a -= turn;
    }
    if a < -std::f32::consts::PI {
        a += turn;
    }
    a
}

/// How far round the bike came over these frames, signed the way the stick is: positive to the right.
fn turned(seen: &[&Seat]) -> f32 {
    // Tick by tick, not end against start: four seconds of full lock comes round further than half a
    // circle and the difference of two headings cannot tell that from a short turn the other way.
    seen.windows(2).map(|w| wrap(w[1].heading - w[0].heading)).sum()
}

fn turn_rate(seen: &[&Seat]) -> f32 {
    turned(seen) / (seen.len() as f32 * DT.0)
}

/// How far over the bike went, backwards positive: [`turned`] for the nose.
fn came_over(seen: &[&Seat]) -> f32 {
    seen.windows(2).map(|w| wrap(w[1].pitch - w[0].pitch)).sum()
}

/// Ride [`MINUTE`] through the pad, the input layer and the simulation, and keep what the rider saw.
///
/// The three lines in the middle are the shell's fixed update: the frame's reports, `Sim::poll` for the
/// controls the input layer makes of them, `Sim::step` for the frame. Pad 1 is missing, which is what a
/// one-player session reports.
fn ride_the_minute() -> Minute {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    // The wheel mounts come off the *unpatched* text, because `ridden_bike`'s third patch moves the
    // key this crate reads as the mounts when the bike model has no wheel nodes — the sibling file's
    // harness keeps them the same way, and the two minutes have to be one bike. That is why the rider
    // is built here and handed over (`Sim::with_rider`, which is also how the shell itself builds one)
    // rather than through `Sim::ride`, which would take the mounts off the patched copy.
    //
    // The wreck files are the disc's and this ride has none, which is the state the shell runs in
    // without them — the machine still calls a wreck, there is just no body to tumble.
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let rider = game_sim::rider::Rider::new(
        &phy,
        stock::mounts(&geom),
        stock::SPAWN,
        stock::FACING,
        Set::default(),
    );
    let mut sim = Sim::with_rider(0, tables, phy, rider);
    let mut world = Jump::nothing();

    let mut out = Minute { legs: Vec::new() };
    for (name, reports) in script(MINUTE) {
        // The ramp is laid when the run-up starts, ahead of wherever the bike actually is and along
        // whatever heading it actually has — the jump has to work on the bike's own line.
        let ramp = match name.as_str() {
            "the run-up" => Some(RAMP),
            _ => None,
        };
        if let Some((start, slope, length)) = ramp {
            let m = sim.rider.ride.transform();
            world = Jump {
                origin: sim.rider.ride.position(),
                dir: Vf::new(m.0[2].x(), 0.0, m.0[2].z(), 0.0),
                start,
                slope,
                length,
            };
        }
        let mut seen = Vec::with_capacity(reports.len());
        for report in reports {
            let reports: [Report; PADS] = [report, Report::missing()];
            let controls = sim.poll(DT, &reports);
            let frame = sim.step(DT, &controls, &mut world);
            seen.push(Seat::of(&sim, &controls, frame.wreck.is_some(), &world));
        }
        out.legs.push((name, seen));
    }
    out
}

// ---------------------------------------------------------------------------------------------------
// the legs
// ---------------------------------------------------------------------------------------------------

/// The script is the minute it says it is, and the hands it writes down are the hands that arrive.
///
/// This is the leg the sibling file cannot have: between a `down cross` in a text file and a throttle
/// in the simulation sit the report's pressure bytes, the input layer's poll, its dead zone and its
/// rate limits, and `Controls::from_pad`'s binding. A minute of perfect physics driven by a pad that
/// delivers nothing is still a bike nobody can ride.
#[test]
fn the_pad_script_is_the_minute() {
    let legs = script(MINUTE);
    let read: Vec<(&str, usize)> =
        legs.iter().map(|(name, reports)| (name.as_str(), reports.len())).collect();
    assert_eq!(read, LEGS.to_vec(), "the script is not the ten legs the tests assert about");
    assert_eq!(
        read.iter().map(|(_, n)| n).sum::<usize>(),
        3600,
        "the script is not the minute at 60 frames a second"
    );

    let m = ride_the_minute();
    // The throttle: full by the end of the launch, and shut once the script lets go of it. `gas` is
    // the rider's hand and not the rate-limited command, so there is nothing to wait for but the pad.
    let launch = m.leg("the launch");
    assert_eq!(launch[0].gas, 1.0, "`down cross` did not reach the throttle on the first frame");
    assert_eq!(launch[launch.len() - 1].gas, 1.0, "the throttle let go during the launch");

    // The brake, which is the other analogue button, and the throttle let go in the same breath.
    let brake = m.leg("the brake");
    assert_eq!(brake[brake.len() - 1].brake, 1.0, "`down square` did not reach the brake");
    assert_eq!(brake[brake.len() - 1].gas, 0.0, "`up cross` did not let the throttle go");

    // `release` means hands off: no button, no stick.
    let stopped = m.leg("stopped");
    let last = stopped[stopped.len() - 1];
    assert_eq!((last.gas, last.brake), (0.0, 0.0), "`release` left a button down");

    // And the stick, which is the only control that travels: full lock each way, through the dead
    // zone's stretch, by the end of a four-second leg.
    let right = m.leg("turning right");
    let left = m.leg("turning left");
    assert!(
        right[right.len() - 1].stick_x > 0.9,
        "`stick l 1 0` reached the bike as {}",
        right[right.len() - 1].stick_x
    );
    assert!(
        left[left.len() - 1].stick_x < -0.9,
        "`stick l -1 0` reached the bike as {}",
        left[left.len() - 1].stick_x
    );
    let straight = m.leg("straight after the right");
    assert!(
        straight[straight.len() - 1].stick_x.abs() < 0.05,
        "`stick l 0 0` left the bars at {}",
        straight[straight.len() - 1].stick_x
    );
}

#[test]
fn it_launches_from_a_standstill() {
    let m = ride_the_minute();
    let launch = m.leg("the launch");
    assert!(launch[0].speed < 1.0, "the launch did not start from rest: {} ft/s", launch[0].speed);

    // Half a second of full throttle: a rider on a 250 is moving by now.
    let half = launch[(0.5 / DT.0) as usize].speed;
    assert!(
        half > ORIGINAL_LAUNCH.0 * LAUNCH_SHARE.0,
        "half a second of full throttle moved the bike at {half} ft/s, against the original's {}",
        ORIGINAL_LAUNCH.0
    );

    // Two seconds: most of the way to the original's slowest launch, and still on its wheels.
    let two = launch[launch.len() - 1].speed;
    assert!(
        two > ORIGINAL_LAUNCH.1 * LAUNCH_SHARE.1,
        "two seconds of full throttle only reached {two} ft/s, against the original's {}",
        ORIGINAL_LAUNCH.1
    );
    assert!(flattest(&launch) > 0.9, "the launch leant to up {}", flattest(&launch));
    assert_eq!(wrecks(&launch), 0, "the launch wrecked");
}

#[test]
fn it_reaches_the_top_speed_and_holds_it() {
    let m = ride_the_minute();
    let flat_out = m.leg("flat out");
    let top = fastest(&flat_out);
    assert!(
        top > CAPTURED_TOP_SPEED * 0.9,
        "twenty seconds of full throttle reached {top} ft/s, against the original's {CAPTURED_TOP_SPEED}"
    );
    assert!(
        top < CAPTURED_TOP_SPEED * 1.1,
        "the bike runs away to {top} ft/s, past the original's {CAPTURED_TOP_SPEED}"
    );

    // And keeps it: over the last quarter of the leg the speed never falls away from the peak.
    let tail = &flat_out[flat_out.len() * 3 / 4..];
    assert!(
        slowest(tail) > top * 0.97,
        "the speed fell to {} ft/s after topping out at {top}",
        slowest(tail)
    );
    assert_eq!(wrecks(&flat_out), 0, "the bike wrecked flat out");
}

/// The stick goes hard over and the bike comes round — through the shell's own pad, which is the half
/// of the turn this crate can prove and the sibling file cannot.
///
/// P10 shipped this ignored: the simulation never put the steering command on the hinge, so the bike
/// could not turn however the pad was waggled. P11 bridged the command onto the hinge in `ride.rs`'s
/// `Ride::frame`; **P12 then moved the bridge to where the aliasing is.** The ground step's commanded
/// angle and the steering hinge's own angle are one word on the wheel in the original, and `ride.rs`
/// now copies it both ways round the ported call — in `Wheel::to_wheel`/`Wheel::from_wheel` — instead
/// of leaving the hinge at zero. The sibling file's version of this test has the long version.
#[test]
fn it_turns_the_way_the_stick_is_pushed() {
    let m = ride_the_minute();
    for (leg, sign) in [("turning right", 1.0), ("turning left", -1.0)] {
        let seen = m.leg(leg);
        let rate = turn_rate(&seen) * sign;
        // A floor at what the fixture carries, not the original's 0.6 to 2.5 (P15, the sibling file).
        assert!(rate > 0.2, "{leg}: the bike turned at {rate} rad/s the way the stick was pushed");
        assert!(
            turned(&seen).abs() > 0.2,
            "{leg}: four seconds of full lock came round {} rad",
            turned(&seen)
        );
        // Still rolling at the end rather than a speed floor: full lock with the throttle pinned at
        // this bike's top speed scrubs most of the speed off, which is the simulation's business and
        // is written up in the sibling file.
        let end = seen[seen.len() - 1].speed;
        assert!(end > 2.0, "{leg}: the turn left the bike at {end} ft/s");
        assert!(flattest(&seen) > 0.5, "{leg}: the bike leant to up {}", flattest(&seen));
        assert_eq!(wrecks(&seen), 0, "{leg}: the turn wrecked");
    }
    let (right, left) = (m.leg("turning right"), m.leg("turning left"));
    assert!(
        turned(&right) > 0.0 && turned(&left) < 0.0,
        "both turns went the same way: {} rad and {} rad",
        turned(&right),
        turned(&left)
    );
}

#[test]
fn it_takes_the_jump_and_lands_on_its_wheels() {
    let m = ride_the_minute();
    let jump = m.span("the run-up", "the jump");
    let air = airborne(&jump);
    assert!(air > 70, "the ramp only put the bike in the air for {air} frames");
    assert!(
        highest(&jump) > RAMP.1 * RAMP.2 + 1.0,
        "the jump's highest point was {} ft, which is no higher than the lip",
        highest(&jump)
    );
    assert!(flattest(&jump) > 0.6, "in the air the bike got to up {}", flattest(&jump));

    // And down: landed with a second of the leg still to run, upright, still rolling, never having
    // wrecked. From the last airborne frame rather than the leg's own last second, because where in
    // the leg the jump falls depends on how fast the bike reaches the ramp — and now that the turns
    // before it really turn, it gets there later than P10's bike did.
    let last_air = jump.iter().rposition(|t| !t.on_ground).expect("the bike never left the ground");
    let landed = &jump[last_air + 1..];
    // The impact and the ride away from it are asserted apart: through the impact the bound is the
    // flight's own "not on its head", and from `SETTLE` to the end of the leg the bike must be level
    // and stay level. The split is the P8+P11 merge's; the sibling file's copy carries the three
    // measurements and why it is worth a human's eye. Here it touches down at up 0.675 and is level
    // by the eighth frame.
    const SETTLE: usize = 15; // a quarter of a second; this landing takes 8 frames.
    assert!(
        landed.len() > SETTLE + 30,
        "the bike came down with only {} frames of the leg left to settle in",
        landed.len()
    );
    assert!(
        flattest(&landed[..SETTLE]) > 0.6,
        "it came down on its head, at up {}",
        flattest(&landed[..SETTLE])
    );
    assert!(
        flattest(&landed[SETTLE..]) > 0.9,
        "a quarter of a second after it landed it was still at up {}",
        flattest(&landed[SETTLE..])
    );
    assert!(
        slowest(landed) > CAPTURED_TOP_SPEED * 0.65,
        "the landing left the bike at {} ft/s",
        slowest(landed)
    );
    assert_eq!(wrecks(&jump), 0, "the jump wrecked");
}

/// A flip, held to what a rider would see: both gestures went off on the lip, the bike came once
/// round backwards, and it rode away on its wheels. The frame the backflip fired, for the keyboard's
/// ride to be compared with.
fn assert_it_flipped(how: &str, flip: &[&Seat]) -> usize {
    let lip = flip.iter().filter(|t| t.lip).count();
    assert!(lip > 20, "{how}: the flip's ramp only held the lip for {lip} frames");
    let shoved = flip.iter().position(|t| t.preload);
    let shoved = shoved.unwrap_or_else(|| panic!("{how}: the preload never fired"));
    assert!(flip[shoved].lip, "{how}: the preload fired at frame {shoved} of the leg, off the lip");
    let fired = flip.iter().position(|t| t.flip);
    let fired = fired.unwrap_or_else(|| panic!("{how}: the backflip never fired"));
    assert!(flip[fired].lip, "{how}: the backflip fired at frame {fired} of the leg, off the lip");
    assert!(shoved < fired, "{how}: the backflip (frame {fired}) went before the preload ({shoved})");

    let air = airborne(&flip[fired..]);
    assert!(air > 30, "{how}: after the backflip fired the bike was only {air} frames in the air");
    let (over, turn) = (came_over(&flip[fired..]), std::f32::consts::TAU);
    assert!((over - turn).abs() < turn / 4.0, "{how}: the bike came {:.2} turns over", over / turn);

    // The jump's two-part landing ([`it_takes_the_jump_and_lands_on_its_wheels`]).
    const SETTLE: usize = 15;
    let last_air = flip.iter().rposition(|t| !t.on_ground).expect("the bike never left the ground");
    let landed = &flip[last_air + 1..];
    assert!(
        landed.len() > SETTLE + 30,
        "{how}: the bike came down with only {} frames of the leg left to settle in",
        landed.len()
    );
    assert!(
        flattest(&landed[..SETTLE]) > 0.6,
        "{how}: it came down on its head, at up {}",
        flattest(&landed[..SETTLE])
    );
    assert!(
        flattest(&landed[SETTLE..]) > 0.9,
        "{how}: a quarter of a second after it landed it was still at up {}",
        flattest(&landed[SETTLE..])
    );
    assert!(slowest(landed) > 20.0, "{how}: the landing left the bike at {} ft/s", slowest(landed));
    assert_eq!(wrecks(flip), 0, "{how}: the flip wrecked");
    fired
}

/// The disc's `race125p.phy` and where the `125_faisst` model hangs its wheels — the sibling file's
/// `the_disc_125`, repeated for the reason the file's head gives. `None` when the disc is not here.
fn the_disc_125(tables: &Tables) -> Option<(Phy, (Vf, Vf))> {
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
    let mut member = |name: &str| -> Option<Vec<u8>> {
        let e = psr.find(name)?;
        file.seek(SeekFrom::Start(e.at)).ok()?;
        let mut bytes = vec![0u8; e.size as usize];
        file.read_exact(&mut bytes).ok()?;
        Some(bytes)
    };
    let leaf = |res: &Res, want: &str| {
        res.entries
            .iter()
            .find(|e| {
                let n = e.name.to_ascii_lowercase();
                n == want || n.ends_with(&format!("\\{want}"))
            })
            .cloned()
    };
    let archives: Vec<String> = psr
        .entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let (mut phy, mut wheels) = (None, None);
    for archive in &archives {
        let Some(bytes) = member(archive) else { continue };
        let Ok(res) = Res::parse(&bytes) else { continue };
        if phy.is_none() {
            if let Some(e) = leaf(&res, "race125p.phy") {
                phy = res.read(&bytes, &e).ok().map(|b| Phy::read_bytes(&b, &tables.ride.curve));
            }
        }
        if wheels.is_none() && archive.to_ascii_lowercase().contains("125_faisst") {
            let Some(e) = leaf(&res, "bike.psg") else { continue };
            let Ok(psg) = res.read(&bytes, &e) else { continue };
            let Ok(model) = game_render::psg::Model::parse(&psg) else { continue };
            let node = |name: &str| -> Option<Vf> {
                let i = model.nodes.iter().position(|n| n.name == name)?;
                let mut root = i;
                while let Some(up) = model.nodes[root].parent {
                    root = up;
                }
                Some(Vf(model.place_in(i, root)))
            };
            wheels = node("TireFront").zip(node("TireRear"));
        }
    }
    Some((phy?, wheels?))
}

/// Ride [`FLIP`] on the disc's 125 from a standstill at the origin, facing +z up [`FLIP_RAMP`], and
/// keep what the rider saw. With `keys`, the left stick's y is not the script's but the shell's
/// own [`pad::Pads`] with the arrow keys held where the script has the stick — `up` where it is
/// pushed forward, `down` where it is pulled back — which since P30 is the d-pad. `None` without the
/// disc.
fn ride_the_flip(keys: Option<&mut pad::Pads>) -> Option<Vec<Seat>> {
    ride_the_flip_watched(keys, &mut |_, _| {})
}

/// [`ride_the_flip`], with `watch` handed the simulation and the ramp after every frame — which is
/// where the shell runs its camera.
fn ride_the_flip_watched(
    mut keys: Option<&mut pad::Pads>,
    watch: &mut dyn FnMut(&Sim, &mut Jump),
) -> Option<Vec<Seat>> {
    let tables = Arc::new(Tables::build());
    let (phy, wheels) = the_disc_125(&tables)?;
    let at = Vf::new(0.0, 2.0, 0.0, 1.0);
    let rider = game_sim::rider::Rider::new(&phy, wheels, at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let mut sim = Sim::with_rider(0, tables, phy, rider);
    let (start, slope, length) = FLIP_RAMP;
    let dir = Vf::new(0.0, 0.0, 1.0, 0.0);
    let mut world = Jump { origin: Vf::new(0.0, 0.0, 0.0, 1.0), dir, start, slope, length };
    let legs = script(FLIP);
    assert_eq!(legs.len(), 1, "ride-flip.pad is one leg");
    let mut seen = Vec::with_capacity(legs[0].1.len());
    for &report in &legs[0].1 {
        let mut report = report;
        if let Some(keys) = keys.as_deref_mut() {
            // The arrows are the d-pad (`pad::KEYBOARD`): the stick goes back to centre and the
            // keys' d-pad bits and pressures go into the script's report, which keeps its throttle.
            let y = report.axis(REPORT_LEFT_Y);
            let (up, down) = (y < AXIS_CENTRE - 64, y > AXIS_CENTRE + 64);
            let held = |code: u8| (code == window::key::UP && up) || (code == window::key::DOWN && down);
            let from_keys = keys.build(DT.0, &held);
            let word = |r: &Report| !(u16::from(r.0[2]) << 8 | u16::from(r.0[3]));
            let dpad = word(&from_keys) & (Control::Up.mask() | Control::Down.mask());
            let low = !(word(&report) | dpad);
            report.0[2] = (low >> 8) as u8;
            report.0[3] = low as u8;
            for binding in CONTROL_MAP {
                if binding.is_analogue() && binding.mask & dpad != 0 {
                    report.0[binding.pressure as usize] = from_keys.0[binding.pressure as usize];
                }
            }
            report.set_axis(REPORT_LEFT_Y, AXIS_CENTRE);
        }
        let reports: [Report; PADS] = [report, Report::missing()];
        let controls = sim.poll(DT, &reports);
        let frame = sim.step(DT, &controls, &mut world);
        seen.push(Seat::of(&sim, &controls, frame.wreck.is_some(), &world));
        watch(&sim, &mut world);
    }
    Some(seen)
}

/// P16 and P30: pump the lip and flip off it on the disc's 125, through the pad and then on the
/// keyboard.
///
/// The script is the sibling file's `THE_FLIP`, frame for frame, and it is asserted the same way.
/// Then it is ridden again on the arrow keys through the shell's own [`pad::Pads`]
/// ([`ride_the_flip`]).
///
/// What the keys have to get right changed with P30. Before it every stick y the backflip read had
/// been walked toward the hand at `YAxisRateLimit`, which the arrows' key ramp never got ahead of,
/// so the keys fired on the stick's frames by construction. The original reads the stick the
/// gesture fires on raw and sizes the push by how fast the thumb crossed it, and on that reading
/// the ramp could not fit three crosses on the 125's lip: the preload never fired. The arrows are
/// the d-pad now, the original's own digital stick, and this is what holds that to being a control
/// a rider can flip on.
#[test]
fn it_flips_off_the_lip() {
    let Some(ridden) = ride_the_flip(None) else { return };
    let flip: Vec<&Seat> = ridden.iter().collect();
    let by_stick = assert_it_flipped("on the stick", &flip);
    // The stick did reach the hand's stops, so the crosses above were the hand's and not a stick the
    // dead zone kept short.
    let reach = flip.iter().map(|t| t.stick_y).fold((0.0f32, 0.0f32), |(lo, hi), y| (lo.min(y), hi.max(y)));
    assert!(reach.0 < -0.95 && reach.1 > 0.95, "the script's stick y only reached {reach:?}");

    let mut keys = pad::Pads::unplugged(pad::Wiring::default());
    let seen = ride_the_flip(Some(&mut keys)).expect("the disc went away between two rides");
    let on_keys: Vec<&Seat> = seen.iter().collect();
    let by_keys = assert_it_flipped("on the keys", &on_keys);
    assert!(
        by_keys.abs_diff(by_stick) <= 2,
        "the keys fired the backflip at frame {by_keys} and the stick at {by_stick}"
    );
}

// ---------------------------------------------------------------------------------------------------
// the chase camera through the flip (P34)
// ---------------------------------------------------------------------------------------------------

/// The shell's camera numbers (`src/main.rs`'s `NEAR_CLIP`, `REVERSE_SPEED`, `SPEED_REFERENCE` and
/// `START_CAMERA`), repeated because a binary's consts cannot be imported; each says there why it is
/// what it is.
const NEAR_CLIP: f32 = 2.4;
const REVERSE_SPEED: f32 = 5.0;
const SPEED_REFERENCE: f32 = 117.332_8;
const START_CAMERA: &str = "Race Cam 1";

impl game_sim::camera::Surface for Jump {
    /// The ramp, as the camera's ground probe asks it: the point moved onto the ground under it.
    fn on_surface(&mut self, point: Vf) -> Vf {
        let (ground, _) = self.ground(point);
        Vf::new(point.x(), ground.y(), point.z(), point.w())
    }
}

/// free06's follow cameras, read off the disc the way the shell reads them: the track's second
/// archive's `cpresets.ini` — that file exactly, not `biplanecpresets.ini` beside it. `None` without
/// the disc.
fn the_disc_cameras() -> Option<Vec<game_sim::camera::Preset>> {
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
    let presets = game_sim::camera::Preset::read_bytes(&res.read(&bytes, ini).ok()?, "");
    (!presets.is_empty()).then_some(presets)
}

/// One frame of the chase camera as the rider sees it: is the bike in the air, which side of it the
/// eye is on along the way it is travelling, and how far the view has turned from the run-up's
/// heading (+z), in degrees.
struct Shot {
    airborne: bool,
    behind: f32,
    turned: f32,
}

/// Ride the flip with the shell's race camera on it — `Race Cam 1`, snapped on the first frame, at
/// the shell's speed reference — handed its target by `target`. `None` without the disc.
fn film_the_flip(target: fn(&game_sim::ride::Ride) -> game_sim::camera::Ridden) -> Option<Vec<Shot>> {
    use game_sim::camera::{Preset, Race, Switches};
    let presets = the_disc_cameras()?;
    let mut camera = Race::new(presets, F(REVERSE_SPEED));
    let start = Preset::find(&camera.cam.presets, START_CAMERA).expect("free06 has no Race Cam 1");
    camera.cam.set_preset(start);
    camera.cam.snap = true;
    camera.cam.speed_reference = F(SPEED_REFERENCE);
    let mut shots = Vec::new();
    ride_the_flip_watched(None, &mut |sim, world| {
        let ride = &sim.rider.ride;
        let rider = target(ride);
        let switches = Switches { no_look_ahead: false, short_viewport: false };
        let view = camera.update(DT, &rider, F(NEAR_CLIP), &sim.tables.ride.curve, switches, world);
        let (at, vel) = (ride.position(), ride.chassis.vel);
        let along = vel.x().hypot(vel.z()).max(1e-6);
        let (dx, dz) = (view.position.x() - at.x(), view.position.z() - at.z());
        let (lx, lz) = (view.look_at.x() - view.position.x(), view.look_at.z() - view.position.z());
        shots.push(Shot {
            airborne: !ride.air.on_ground,
            behind: -(dx * vel.x() + dz * vel.z()) / along,
            turned: lx.atan2(lz).to_degrees(),
        });
    })?;
    Some(shots)
}

/// P34: the chase camera stays behind the bike through a backflip.
///
/// The original's camera heads along the velocity while the bike is in the air and along the bike's
/// forward row only on the ground. The shell handed it the on-ground flag where it reads an airborne
/// one, so it was the other way round: in the air the heading came off the forward row, which a
/// backflip points straight up and then over backwards, and the view swung round to the front of the
/// bike at the top of the flip. Here the flip is filmed through `src/chase.rs` as the shell films it,
/// and from the lip to the landing the eye has to stay behind the bike and the view within a few
/// degrees of the line the bike is travelling along. Then it is filmed again with the flag the wrong
/// way round, which has to lose the bike, so the ride is known to be one that shows the fault.
#[test]
fn the_camera_stays_behind_the_backflip() {
    let Some(shots) = film_the_flip(chase::bike) else { return };
    // The bike is put down two feet up and drops onto the flat first; the jump is every frame in the
    // air after it has stood on the ground.
    let jump = |shots: &[Shot]| -> Vec<(usize, f32, f32)> {
        let landed = shots.iter().position(|s| !s.airborne).unwrap_or(shots.len());
        (landed..shots.len()).filter(|&i| shots[i].airborne).map(|i| (i, shots[i].behind, shots[i].turned)).collect()
    };
    let air = jump(&shots);
    assert!(air.len() > 30, "the flip was only {} frames in the air", air.len());
    let nearest = air.iter().map(|s| s.1).fold(f32::INFINITY, f32::min);
    let widest = air.iter().map(|s| s.2.abs()).fold(0.0f32, f32::max);
    assert!(nearest > 5.0, "in the air the eye came within {nearest:.1} ft of the bike's side, or ahead of it");
    assert!(widest < 10.0, "in the air the view turned {widest:.1} degrees off the run-up's line");

    let inverted = |ride: &game_sim::ride::Ride| {
        let mut rider = chase::bike(ride);
        rider.airborne = !rider.airborne;
        rider
    };
    let shots = film_the_flip(inverted).expect("the disc went away between two rides");
    let wrong = jump(&shots);
    let lost = wrong.iter().map(|s| s.2.abs()).fold(0.0f32, f32::max);
    assert!(lost > 90.0, "with the flag the wrong way round the view only turned {lost:.1} degrees");
}

/// The brake is pinned for ten seconds and the bike stops, through the shell's own bike and tables.
///
/// It used to hold about 26 ft/s for ever, and until P13 both this file and the sibling blamed a
/// locked clutch idling through first gear. The real cause was that `ride::surface_drag` took the
/// brake off the rear wheel's spin, which the road overwrites every substep, so the pad never
/// reached the chassis at all; the clutch was slipping and merely out-pushing a brake that was not
/// there. The sibling file's version has the long version.
#[test]
fn it_brakes_to_a_stop() {
    let m = ride_the_minute();
    let brake = m.leg("the brake");
    let from = brake[0].speed;
    let after = brake[(2.0 / DT.0) as usize].speed;
    assert!(after < from * 0.2, "two seconds of brake took {from} ft/s to {after}");

    let end = brake[brake.len() - 1].speed;
    assert!(end < 1.0, "ten seconds of brake left the bike doing {end} ft/s");
    // Within three seconds, as in the sibling file (P15).
    let stop = brake.iter().position(|t| t.speed < 1.0).map_or(f32::MAX, |i| i as f32 * DT.0);
    assert!(stop < 3.0, "the brake took {stop} s to stop the bike from {from} ft/s");

    // And it stays stopped with the rider's hands off the pad.
    let stopped = m.leg("stopped");
    assert!(fastest(&stopped) < 2.0, "it rolled away again at {} ft/s", fastest(&stopped));
    assert_eq!(wrecks(&brake) + wrecks(&stopped), 0, "stopping wrecked");
}

/// A bike that has been sitting on the start line pulls away when the player reaches the throttle —
/// through the pad, which is the state the shell actually starts a session in.
///
/// The sibling file's `it_pulls_away_after_a_frame_off_the_throttle` has the long version. The short
/// one: `ride.rs`'s idle rule pulls the clutch in for a rider at rest off the throttle, and until
/// P11 it wrote that to a copy of the engine's clutch word that nothing walked back out, so one
/// frame of a shut throttle — which is every session before the player's thumb arrives — left the
/// clutch fully in and the bike would not move at any throttle. The minute above never caught it
/// because its script has `down cross` on the very first frame.
///
/// This file's version is here rather than only in the sibling because the throttle in question is
/// the one that comes out of a report's pressure byte: the frames this test spends waiting are
/// frames the input layer sees an idle pad, and `gas_rate_limited` being exactly zero on them is
/// what arms the rule.
#[test]
fn it_pulls_away_after_a_frame_off_the_throttle() {
    let gas = Hands { buttons: Control::Cross.mask(), ..Hands::default() };

    for (what, wait) in [("one frame", 1usize), ("a second", 60)] {
        let tables = Arc::new(Tables::build());
        let phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
        let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
        let rider = game_sim::rider::Rider::new(
            &phy,
            stock::mounts(&geom),
            stock::SPAWN,
            stock::FACING,
            Set::default(),
        );
        let mut sim = Sim::with_rider(0, tables, phy, rider);
        let mut world = Jump::nothing();

        let mut ride = |sim: &mut Sim, hands: &Hands| {
            let reports: [Report; PADS] = [hands.report(), Report::missing()];
            let controls = sim.poll(DT, &reports);
            let frame = sim.step(DT, &controls, &mut world);
            assert!(frame.wreck.is_none(), "the bike wrecked on the start line");
        };

        for _ in 0..wait {
            ride(&mut sim, &Hands::default());
        }
        // An idle pad at a standstill is what pulls the clutch in, and it is all the way in.
        assert_eq!(
            sim.rider.ride.mv.clutch_in.0, 1.0,
            "after {what} on an idle pad the clutch is {} and not in",
            sim.rider.ride.mv.clutch_in.0
        );

        let mut top = 0.0f32;
        let mut clutch = Vec::with_capacity(600);
        for _ in 0..600 {
            ride(&mut sim, &gas);
            top = top.max(sim.rider.ride.chassis.speed.0);
            clutch.push(sim.rider.ride.mv.clutch_in.0);
        }

        // The clutch is fed back out over the file's `ClutchOutRateLimit` — a quarter of a second
        // on this bike — rather than dropped in the first frame on the throttle.
        assert!(
            clutch[0] > 0.9 && clutch[0] < 1.0,
            "after {what}, the first frame on the throttle took the clutch from in to {} — it was \
             dropped, not fed",
            clutch[0]
        );
        let out = clutch.iter().position(|&c| c == 0.0).unwrap_or_else(|| {
            panic!("after {what} on an idle pad, the clutch never came out: {:?}", &clutch[..8])
        });
        let quarter = (0.25 / DT.0).round() as usize;
        assert!(
            out >= quarter - 2 && out <= quarter + 2,
            "after {what}, the clutch was out {out} frames in, not the file's quarter second \
             ({quarter} frames)"
        );

        // And the bike goes.
        assert!(
            top > 20.0,
            "after {what} on an idle pad, ten seconds of `down cross` reached {top} ft/s"
        );
    }
}

#[test]
fn none_of_the_minute_wrecks() {
    let m = ride_the_minute();

    // The minute, leg by leg, for a human reading a failure rather than for the assert.
    for (name, seen) in &m.legs {
        let seen: Vec<&Seat> = seen.iter().collect();
        println!(
            "{name:24} {:4} frames | speed {:6.2}..{:6.2} | came round {:6.3} rad | up {:5.3} \
             | peak y {:6.2} | air {:4} | wrecks {}",
            seen.len(),
            slowest(&seen),
            fastest(&seen),
            turned(&seen),
            flattest(&seen),
            highest(&seen),
            airborne(&seen),
            wrecks(&seen),
        );
    }

    let all = m.all();
    assert_eq!(wrecks(&all), 0, "the minute wrecked {} of its {} frames", wrecks(&all), all.len());
    assert!(all[all.len() - 1].on_ground, "the minute ended in the air");
}

// ---------------------------------------------------------------------------------------------------
// the biplane switch (P19)
// ---------------------------------------------------------------------------------------------------

use vehicle::{Garage, Kind, Plane, Spawn};

/// A plane parked on the floor somewhere off the bike's line, nose along −z: numbers of this file's
/// own, because the test has no disc and a track's `machine.txt` is the disc's.
fn parked_plane() -> Plane {
    Plane::parked(Spawn { at: Vf::new(400.0, 0.0, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) })
}

/// The bike [`ride_the_minute`] rides, on its start line.
fn the_minutes_bike() -> Sim {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(ridden_bike().as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let rider = game_sim::rider::Rider::new(
        &phy,
        stock::mounts(&geom),
        stock::SPAWN,
        stock::FACING,
        Set::default(),
    );
    Sim::with_rider(0, tables, phy, rider)
}

/// The minute's first `n` frames, as the reports the shell's pad would have built.
fn first_frames(n: usize) -> Vec<Report> {
    script(MINUTE).into_iter().flat_map(|(_, reports)| reports).take(n).collect()
}

/// One pass of the shell's fixed update through the garage: the poll, then whichever machine is up.
fn pass(sim: &mut Sim, garage: &mut Garage, report: Report, world: &mut Jump) {
    let reports: [Report; PADS] = [report, Report::missing()];
    let controls = sim.poll(DT, &reports);
    garage.step(sim, DT, &controls, world);
}

/// The ride's own hash: the bike and the rider, not the pads, which are polled whichever machine is up.
fn bike_hash(sim: &Sim) -> u64 {
    game_sim::hash::of(&sim.rider.ride)
}

/// `--vehicle bike` is the shell as it was: the whole minute through a garage with the bike up hashes
/// the same, frame for frame, as the minute through `Sim::step` alone — and the garage's plane, which
/// a bike session still carries, never moves.
#[test]
fn a_default_session_rides_the_bike_exactly_as_before() {
    let mut before = the_minutes_bike();
    let mut after = the_minutes_bike();
    let mut garage = Garage::new(Kind::Bike, Some(parked_plane())).unwrap();
    let (mut a, mut b) = (Jump::nothing(), Jump::nothing());
    let mut frames = 0;
    for (name, reports) in script(MINUTE) {
        let ramp = match name.as_str() {
            "the run-up" => Some(RAMP),
            _ => None,
        };
        if let Some((start, slope, length)) = ramp {
            let m = before.rider.ride.transform();
            a = Jump {
                origin: before.rider.ride.position(),
                dir: Vf::new(m.0[2].x(), 0.0, m.0[2].z(), 0.0),
                start,
                slope,
                length,
            };
            b = a.clone();
        }
        for report in reports {
            let reports: [Report; PADS] = [report, Report::missing()];
            let controls = before.poll(DT, &reports);
            before.step(DT, &controls, &mut a);
            pass(&mut after, &mut garage, report, &mut b);
            assert_eq!(after.hash(), before.hash(), "{name}: the garage's bike left the bare one at frame {frames}");
            frames += 1;
        }
    }
    assert_eq!(frames, 3600);
    assert_eq!(garage.up, Kind::Bike);
    assert_eq!(garage.plane, Some(parked_plane()), "a bike session moved the parked plane");
    // And the bike went somewhere, or the comparison above was of two bikes standing still.
    assert!(before.rider.ride.position().z().abs() + before.rider.ride.position().x().abs() > 100.0);
}

/// `--vehicle biplane` starts with the plane up, where its `machine.txt` parked it, and the bike is
/// left alone: the minute's launch and first seconds flat out on the throttle reach the plane's
/// controller and not the bike, which does not move a bit.
#[test]
fn a_biplane_session_starts_in_the_plane_and_leaves_the_bike_alone() {
    let mut sim = the_minutes_bike();
    let mut garage = Garage::new(Kind::Biplane, Some(parked_plane())).unwrap();
    assert_eq!(garage.up, Kind::Biplane);
    assert_eq!(garage.plane.as_ref().unwrap().at, parked_plane().spawn.at);

    let parked = bike_hash(&sim);
    let mut world = Jump::nothing();
    for report in first_frames(5 * 60) {
        pass(&mut sim, &mut garage, report, &mut world);
        assert_eq!(bike_hash(&sim), parked, "the bike moved with the plane up");
    }
    assert_eq!(sim.ticks, 0, "the bike was stepped with the plane up");
    // The pad still reached the port's input manager: the throttle is down on it.
    assert!(sim.input.pad(0).held(Control::Cross), "the plane's pad was never polled");
}

/// The disc's `biplane.phy`, out of `machine.res`, or `None` (and a note) with no disc configured.
fn the_discs_plane() -> Option<game_sim::phy::Plane> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res"))?.name.clone();
    let res = disc.res(&name).expect("machine.res");
    let bytes = res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("machine.res carries biplane.phy");
    Some(game_sim::phy::Plane::read_bytes(&bytes))
}

/// A session that starts in the plane flies it (P31), the way the shell flies it: the garage with the
/// plane up, its flight built off the disc's `biplane.phy`, stepped at the shell's 60 Hz on the seat's
/// pad. Throttle held (Cross), it gains airspeed and leaves the ground; throttle closed (Square), it
/// loses height.
///
/// Over this file's flat floor, not a track: the original's contact search (S45/S46/S48) is not
/// ported, so the plane's world is the ground alone anywhere. The plane is parked with its lowest
/// wheel on the floor. The thresholds are what any plane that flies does; nothing of the original's
/// was flown here to compare with.
#[test]
fn a_biplane_session_flies() {
    let Some(file) = the_discs_plane() else { return };
    let flight = game_sim::flight::Flight::new(&file);
    let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
    let spawn = Spawn { at: Vf::new(400.0, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) };
    let mut sim = the_minutes_bike();
    let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(spawn, flight))).unwrap();
    let mut world = Jump::nothing();
    let mut fly = |sim: &mut Sim, garage: &mut Garage, report: Report, seconds: usize| {
        for _ in 0..seconds * 60 {
            pass(sim, garage, report, &mut world);
        }
        garage.plane.clone().unwrap()
    };

    let rest = fly(&mut sim, &mut garage, Report::from_buttons(0), 2);
    assert!(rest.on_ground(), "the parked plane is not on its wheels");
    let (throttle, still) = rest.gauges().unwrap();
    assert_eq!(throttle, F::ZERO);
    assert!(still.0 < 1.0, "the parked plane is rolling at {} ft/s", still.0);

    let gas = Report::from_buttons(Control::Cross.mask());
    let mut fast = still;
    let mut left = None;
    let mut top = rest.at.y();
    for s in 1..=30 {
        let plane = fly(&mut sim, &mut garage, gas, 1);
        fast = fast.max(plane.gauges().unwrap().1);
        if left.is_none() && !plane.on_ground() && plane.at.y() > rest.at.y() + 10.0 {
            left = Some(s);
        }
        top = top.max(plane.at.y());
    }
    let plane = garage.plane.as_ref().unwrap();
    assert!(plane.gauges().unwrap().0 > F(0.9), "thirty seconds of Cross did not open the throttle");
    assert!(fast.0 > 50.0, "thirty seconds of throttle and the airspeed is {} ft/s", fast.0);
    let left = left.expect("thirty seconds of throttle and the plane never left the ground");
    assert!(top > rest.at.y() + 100.0, "off the ground at {left} s, but only {top:.1} ft up");
    assert_eq!(sim.ticks, 0, "the bike was stepped with the plane up");

    let high = fly(&mut sim, &mut garage, Report::from_buttons(Control::Square.mask()), 5);
    // Closed, not zero: held back, the control law puts a negative throttle back to zero and closes
    // it one step again, so it alternates between zero and one step under.
    assert!(high.gauges().unwrap().0 <= F::ZERO, "five seconds of Square did not close the throttle");
    let low = fly(&mut sim, &mut garage, Report::from_buttons(0), 10);
    assert!(
        low.at.y() < high.at.y() - 50.0,
        "the throttle is shut and the plane went from {:.1} to {:.1} ft",
        high.at.y(),
        low.at.y()
    );
}

/// P50: upside down, the new biplane controls move the plane on the screen the way they move it the
/// right way up, and the original's reach the plane's simulation unchanged.
///
/// The shell's pass, as `main.rs` makes it: the plane's attitude after the last step says whether
/// it is upside down (`Plane::inverted`), and the seat's `PlaneScheme` turns the report round before
/// the poll. The plane takes off on the disc's `biplane.phy`, then rolls on full right stick until its
/// up points well down. From there and from the right way up, a third of a second of each control is
/// flown against a third of a second of nothing, and what the control added is read the way the
/// follow camera shows it, whose up is the world's: the nose's height for a pulled stick, the turn of
/// the heading for the left rudder. Under the new scheme each goes the same way upside down as the
/// right way up; under the original's both go the other way, which is the original's flip.
#[test]
fn upside_down_the_new_biplane_controls_fly_the_way_they_do_upright() {
    use pad::PlaneScheme;
    let Some(file) = the_discs_plane() else { return };
    let flight = game_sim::flight::Flight::new(&file);
    let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
    let spawn = Spawn { at: Vf::new(400.0, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) };
    let mut sim = the_minutes_bike();
    let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(spawn, flight))).unwrap();
    let mut world = Jump::nothing();

    // One frame of the shell: the remap off the attitude, then the pass.
    let shell = |sim: &mut Sim, garage: &mut Garage, inverted: &mut bool, scheme: PlaneScheme, report: Report, world: &mut Jump| {
        *inverted = garage.inverted(*inverted);
        pass(sim, garage, scheme.fly(report, *inverted), world);
    };
    let gas = Report::from_buttons(Control::Cross.mask());
    let with_stick = |mut r: Report, x: u8, y: u8| {
        r.set_axis(2, x);
        r.set_axis(3, y);
        r
    };
    let mut inverted = false;
    for _ in 0..30 * 60 {
        shell(&mut sim, &mut garage, &mut inverted, PlaneScheme::New, gas, &mut world);
    }
    let plane = garage.plane.clone().unwrap();
    assert!(!plane.on_ground() && plane.up.y() > 0.9, "thirty seconds of gas and the plane is not flying level");
    assert!(!inverted);
    let upright = (sim.clone(), garage.clone());

    let mut frames = 0;
    // Full stick rolls it with the nose falling away, so its up gets to about -0.8 at the most:
    // -0.75 is well past where the shell calls it upside down.
    while garage.plane.as_ref().unwrap().up.y() > -0.75 {
        shell(&mut sim, &mut garage, &mut inverted, PlaneScheme::New, with_stick(gas, 0xff, AXIS_CENTRE), &mut world);
        frames += 1;
        assert!(frames < 5 * 60, "five seconds of full right stick and the plane did not roll over");
    }
    let plane = garage.plane.as_ref().unwrap();
    assert!(inverted, "the plane is upside down (up {:.2}) and the shell does not know it", plane.up.y());
    assert!(!plane.on_ground());
    let upside_down = (sim.clone(), garage.clone());

    // What a third of a second of `report` does to the nose's height and the heading, from `from`,
    // under `scheme`; and the plane it leaves.
    let try_it = |from: &(Sim, Garage), was: bool, scheme: PlaneScheme, report: Report| {
        let (mut sim, mut garage, mut inverted) = (from.0.clone(), from.1.clone(), was);
        let before = garage.plane.as_ref().unwrap().facing;
        let mut world = Jump::nothing();
        for _ in 0..20 {
            shell(&mut sim, &mut garage, &mut inverted, scheme, report, &mut world);
        }
        let plane = garage.plane.unwrap();
        let after = plane.facing;
        // The heading's turn about the world's up, which is the camera's: only its sign against
        // the right way up's is asserted, so which way round the world's axes are does not matter.
        let turn = before.z() * after.x() - before.x() * after.z();
        (after.y() - before.y(), turn, plane)
    };
    let pulled = with_stick(gas, AXIS_CENTRE, 0xff);
    let left = Report::from_buttons(Control::Cross.mask() | Control::L1.mask());
    let effect = |from: &(Sim, Garage), was: bool, scheme: PlaneScheme| {
        let (rise, turn, _) = try_it(from, was, scheme, gas);
        let (pull_rise, _, _) = try_it(from, was, scheme, pulled);
        let (_, left_turn, _) = try_it(from, was, scheme, left);
        (pull_rise - rise, left_turn - turn)
    };

    let (climb, yaw) = effect(&upright, false, PlaneScheme::New);
    assert!(climb > 0.01, "the right way up a pulled stick does not lift the nose ({climb})");
    assert!(yaw.abs() > 1e-3, "the right way up the left rudder does not turn the plane ({yaw})");
    assert_eq!(effect(&upright, false, PlaneScheme::Original), (climb, yaw), "the right way up the schemes differ");

    let (new_climb, new_yaw) = effect(&upside_down, true, PlaneScheme::New);
    assert!(new_climb > 0.01, "upside down a pulled stick does not lift the nose under the new controls ({new_climb})");
    assert!(new_yaw.signum() == yaw.signum() && new_yaw.abs() > 1e-3, "upside down the left rudder turns {new_yaw}, upright {yaw}");
    let (old_climb, old_yaw) = effect(&upside_down, true, PlaneScheme::Original);
    assert!(old_climb < -0.01, "upside down the original's pulled stick does not drop the nose ({old_climb})");
    assert!(old_yaw.signum() != yaw.signum() && old_yaw.abs() > 1e-3, "upside down the original's left rudder turns {old_yaw}, upright {yaw}");

    // And the original's is the pad as it is: the same plane, field for field, as the raw report.
    for report in [pulled, left, with_stick(Report::from_buttons(Control::Up.mask() | Control::R1.mask()), 0x30, 0xc0)] {
        let (_, _, through) = try_it(&upside_down, true, PlaneScheme::Original, report);
        let (mut sim, mut garage) = upside_down.clone();
        let mut world = Jump::nothing();
        for _ in 0..20 {
            pass(&mut sim, &mut garage, report, &mut world);
        }
        assert_eq!(through, garage.plane.unwrap(), "the original's biplane controls changed what the plane read");
    }
    println!(
        "biplane upside down: pulled stick lifts the nose {climb:.4} upright, {new_climb:.4} new, {old_climb:.4} original; \
         left rudder turns {yaw:.4} upright, {new_yaw:.4} new, {old_yaw:.4} original ({frames} frames to roll over)"
    );
}

/// A swap mid-session swaps, the way the original's swap does (R8): ride the bike off its start
/// line, swap, and the plane has the controller from where it was parked while the bike stands
/// exactly where it was left; swap back and the bike carries on from there rather than from its
/// start line, and a second swap into the plane puts the plane back where it was parked.
#[test]
fn a_swap_mid_session_swaps() {
    let mut sim = the_minutes_bike();
    let mut garage = Garage::new(Kind::Bike, Some(parked_plane())).unwrap();
    let mut world = Jump::nothing();
    let frames = first_frames(10 * 60);
    let (bike, plane, back) = (&frames[..4 * 60], &frames[4 * 60..7 * 60], &frames[7 * 60..]);

    for &report in bike {
        pass(&mut sim, &mut garage, report, &mut world);
    }
    let start = stock::SPAWN;
    let left_at = sim.rider.ride.position();
    let left = bike_hash(&sim);
    let ridden = (left_at.x() - start.x()).hypot(left_at.z() - start.z());
    assert!(ridden > 20.0, "four seconds of the minute rode the bike only {ridden:.1} ft");
    let speed = sim.rider.ride.chassis.speed.0;
    assert!(speed > 10.0, "the bike is doing {speed:.1} ft/s when it is left");

    // Into the plane, at speed: the pause menu's swap has no 40 mph gate (only the prompt does).
    assert_eq!(garage.swap(), Ok(Kind::Biplane));
    assert_eq!(garage.plane.as_ref().unwrap().at, parked_plane().spawn.at, "the plane was not reset to its spawn");
    let ticks = sim.ticks;
    for &report in plane {
        pass(&mut sim, &mut garage, report, &mut world);
        assert_eq!(bike_hash(&sim), left, "the bike moved after it was left");
    }
    assert_eq!(sim.ticks, ticks);

    // And back: the bike picks up where it was left, still rolling, and the throttle reaches it.
    assert_eq!(garage.swap(), Ok(Kind::Bike));
    pass(&mut sim, &mut garage, back[0], &mut world);
    let first = sim.rider.ride.position();
    let step = (first.x() - left_at.x()).hypot(first.z() - left_at.z());
    assert!(step < 5.0, "the bike's first frame back is {step:.2} ft from where it was left");
    assert_ne!(bike_hash(&sim), left, "the bike did not move once it was back");
    for &report in &back[1..] {
        pass(&mut sim, &mut garage, report, &mut world);
    }
    let end = sim.rider.ride.position();
    let further = (end.x() - left_at.x()).hypot(end.z() - left_at.z());
    assert!(further > 20.0, "three seconds back on the bike rode it only {further:.1} ft");

    // A plane taken again is put back where it was parked, whatever had moved it.
    garage.plane.as_mut().unwrap().at = Vf::new(0.0, 500.0, 0.0, 1.0);
    assert_eq!(garage.swap(), Ok(Kind::Biplane));
    assert_eq!(garage.plane.as_ref().unwrap().at, parked_plane().spawn.at);
}

/// A four-player session steps and draws four views (V22): the shell itself, run with no window for
/// sixty frames, says on its last lines that every player's simulation ran every frame, that each of
/// the four quarters drew triangles of its own, and that each view's camera sits where it follows its
/// own rider from — the same place behind each of four riders who started six feet apart — rather
/// than all four following the first. It writes its shot under the test's own directory, which is
/// also how the frame was looked at by eye.
///
/// Off the disc, like every session of the shell; skipped without one.
#[test]
fn a_four_player_session_steps_and_draws_four_views() {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let shot = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("v22-four-views.png");
    let _ = std::fs::remove_file(&shot);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--players", "4", "--size", "320x224", "--frames", "60", "--shot-frame", "59"])
        .arg("--shot")
        .arg(&shot)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    assert!(said.contains("views: 4 players, quarters"), "{said}");
    assert!(shot.exists(), "no shot written:\n{said}");

    // "player N: T ticks, at x, y, z; view WxH at X,Y, camera at x, y, z, K triangles a frame"
    let numbers = |s: &str| -> Vec<f32> {
        s.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|w| w.parse().ok()).collect()
    };
    let players: Vec<Vec<f32>> = (1..=4)
        .map(|n| {
            let line = said
                .lines()
                .find(|l| l.starts_with(&format!("player {n}: ")) && l.contains(" ticks, at "))
                .unwrap_or_else(|| panic!("no last line for player {n}:\n{said}"));
            numbers(line)
        })
        .collect();
    let mut views = Vec::new();
    let mut behind = Vec::new();
    for p in &players {
        // [n, ticks, x, y, z, w, h, vx, vy, cx, cy, cz, triangles]
        assert_eq!(p.len(), 13, "{p:?}");
        assert!(p[1] >= 60.0, "player {} stepped {} ticks of 60 frames", p[0], p[1]);
        assert_eq!((p[5], p[6]), (160.0, 112.0), "player {}'s view is not a quarter", p[0]);
        assert!(p[12] > 0.0, "player {}'s view drew nothing", p[0]);
        views.push((p[7] as usize, p[8] as usize));
        behind.push([p[9] - p[2], p[10] - p[3], p[11] - p[4]]);
    }
    views.sort();
    assert_eq!(views, vec![(0, 0), (0, 112), (160, 0), (160, 112)], "four views, one a quarter");
    // Every camera is within a camera's distance of its own rider, and at the same place behind them.
    for (i, d) in behind.iter().enumerate() {
        let far = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!(far > 1.0 && far < 40.0, "player {}'s camera is {far} ft from them", i + 1);
        for k in 0..3 {
            assert!((d[k] - behind[0][k]).abs() < 3.0, "player {}'s camera does not follow them: {d:?} against {:?}", i + 1, behind[0]);
        }
    }
    // And four riders, not one drawn four times.
    for i in 0..4 {
        for j in i + 1..4 {
            let (a, b) = (&players[i], &players[j]);
            assert!((a[2] - b[2]).abs() + (a[4] - b[4]).abs() > 1.0, "players {} and {} are in one place", i + 1, j + 1);
        }
    }
}

/// S57: L1+R1 puts the plane back where it was parked, on the press's own frame, when it is on its
/// wheels or wrecked — the original's reset chord — and nowhere else. The original has no fade and no
/// smoke for it (S57's capture: a hard cut, the plane drawn whole on the next frame), so the frame
/// count asked is zero: the frame R1 goes down with L1 held is the frame the plane is back. L1 or R1
/// alone is the rudder, and in the air the chord does nothing. Under both of P40's layouts: the new
/// one moves the gas and the brake, not the shoulder buttons.
///
/// Over the flat floor `a_biplane_session_flies` uses; a crash needs the world's contact (S48).
#[test]
fn l1_and_r1_put_the_biplane_back_on_the_press_frame() {
    use pad::Scheme;
    let Some(file) = the_discs_plane() else { return };
    for scheme in Scheme::ALL {
        let flight = game_sim::flight::Flight::new(&file);
        let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
        let spawn = Spawn { at: Vf::new(400.0, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) };
        let mut sim = the_minutes_bike();
        let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(spawn, flight))).unwrap();
        let mut world = Jump::nothing();
        // One pass as the shell makes it: the seat's layout over the pad, the poll, the garage.
        let mut pass = |sim: &mut Sim, garage: &mut Garage, buttons: &[Control]| {
            let report = scheme.ride(Report::from_buttons(game_sim::input::mask_of(buttons)));
            let controls = sim.poll(DT, &[report, Report::missing()]);
            let said = garage.step(sim, DT, &controls, &mut world);
            (said.respawned, garage.plane.clone().unwrap())
        };
        let away = |p: &Plane| (p.at.x() - spawn.at.x()).abs() + (p.at.z() - spawn.at.z()).abs();
        // The original's gas under either layout: what the ride reads after the remap.
        let gas = match scheme {
            Scheme::Original => Control::Cross,
            Scheme::New => Control::R2,
        };

        for _ in 0..60 {
            pass(&mut sim, &mut garage, &[]);
        }
        // Rolling on the wheels: two seconds of gas, then the throttle shut and the plane rolling on.
        for _ in 0..120 {
            pass(&mut sim, &mut garage, &[gas]);
        }
        let (_, rolling) = pass(&mut sim, &mut garage, &[]);
        assert!(rolling.on_ground(), "{scheme:?}: two seconds of gas and the plane is off the ground");
        assert!(away(&rolling) > 5.0, "{scheme:?}: the plane has not rolled away from its spawn");

        // L1 alone, then R1 alone: the rudder, never the reset.
        for buttons in [[Control::L1], [Control::R1]] {
            for _ in 0..10 {
                let (back, plane) = pass(&mut sim, &mut garage, &buttons);
                assert!(!back && away(&plane) > 5.0, "{scheme:?}: {buttons:?} alone put the plane back");
            }
            pass(&mut sim, &mut garage, &[]);
        }

        // L1 down, then R1 with it: back on that frame, at rest, the throttle shut.
        let (back, plane) = pass(&mut sim, &mut garage, &[Control::L1]);
        assert!(!back && away(&plane) > 5.0, "{scheme:?}: L1 alone put the plane back");
        let (back, plane) = pass(&mut sim, &mut garage, &[Control::L1, Control::R1]);
        assert!(back, "{scheme:?}: R1 pressed with L1 held did not put the plane back");
        assert!(away(&plane) < 0.5, "{scheme:?}: put back {:.2} ft from the spawn", away(&plane));
        let (throttle, speed) = plane.gauges().unwrap();
        assert_eq!(throttle, F::ZERO, "{scheme:?}");
        assert!(speed.0 < 1.0, "{scheme:?}: put back moving at {} ft/s", speed.0);
        // Held on, it is not taken again: the chord is a press.
        let (back, _) = pass(&mut sim, &mut garage, &[Control::L1, Control::R1]);
        assert!(!back, "{scheme:?}: the chord held put the plane back a second time");

        // In the air: nothing.
        let mut high = None;
        for _ in 0..40 * 60 {
            let (_, plane) = pass(&mut sim, &mut garage, &[gas]);
            if !plane.on_ground() && plane.at.y() > spawn.at.y() + 50.0 {
                high = Some(plane);
                break;
            }
        }
        let high = high.unwrap_or_else(|| panic!("{scheme:?}: forty seconds of gas and the plane never climbed"));
        pass(&mut sim, &mut garage, &[gas, Control::L1]);
        let (back, plane) = pass(&mut sim, &mut garage, &[gas, Control::L1, Control::R1]);
        assert!(!back, "{scheme:?}: the chord put the plane back in the air");
        assert!(plane.at.y() > high.at.y() - 20.0, "{scheme:?}: the chord moved the plane in the air");
    }
}

// ---------------------------------------------------------------------------------------------------
// the put-back's ghost (P57)
// ---------------------------------------------------------------------------------------------------

/// P57: the bike's ghost after a put-back runs as the original's does, bit for bit. Captured live on
/// copy 3 (the journal's `capture/ghost_watch.py`, freeride state 8): the put-back's own frame draws
/// the bike at half, the clock at its 3 s; each tick after it takes the tick's 0.03333 off the clock
/// and the alpha climbs from 0.1 towards 1 with the time gone; on the 91st the clock is past zero and
/// the bike is solid again. The bit patterns below are the game's own, read off the model and the
/// rider object.
#[test]
fn the_put_back_ghost_fades_in_as_the_originals() {
    use vehicle::Ghost;
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let mut ghost = Ghost::default();
    assert_eq!(ghost.shown(), 1.0, "a bike never put back is solid");
    ghost.step(dt, || false);
    assert_eq!(ghost, Ghost::default(), "the tick does nothing with no ghost running");

    ghost.start();
    assert_eq!((ghost.clock.0, ghost.alpha.0, ghost.on), (3.0, 0.5, true));
    assert_eq!(ghost.shown(), 0.5);
    // (tick after the put-back, clock bits, alpha bits)
    let seen: [(usize, u32, u32); 7] = [
        (1, 0x403dddec, 0x3de14726),
        (2, 0x403bbbd8, 0x3df5c180),
        (3, 0x403999c4, 0x3e051dec),
        (4, 0x403777b0, 0x3e0f5b19),
        (5, 0x4035559c, 0x3e199846),
        (89, 0x3d09c19e, 0x3f7d6ac5),
        (90, 0x399e4a00, 0x3f7ffa10),
    ];
    let mut tick = 0;
    for (at, clock, alpha) in seen {
        while tick < at {
            ghost.step(dt, || false);
            tick += 1;
        }
        assert_eq!((ghost.clock.0.to_bits(), ghost.alpha.0.to_bits()), (clock, alpha), "tick {at}");
        assert!(ghost.on, "tick {at}: solid too soon");
    }
    ghost.step(dt, || false);
    assert_eq!(ghost.clock.0.to_bits(), 0xbd074876, "tick 91");
    assert!(!ghost.on);
    assert_eq!(ghost.shown(), 1.0, "solid on the 91st tick");
    ghost.step(dt, || false);
    assert_eq!(ghost.clock.0.to_bits(), 0xbd074876, "a spent clock is not counted further");

    // A put-back while one runs starts it over (the capture's L1+R1 25 ticks into the wreck's).
    ghost.start();
    for _ in 0..25 {
        ghost.step(dt, || false);
    }
    ghost.start();
    assert_eq!((ghost.clock.0, ghost.alpha.0, ghost.on), (3.0, 0.5, true));
}

/// P57 through the garage: L1+R1 on the bike puts it back and starts its ghost on that frame, at
/// half; three seconds of the shell's frames later it is solid. The plane's put-back (S57) has no
/// ghost — the original's plane is drawn whole straight away — and a plane session never ticks the
/// bike's.
#[test]
fn l1_and_r1_put_the_bike_back_as_a_ghost() {
    let mut sim = the_minutes_bike();
    let mut garage = Garage::new(Kind::Bike, Some(parked_plane())).unwrap();
    let mut world = Jump::nothing();
    for &report in &first_frames(2 * 60) {
        pass(&mut sim, &mut garage, report, &mut world);
        assert_eq!(sim.rider.ghost.shown(), 1.0, "the bike is a ghost before any put-back");
    }
    let chord = Report::from_buttons(game_sim::input::mask_of(&[Control::L1, Control::R1]));
    pass(&mut sim, &mut garage, chord, &mut world);
    assert!(sim.rider.ghost.on, "L1+R1 did not start the ghost");
    assert_eq!(sim.rider.ghost.shown(), 0.5);
    let idle = Report::from_buttons(game_sim::input::mask_of(&[]));
    let mut frames = 0;
    while sim.rider.ghost.on {
        pass(&mut sim, &mut garage, idle, &mut world);
        frames += 1;
        assert!(frames < 400, "the ghost never ended");
    }
    // 3 s at the shell's 60 Hz, give or take the frame the clock crosses zero on.
    assert!((179..=181).contains(&frames), "the ghost lasted {frames} frames");
    assert_eq!(sim.rider.ghost.shown(), 1.0);

    // The plane: its own put-back starts no ghost, and the bike's is not ticked while it flies.
    sim.rider.ghost.start();
    let held = sim.rider.ghost;
    assert_eq!(garage.swap(), Ok(Kind::Biplane));
    for _ in 0..30 {
        pass(&mut sim, &mut garage, chord, &mut world);
    }
    assert_eq!(sim.rider.ghost, held, "the bike's ghost moved while the plane had the controller");
}
