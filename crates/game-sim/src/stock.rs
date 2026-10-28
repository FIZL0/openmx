//! The bike and the floor a simulation rides when nobody has handed it a disc.
//!
//! [`crate::Sim::new`] takes a seed and nothing else — it is what a replay fixture carries when it
//! does not carry a whole state — and a seed does not describe a ride. A ride needs a vehicle's
//! `.phy`, the two nodes its wheels hang off, a spawn and a world, all four of which come off the
//! user's own disc at runtime and none of which can live in this repository (`AGENT.md`). So the
//! state a bare seed builds rides *this*: a bike written out below in the game's own parameter
//! format, on a level floor with nothing standing on it.
//!
//! **No number in this module is the game's.** It is a plausible 250 cc four-stroke, read by the
//! ported `.phy` loader like any other, and it exists so that the harness has something that moves:
//! a `Phy` with the file's defaults everywhere has no weight, and a bike with no weight has an
//! infinite inverse mass clamped to zero, which is a bike pinned to the spot. A fixture that means
//! to prove the port against a *recording of the original* carries the disc's bike in
//! [`crate::replay::Fixture::start`] instead; see that field's own documentation.
//!
//! The text is the same bike as `crates/game-sim/tests/freeride-bike.phy.txt`, which the crate's
//! tests have ridden since P2 — two copies because `src` cannot `include_str!` a `tests` fixture and
//! neither half is the disc's, so a drift between them costs nothing but a different stand-in.
//! One line has drifted on purpose: `ReverseSpeed` here is the −10 every disc bike carries, because
//! the fixture's +4 pushes a reversing bike forwards (S122). The `[Collisions]` pair is the disc
//! bike's too (S125): with the file's defaults the chassis' sweep back starts at the bike's origin,
//! under a deck it rides, and pushes it up out of anything not moving within a degree of its own
//! way — a bike parked on the train was pushed along the wagon's deck.

use ps2_float::Vf;

use crate::camera::CurveTable;
use crate::phy::Phy;

/// Where a bare [`crate::Sim`] stands its bike: a foot above the floor, so the suspension has to
/// catch the bike rather than being handed the pose that is already its answer.
pub const SPAWN: Vf = Vf([0.0, 1.0, 0.0, 1.0]);
/// Which way it points there: down +z, the way the crate's other tests face a bike.
pub const FACING: Vf = Vf([0.0, 0.0, 1.0, 0.0]);

/// The bike, in the game's own `.phy` format — hand-written, not the disc's (see the module's head).
pub const BIKE: &str = "\
# A bike, hand-written for the port's own harness — NOT the disc's.
#
# `.phy` is the game's parameter format and `phy::Phy::read_bytes` is the ported loader, so a
# stand-in bike is written in it. Nothing here came off a disc: it is a plausible 250 cc four-stroke
# and no number in it is claimed to be the game's. The keys are the ones `ride.rs` actually drives;
# the loader's defaults cover the rest of the file.
#
# Units are the simulation's: feet, slugs, pounds of force, g = 32.2 ft/s^2. The per-surface grip
# keys are coefficients *per unit of tire load*, not pounds, which is why they are small numbers.
#
# `BodyTireForcePosition` is where the spin step puts the tire's force on the chassis, a short way
# under the centre of gravity, not where the wheel hangs: the hubs are `MOUNTS` below. The `Skid*`
# and `Slipping*` keys are what a sliding, spinning or locked tire still pushes with (S58).
[Mass Properties]
Weight = 400
Gravity = 1
Inertia = 22.4, 18.4, 10.9
CofG = 0.0, 1.6, 0.0
BalanceCofG = 0.0, 1.6, 0.0
CofGTravelForward = 0.5
CofGTravelBack = 0.5
[Front Tire]
StrutCompression = 1.0
Spring = 2.5
SpringWhenStopped = 2.5
BounceDampingRatio = 0.4
ReboundDampingRatio = 0.6
TireRadius = 1.08
TireInertia = 1.2
SteeringTorque = 40
SteeringDamping = 8
SteeringInertia = 2
MaxSteerAngle = 30
BodyTireForcePosition = 0.0, -0.3, 0.8
NumberOfSurfaces = 1
WheelFriction1 = 8.0
WheelDrag1 = 0.001
MaxBrakingTorque1 = 330
MaxDriveForce1 = 1.5
SkidDriveForce1 = 1.4
MaxSideForce1 = 3.5
SideForce1 = 0.2
SkidSideForce1 = 3.3
SlippingSideForce1 = 0.2
MaxSlippingSideForce1 = 3.5
SkidSlippingSideForce1 = 3.3
[Rear Tire]
StrutCompression = 1.0
Spring = 2.2
SpringWhenStopped = 2.2
BounceDampingRatio = 0.4
ReboundDampingRatio = 0.6
TireRadius = 1.12
TireInertia = 1.4
BodyTireForcePosition = 0.0, -0.3, -0.8
OtherTireNormalLoadScale = 1.0
NumberOfSurfaces = 1
WheelFriction1 = 9.0
WheelDrag1 = 0.001
MaxBrakingTorque1 = 400
MaxDriveForce1 = 2.4
SkidDriveForce1 = 2.2
MaxSideForce1 = 4.0
SideForce1 = 0.25
SkidSideForce1 = 3.8
SlippingSideForce1 = 0.25
MaxSlippingSideForce1 = 4.0
SkidSlippingSideForce1 = 3.8
[Engine]
NumberOfRPMPoints = 4
RPM1 = 2000
HP1 = 6
RPM2 = 5000
HP2 = 26
RPM3 = 8000
HP3 = 38
RPM4 = 11000
HP4 = 30
NumberOfGears = 5
GearRatio1 = 2.1
GearRatio2 = 1.7
GearRatio3 = 1.4
GearRatio4 = 1.15
GearRatio5 = 1.0
FinalGearRatio = 3.6
EngineInertia = 0.4
MinRPM = 1800
IdleThrottle = 0.08
EngineBraking = 0.3
ClutchStaticFriction = 90
ClutchKineticFriction = 70
ClutchTorqueScaleTime = 1.5
MaxClutchTorqueScale = 2.5
MaxClutchTorqueScaleSpeed = 15
ClutchCofGTravelBack = 0.2
UpShiftRPM = 9500
DownShiftRPM = 3000
ReverseGain = 40
ReverseSpeed = -10
[Aerodynamics]
Drag = 0.5
[Joystick]
XAxisRateLimit = 6
YAxisRateLimit = 6
GasRateLimit = 4
BrakeRateLimit = 6
ClutchInRateLimit = 8
ClutchOutRateLimit = 4
[Roll Control]
RollGain = 6
MaxRollCommand = 0.4
[Pitch Control]
WheelieTopSpeed = 40
[Collisions]
HighSpeedRayOffset = 0, 3, 0
HighSpeedMinCollisionAngle = 45
";

/// The stand-in bike's parameters, through the ported loader.
pub fn phy(curve: &CurveTable) -> Phy {
    Phy::read_bytes(BIKE.as_bytes(), curve)
}

/// Where the stand-in's two wheels hang off its chassis: its hubs, in the chassis' frame.
pub const MOUNTS: (Vf, Vf) = (Vf([0.0, 1.1, 2.1, 1.0]), Vf([0.0, 1.1, -2.1, 1.0]));

/// Where the two wheels hang off the chassis: the stand-in's [`MOUNTS`], whichever `.phy` it is
/// handed.
///
/// The original reads each one off the bike's model — the front and rear wheel nodes — which is the
/// renderer's side of the disc; a bike with no model has to put a hub somewhere. Until S58 that was
/// the `.phy`'s own `BodyTireForcePosition`, but that key is not where a wheel is: it is the arm the
/// spin step puts the tire's force on the chassis at, and the original's bike has it a short way
/// under the centre of gravity. A hub-length arm above it turns every correction the tire makes into
/// a bigger slide, so the stand-in's two are now different numbers and the hubs live here.
pub fn mounts(_: &Phy) -> (Vf, Vf) {
    MOUNTS
}
