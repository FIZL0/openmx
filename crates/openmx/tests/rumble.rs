//! P183 — a wreck rumbles the rider's pad, as the original's does.
//!
//! The player's wreck entry asks a human's pad device for its big motor at 200 of 255 for a second
//! (`(1.0, pad, 0, 200)`), unless the options' vibration is off; the biplane's crash asks the same.
//! The shell's half is [`pad::WreckRumble`], which says which frames ask, and [`pad::Pads::rumble`],
//! which asks the device (its own unit tests: the seat's pad and no other, and a uinput motor turned
//! by hand). This is the ride between them: a real `game_sim` wreck, and the asks it makes.

use game_sim::phy::{self, Phy};
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::stock;
use game_sim::trick::Set;
use ps2_float::{F, Vf};

// The shell's pad, compiled in as `tests/ride.rs` takes it: the crate is a binary.
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;

use pad::{Rumble, WreckRumble};

/// `tests/audio.rs`' wreck fixture (P180): `game-sim`'s ridden test bike, a vertical limit low enough
/// that a drop onto flat ground wrecks, a wreck body and the rider's half. Plausible numbers, not the
/// disc's.
const WRECK_BIKE: &str = include_str!("../../game-sim/tests/freeride-bike.phy.txt");
const DROP_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 400
BackWreckSpeedLimit = 400
SideWreckSpeedLimit = 400
RacingDownWreckSpeedLimit = 400
FreestyleDownWreckSpeedLimit = 5
GroundPitchDownWreckAngle = 85
GroundPitchUpWreckAngle = 85
AirbornePitchDownWreckAngle = 175
AirbornePitchUpWreckAngle = 175
AirborneRollWreckAngle = 175
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";
const DROP_PART: &str = "
[PhysicsPart1]
Type = RigidBody
Name = wreck_physics
Weight = 250.0
CollisionWeight = 0.0001
GravityScale = 1.0
InertiaScale = 1.0, 1.0, 1.0
SpringStiffness = 5.0
SpringDamping = 0.4
MaxPenetration = 0.5
CofG = 0.0, 1.5, 0.0
SpringMomentScale = 1.0, 1.0, 1.0
FrictionMomentScale = 1.0, 1.0, 1.0
FrictionZone1 = 0.5
Radius = 1.0
AccelerationThreshold = 0.01
AngularAccelerationThreshold = 0.01
";
const DROP_RIDER: &str = "
[Wrecks]
HitBottomForceTime = 0.1
HitBottomHandHoldTime = 0.3
HitBottomForce = 0.0, 800.0, 0.0
GroundAngularRateDamping = 1.0
RiderFlailHeight = 3.0
RiderRollTime = 0.75
RiderRollSpeed = 0.0
";

/// **A wreck ride's asks**: the bike dropped 30 ft onto flat ground with nobody on the pad, wrecked
/// by the landing, down and put back by the clock — 400 ticks at 30 Hz, each one's wreck entry handed
/// to the seat's [`WreckRumble`] as the shell's ride loop hands it, the asks going to a pad. Returns
/// the pad's asks and the ticks the wreck entry ran on.
fn ride_a_wreck(vibration: bool) -> (Vec<Rumble>, Vec<usize>) {
    let tables = Tables::build();
    let p = Phy::read_bytes(format!("{WRECK_BIKE}{DROP_LIMITS}").as_bytes(), &tables.ride.curve);
    let mut r = Rider::new(&p, stock::MOUNTS, Vf::new(0.0, 30.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    let part = phy::Part::read_bytes(DROP_PART.as_bytes(), "PhysicsPart1");
    r.carries_wreck(part, phy::Wrecks::read_bytes(DROP_RIDER.as_bytes()), part.inertia(Vf::new(2.5, 4.0, 7.0, 1.0)));
    let mut world = game_sim::world::Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    let mut pads = pad::Pads::unplugged(pad::Wiring::default());
    let mut asks = WreckRumble::default();
    let mut entries = Vec::new();
    for tick in 0..400 {
        let f = r.frame(&p, &tables, &Controls::default(), F(0.03333), &mut world);
        if f.wreck.is_some() {
            entries.push(tick);
        }
        if let Some(ask) = asks.frame(f.wreck.is_some(), false, vibration) {
            pads.rumble(ask);
        }
    }
    (pads.rumbles().to_vec(), entries)
}

/// **A wreck asks the seat's pad for one rumble, the original's** (P183): one wreck entry in the
/// ride, and one ask — the big motor at 200 of 255 for a second, 1000 ms at 51400 of the kernel's
/// 65535 — and none with Vibration off.
#[test]
fn a_wreck_rumbles_the_pad_once_and_not_with_vibration_off() {
    let (asked, entries) = ride_a_wreck(true);
    assert_eq!(entries.len(), 1, "the drop wrecks once: {entries:?}");
    assert_eq!(asked, [Rumble::WRECK]);
    assert_eq!((asked[0].millis(), asked[0].magnitude()), (1000, 51400));
    let (asked, entries) = ride_a_wreck(false);
    assert_eq!(entries.len(), 1);
    assert_eq!(asked, []);
}

/// **The plane's crash rumbles once, as it goes up** (P183): the biplane's wreck wrapper asks on the
/// crash, not on every tick the crash word stands; a second crash after a put-back asks again; and
/// Vibration off holds it as it holds the bike's.
#[test]
fn a_plane_crash_rumbles_on_its_first_tick_only() {
    let mut asks = WreckRumble::default();
    let crashed = [false, true, true, true, false, false, true, true];
    let got: Vec<bool> = crashed.iter().map(|&c| asks.frame(false, c, true).is_some()).collect();
    assert_eq!(got, [false, true, false, false, false, false, true, false]);
    let mut asks = WreckRumble::default();
    assert!(crashed.iter().all(|&c| asks.frame(false, c, false).is_none()));
    // and the bike's entry on a frame the plane is down asks once, not twice
    let mut asks = WreckRumble::default();
    assert_eq!(asks.frame(true, true, true), Some(Rumble::WRECK));
}
