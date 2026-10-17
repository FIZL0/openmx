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
//! It is headless: no window, no device, no disc. The bike is [`game_sim::stock::BIKE`], the stand-in
//! the simulation crate ships for exactly this (no number in it is the game's), with the two keys
//! [`ridden_bike`] patches; the world is a floor with one take-off ramp on it.
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

use game_sim::collide::{Bundle, Record};
use game_sim::input::{AXIS_CENTRE, CONTROL_MAP, Control, PADS, Report};
use game_sim::phy::Phy;
use game_sim::rider::Tables;
use game_sim::trick::Set;
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::sync::Arc;

/// The minute, in the kit's pad notation. A file rather than a string const so that a human can send
/// the same ride to a real machine with `./kit pad script`.
const MINUTE: &str = include_str!("ride-minute.pad");

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

/// The top speed the human's recording of the original reaches, in feet a second — measured in
/// `crates/game-sim/tests/ride.rs`, which is where the capture is read and this number is checked back
/// against it. Here it is the scale the legs' own tolerances are written in.
const CAPTURED_TOP_SPEED: f32 = 75.78;

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
/// like the 75.78 the human's recording reaches, so the minute's speeds would mean nothing. Neither
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
        ("Drag = 0.5", "Drag = 0.06"),
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
        if name == "the run-up" {
            let m = sim.rider.ride.transform();
            world = Jump {
                origin: sim.rider.ride.position(),
                dir: Vf::new(m.0[2].x(), 0.0, m.0[2].z(), 0.0),
                start: RAMP.0,
                slope: RAMP.1,
                length: RAMP.2,
            };
        }
        let mut seen = Vec::with_capacity(reports.len());
        for report in reports {
            let reports: [Report; PADS] = [report, Report::missing()];
            let controls = sim.poll(DT, &reports);
            let frame = sim.step(DT, &controls, &mut world);
            let m = sim.rider.ride.transform();
            seen.push(Seat {
                speed: sim.rider.ride.chassis.speed.0,
                heading: f32::atan2(m.0[2].x(), m.0[2].z()),
                up_y: m.0[1].y(),
                y: sim.rider.ride.position().y(),
                on_ground: sim.rider.ride.air.on_ground,
                wreck: frame.wreck.is_some(),
                gas: controls.gas.0,
                brake: controls.brake.0,
                stick_x: controls.stick.0.0,
            });
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
        "the script is not a minute at 60 frames a second"
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
    assert!(half > 3.0, "half a second of full throttle moved the bike at {half} ft/s");

    // Two seconds: a useful fraction of the top speed, and still on its wheels.
    let two = launch[launch.len() - 1].speed;
    assert!(two > 20.0, "two seconds of full throttle only reached {two} ft/s");
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
        assert!(rate > 0.1, "{leg}: the bike turned at {rate} rad/s the way the stick was pushed");
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
    assert!(air > 30, "the ramp only put the bike in the air for {air} frames");
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
        slowest(landed) > CAPTURED_TOP_SPEED * 0.4,
        "the landing left the bike at {} ft/s",
        slowest(landed)
    );
    assert_eq!(wrecks(&jump), 0, "the jump wrecked");
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
    assert!(after < from * 0.5, "two seconds of brake took {from} ft/s to {after}");

    let end = brake[brake.len() - 1].speed;
    assert!(end < 1.0, "ten seconds of brake left the bike doing {end} ft/s");

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
