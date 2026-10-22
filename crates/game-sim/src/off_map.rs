//! **Off the map** (S59): the bike's cannonball. A rider who rides off the edge of the map is shot
//! back toward the middle of it, bike and all, and put back a few seconds after landing.
//!
//! It is the first clause of the original's bike wreck test, ahead of everything `crate::air`
//! ports, and the one clause that runs while the rider is already down. Three things gate it: wrecks
//! are on at all ([`crate::state::Sense::wrecks_enabled`]), the bike is one the clause is armed on, and
//! the reason the rider is down for is not already this one. The bike's builder arms it ([`armed`],
//! S73) for a bike built for a player, at an outdoor event (nationals or free ride), and leaves it off
//! for the AI's, a ghost's and every bike in a supercross stadium. The port keeps the answer on the
//! rider ([`crate::rider::Rider::edge_armed`]).
//!
//! # The test
//!
//! Two points are asked of the terrain's extent ([`crate::biplane::Area`]: root cells × 256 × the
//! sample spacing, across and along), each against a margin of [`MARGIN`] feet. One is the chassis'
//! centre of gravity. The other is the rider's main bone, which the body actor publishes onto the bike
//! every tick, and it is skipped when it is still the empty vector. Either one inside the margin, or
//! past the edge, or `NaN`, throws.
//!
//! # The throw ([`throw`])
//!
//! Toward the middle of the map, level with the bike, as fast as an arc of half the map's width
//! (at most [`REACH`] feet) needs under a gravity of [`ARC_G`], plus the arc's lift. Then:
//!
//! * the chassis' speed limit is raised to [`THROWN_MAX_SPEED`], until the put-back ([`unwreck`]);
//! * the chassis' body-frame spin is two draws of the game's `rand` (½ and 0.4 of a unit) and two
//!   constants;
//! * the transform is moved onto the centre of gravity;
//! * and the bike's air drag is halved ([`drag`], [`crate::ride::Ride::drag_halved`]). Nothing reads
//!   it while the rider is down (the bike is not simulated), and the bike's put-back doubles it again,
//!   so a ride never shows it; it is ported so that the bike's state is the original's throughout.
//!
//! Then the wreck is entered with the cannonball's reason, and the rest of the test is skipped. The
//! entry's own half for this reason is three more draws, which are the wreck body's spin instead of
//! the bike's ([`entry_spin`], handed to [`crate::ride::Crash::begin`]). The state machine waits
//! [`crate::state::SETTLE_CANNONBALL`] and then the bike's `CannonballWreckTime` before it puts the
//! rider back.
//!
//! The biplane's own map edge is the same shape with its own numbers and is in [`crate::biplane`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::biplane::Area;
use crate::chassis::Chassis;
use crate::r#move::NOTHING;
use crate::rng::Rng;
use ps2_float::fpu::Acc;
use ps2_float::{Dest, F, Vf};

/// How close to the edge of the map a point may come, in feet.
pub const MARGIN: F = F(150.0);

/// The farthest the throw aims to carry the bike: half the map, up to this many feet.
pub const REACH: F = F(f32::from_bits(0x44bb_3333));

/// The gravity the throw's arc is worked out under. The speed and the lift are over `2·√g`.
pub const ARC_G: F = F(f32::from_bits(0x40c6_c1fd));

/// The lift's two terms: this times `2·√g`, plus [`LIFT_OVER`] over it.
const LIFT_RATE: F = F(f32::from_bits(0x4180_cccd));
const LIFT_OVER: F = F(100.0);

/// The speed limit a thrown chassis is given, in feet a second (its square is what is kept).
pub const THROWN_MAX_SPEED: F = F(350.0);

/// The speed limit the bike's put-back gives the chassis again, feet a second: the rigid body's own.
pub const RIDING_MAX_SPEED: F = F(135.0);

/// What the throw multiplies the bike's air drag by; the put-back divides by it again.
pub const DRAG_SCALE: F = F(0.5);

/// The bike's air drag as the ground step reads it: the `.phy`'s `riding` one, or that times
/// [`DRAG_SCALE`] while `thrown`. The original's put-back divides the halved value by the scale, which
/// gives `riding` back bit for bit unless the halving flushed it to zero (a drag under 2⁻¹²⁵ ft⁻¹).
pub fn drag(riding: F, thrown: bool) -> F {
    if thrown { riding * DRAG_SCALE } else { riding }
}

/// The thrown chassis' spin: the first two lanes scale a draw each, the last two are constants.
const SPIN_X: F = F(0.5);
const SPIN_Y: F = F(f32::from_bits(0x3ecc_cccd));
const SPIN_Z: f32 = f32::from_bits(0x3ca3_d70a);

/// Who a bike is built for: the bike builder's fourth argument, which the vehicle loader takes off the
/// player's kind (a player 0, the AI 1, a ghost 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Built {
    Player,
    Ai,
    Ghost,
}

/// The event's league, its first word: which set of tracks the event is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum League {
    /// The stadiums.
    Supercross,
    /// The outdoor tracks.
    Nationals,
    /// The free-ride areas, the only ones the port rides.
    FreeRide,
}

/// **Whether the bike builder arms the map edge** (S73): only on a player's bike, and only at an
/// outdoor event. The builder writes the flag for every bike it builds — off for the AI's whatever the
/// league, and off in a stadium (whose ground has no edge to ride off) — so nothing is left over from
/// the bike's constructor, which clears it too.
pub fn armed(built: Built, league: League) -> bool {
    match built {
        Built::Ai => false,
        _ => match league {
            League::Nationals | League::FreeRide => built == Built::Player,
            League::Supercross => false,
        },
    }
}

/// Whether the bike is off the map: the rider's main bone (unless it is [`NOTHING`], bit for bit)
/// or the chassis' centre of gravity within [`MARGIN`] of the edge or past it. Both are asked, the
/// bone first.
pub fn outside(area: &Area, rider: Vf, cofg: Vf) -> bool {
    let unset = rider.0.iter().zip(NOTHING.0.iter()).all(|(a, b)| a.to_bits() == b.to_bits());
    let by_rider = !unset && area.outside(rider, MARGIN);
    let by_bike = area.outside(cofg, MARGIN);
    by_rider || by_bike
}

/// **The throw**, on the chassis and the transform's origin: see the module header. Draws twice from
/// `rng`.
pub fn throw(area: &Area, ch: &mut Chassis, origin: &mut Vf, rng: &mut Rng) {
    let half = F(0.5);
    let mid_z = half * area.depth;
    let mid_x = half * area.width;
    let at = ch.cofg_world;
    let target = Vf::new(mid_x.0, at.y(), mid_z.0, 1.0);
    let d = target.merge(Dest::XYZ, target.sub(at));
    let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
    let len2 = Acc::mula(x, x).madda(y, y).madd(z, z);
    let dir = if F::ZERO == len2 {
        NOTHING
    } else {
        let k = F::ONE.rsqrt(len2);
        Vf::new((x * k).0, (y * k).0, (z * k).0, d.w())
    };
    let root = F(2.0) * ARC_G.sqrt();
    let speed = REACH.min(mid_x) / root;
    let over = LIFT_OVER / root;
    let lift = LIFT_RATE * root + over;
    let vel = dir.merge(Dest::XYZ, dir.mul(Vf::splat(speed.0)));
    ch.vel = Vf::new(vel.x(), (F(vel.y()) + lift).0, vel.z(), vel.w());
    ch.max_speed_sq = THROWN_MAX_SPEED * THROWN_MAX_SPEED;
    let sx = SPIN_X * rng.unit();
    let sy = SPIN_Y * rng.unit();
    ch.ang_vel = Vf::new(sx.0, sy.0, SPIN_Z, 1.0);
    *origin = Vf::new(at.x(), at.y(), at.z(), origin.w());
}

/// **The cannonball's wreck entry**: three draws, the wreck body's spin `x`, `y` and `z` in that
/// order, each a unit. The entry leaves the body's `w` lane alone; `w` is handed through.
pub fn entry_spin(rng: &mut Rng, w: f32) -> Vf {
    let x = rng.unit();
    let y = rng.unit();
    let z = rng.unit();
    Vf::new(x.0, y.0, z.0, w)
}

/// **The bike's put-back**, its chassis' half: the motocross bike's own unwreck runs the vehicle's and
/// then sets the speed limit back to [`RIDING_MAX_SPEED`], whatever the wreck was. Only a cannonball
/// changes it, so after any other wreck it is a no-op. Measured: the north edge's capture reads 350²
/// from the throw to the put-back's tick and 135² on it (journal 2026-10-21-s59).
pub fn unwreck(ch: &mut Chassis) {
    ch.max_speed_sq = RIDING_MAX_SPEED * RIDING_MAX_SPEED;
}

/// [`unwreck`] on a whole bike, the one the rider's put-backs call: the riding limit it sets is the
/// ride's own ([`crate::ride::Ride::riding_max_speed_sq`]) — [`RIDING_MAX_SPEED`] squared on the
/// original's bike, and the raised one on a bike [`crate::modifiers::Knob::BikeSpeed`] made faster
/// (P101), which the stock limit would otherwise cut back to 135 ft/s at its first wreck.
pub fn unwreck_ride(ride: &mut crate::ride::Ride) {
    ride.chassis.max_speed_sq = ride.riding_max_speed_sq;
}
