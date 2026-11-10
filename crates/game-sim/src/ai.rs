//! The opponent riders: who they are and how good they are (S12a).
//!
//! When a world starts, the original spawns its players in one pass: the humans first, then as many AI
//! riders as it takes to make up the event's racer count. Each AI rider is given an identity — one of
//! the rider database's entries, drawn with `rand()` and never the same one twice — and a skill level
//! drawn from a range the event's AI difficulty byte decides. That pass is [`spawn_roster`]; the bikes
//! and the riding are later tasks (S12b–S12e).
//!
//! The draws are all the library's `rand()`, in a fixed order — the AllAI pair, then every identity
//! (re-drawn on a repeat), then one skill per AI rider in spawn order — so a port that agrees on the
//! roster but draws in another order would still put the generator out of step for everything after.
//!
//! Two things the original does here that the port leaves to its caller:
//!
//! * it seeds `srand` from the wall clock if nothing has seeded it yet. The caller hands in the
//!   generator it already has.
//! * an event marked as a career round takes its riders from a saved roster or the championship's own
//!   table instead of drawing them (S12f).
//!
//! [`controls`] is an AI rider's per-tick ground controller (S12b): the bar, lean, throttle and brake
//! that take its bike the way and at the speed the route follower asks for. The route follower, the
//! character file its [`Tuning`] comes from and the tick around it are later pieces of S12.
//!
//! [`plan_trick`] is when an AI rider asks for a trick and which (S12d): once a frame, off the ground
//! and with its cool-down spent, it estimates how long it has left in the air, rolls its trick
//! chance, and hands the estimate to the trick table's own chooser ([`Set::ai_choice`]); what that
//! picks is asked for with [`ask_for`]. [`gesture_gate`] is the AI rider's side of the trick request:
//! while an asked-for trick is starting or running it does not read the pad at all.
//!
//! [`avoid`] is the avoidance (S12c): every tick a motocross AI rider takes the direction its route
//! asks for and bends it away from the other riders near it, and may lower the speed it aims for.
//! That is the swerve and the brake an opponent shows on free09 when it comes up on another rider. It
//! also builds a fan of look-ahead feelers ([`feeler_fan`]) and sweeps it through the scene's objects
//! when the flags allow ([`look`]) — but the motocross update throws the sweep's answer away: nothing
//! it steers or brakes by reads the feelers. They are kept in the port because the original builds
//! them and the flags gate them; the scene sweep itself (a sweep of the fan against each solid object)
//! is not ported.
//!
//! [`path`] is the track's AI path as the AI queries it (S282): the nearest stretch of the racing line
//! to a point, whether a point is on the track between two nodes, and the line's direction at a node;
//! [`path::off_track`] puts the three together as [`avoid`]'s keep-on-the-track push.
//!
//! [`feed`] is the rider-to-brain copy (S12e): once a frame, before the brain thinks, the rider hands
//! it its bike's frame and motion, the ground under it and the ground's facing a little way ahead, and
//! the on-the-ground and tricking bits [`controls`] and [`plan_trick`] read; on a bump it may roll a
//! `rand()`, which puts it in the order of the frame's draws.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::camera::CurveTable;
use crate::ragdoll::{FineAcos, Tables};
use crate::rng::Rng;
use crate::trick::{Bike, Params, SLOTS, Set, Trick, Tricks};
use ps2_float::fpu::Acc;
use ps2_float::{Dest, F, Field, Vf, fpu, vu};

/// The first rider-database entry an AI rider can be. The entries before it are never drawn.
pub const FIRST_AI_IDENTITY: i32 = 22;

/// The most AI riders the original can draw identities for: it keeps them in a twenty-entry array on
/// its stack.
pub const MAX_AI: usize = 20;

/// What the spawn reads from the event and the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RosterIn {
    /// The settings' racer count, humans included.
    pub racers: i32,
    /// The human players asked for.
    pub humans: i32,
    /// The event's AI difficulty byte (90 when no menu has written it).
    pub difficulty: u8,
    /// How many entries the rider database has.
    pub identities: i32,
    /// The AllAI knob (0 on retail).
    pub all_ai: bool,
    /// The event is a ghost race (activity type 3).
    pub ghost_race: bool,
}

/// One AI rider as the spawn leaves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiRider {
    /// Its rider-database entry.
    pub identity: i32,
    /// Its skill level, in the difficulty's [`skill_range`].
    pub skill: i32,
}

/// The two numbers the AllAI knob draws before anything else: a racer number in `1..racers`, and a
/// time between 8 and 13. What reads them later was not traced (see the S12 journal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AllAiDraw {
    pub racer: i32,
    pub seconds: F,
}

/// Who is on the start line.
#[derive(Clone, Debug, PartialEq)]
pub struct Roster {
    /// Human players, numbered first (player 0 up). The AllAI knob makes this 1.
    pub humans: i32,
    /// A ghost race spawns its ghost (as player 1, after the one human) and no AI riders.
    pub ghost: bool,
    /// The AI riders, numbered after the humans in this order.
    pub ai: Vec<AiRider>,
    pub all_ai: Option<AllAiDraw>,
}

/// The AI skill range `(low, high)` for an event's difficulty byte, both ends included.
///
/// Two straight lines rounded to the nearest whole level and put in order: one is 4 from difficulty
/// 90 up (and rises as the difficulty falls below it), the other is 7 below 80 and falls from 7 to 1
/// between 80 and 100. 100 and up is level 1 only; the default of 90 gives `(4, 4)`.
pub fn skill_range(difficulty: u8) -> (i32, i32) {
    if difficulty >= 100 {
        return (1, 1);
    }
    let level = |from: f32, base: f32| {
        let t = (F::from(i32::from(difficulty)) - F(from)) / F(20.0);
        fpu::to_i32((F(0.5) + (F(base) + F(6.0) * (F(1.0) - t))).to_f32())
    };
    let a = if difficulty < 90 { level(70.0, 4.0) } else { 4 };
    let b = if difficulty < 80 { 7 } else { level(80.0, 1.0) };
    if a < b { (a, b) } else { (b, a) }
}

/// The original's player spawn, as far as the dice go: see the module docs.
///
/// Panics where the original would hang or run off its array: more AI riders than there are database
/// entries from [`FIRST_AI_IDENTITY`] up to tell apart, or more than [`MAX_AI`] of them.
pub fn spawn_roster(rng: &mut Rng, input: RosterIn) -> Roster {
    let mut humans = input.humans;
    let mut all_ai = None;
    if input.all_ai {
        humans = 1;
        let racer = rng.index(input.racers - 1) + 1;
        let seconds = F(8.0) + F(5.0) * rng.unit();
        all_ai = Some(AllAiDraw { racer, seconds });
    }
    if input.ghost_race {
        return Roster { humans: 1, ghost: true, ai: Vec::new(), all_ai };
    }

    let count = input.racers - humans;
    let mut identities: Vec<i32> = Vec::new();
    if count > 0 {
        // `index(n)` truncates toward zero, so it reaches |n| values (one when n is 0).
        let drawable = (input.identities - FIRST_AI_IDENTITY).abs().max(1);
        assert!(count as usize <= MAX_AI, "{count} AI riders: the original keeps room for {MAX_AI}");
        assert!(
            count <= drawable,
            "{count} AI riders from {drawable} drawable identities: the original re-draws for ever"
        );
        while identities.len() < count as usize {
            let id = rng.index(input.identities - FIRST_AI_IDENTITY) + FIRST_AI_IDENTITY;
            if !identities.contains(&id) {
                identities.push(id);
            }
        }
    }
    let (low, high) = skill_range(input.difficulty);
    let ai = identities
        .into_iter()
        .map(|identity| AiRider { identity, skill: low + rng.index(high - low + 1) })
        .collect();
    Roster { humans, ghost: false, ai, all_ai }
}

/// What the controller reads off the rider each tick: the bike's frame and motion as the interface
/// copied them in, the ground under it, and where the route follower wants it to go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Situation {
    /// The bike's up axis (`w` unread).
    pub up: Vf,
    /// The bike's forward axis. Its `w` lane comes through into [`Controls::flat_forward`].
    pub forward: Vf,
    /// The bike's height.
    pub height: F,
    /// The ground's height under the bike.
    pub ground: F,
    /// The ground's normal under the bike.
    pub ground_normal: Vf,
    /// The body's turn rates: `x` is the roll the pitch control damps, `y` the yaw the oversteer
    /// correction damps.
    pub spin: Vf,
    /// The bike's speed.
    pub speed: F,
    /// The rider's flag word.
    pub flags: u64,
    /// The way the route follower wants the bike to point.
    pub target_dir: Vf,
    /// The speed the route follower wants.
    pub target_speed: F,
    /// The slowest the controller lets the target speed be.
    pub min_speed: F,
    /// The pitch command from the tick before, which the landing pitch filter blends from.
    pub pitch: F,
}

/// The controller's tuning, most of it out of the rider's character section (S12 split, the reader).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// SteeringGain: radians off the target to steer.
    pub steering_gain: F,
    /// OversteerCorrectionGain: how much of the yaw rate comes off the steer, and of the roll rate
    /// off the air pitch.
    pub oversteer_gain: F,
    /// How hard the bike aims its nose at the target in the air.
    pub aim_gain: F,
    /// PitchControlDistance: below this height off the ground the bike sets up its landing.
    pub pitch_distance: F,
    /// PitchGain: the landing pitch command per unit the ground normal is off the landing attitude.
    pub pitch_gain: F,
    /// PitchJitterDeadZone: how much of the new landing pitch command a tick takes.
    pub pitch_filter: F,
    /// LandingPitchAngle, in radians.
    pub landing_pitch: F,
    /// SpeedMinTolerance: how far under the target the throttle stays wide open.
    pub slow_tolerance: F,
    /// SpeedMaxTolerance: how far over it the brake comes on.
    pub fast_tolerance: F,
    /// The cruise throttle: its share of the target speed, and its gain on the speed short of it.
    pub cruise_base: F,
    pub cruise_gain: F,
    /// The graded brake's gain on the speed over the target.
    pub brake_gain: F,
    /// What the steer and the pitch are scaled by before they are clamped.
    pub steer_scale: F,
    pub pitch_scale: F,
}

/// What the controller hands the bike, and the working values it leaves behind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    /// The bar, −1..1.
    pub steer: F,
    /// The rider's fore and aft lean, −1..1.
    pub pitch: F,
    /// The throttle and the brake, 0..1.
    pub throttle: F,
    pub brake: F,
    /// The steer angle before the sine: the heading error times the steering gain.
    pub heading: F,
    /// The aim error times the aim gain, negated.
    pub aim: F,
    /// The roll angle times the steering gain.
    pub roll: F,
    /// The forward axis flattened and normalised.
    pub flat_forward: Vf,
    /// The target speed after the minimum.
    pub target_speed: F,
}

/// The rider's flag bits the controller reads.
pub mod flag {
    /// The wheels are on the ground: no pitch command.
    pub const ON_GROUND: u64 = 0x20;
    /// Take the yaw rate off the steer.
    pub const OVERSTEER_CORRECTION: u64 = 0x1_0000;
    /// Cruise on a part throttle rather than coast inside the tolerance.
    pub const CRUISE: u64 = 0x1000_0000;
    /// Grade the brake by how far over the target speed the bike is.
    pub const GRADED_BRAKE: u64 = 0x2000_0000;
    /// The rider may trick at all. The AI's set-up turns it on.
    pub const TRICKS: u64 = 0x2;
    /// The rider has been on the ground since its last flight's trick roll, so this flight may roll.
    /// Cleared by a failed roll or an unspent cool-down; a roll that picks a trick leaves it set.
    pub const TRICK_READY: u64 = 0x8;
    /// Holds tricks off, as [`ON_GROUND`] does. [`feed`] sets it while the rider's own trick is
    /// running (S12e), so a flight's trick is not rolled for again under it.
    pub const NO_TRICKS: u64 = 0x10;
    /// Estimate the air time from the vertical speed and the height over the ground rather than from
    /// the route's jump. The AI's set-up turns it on.
    pub const BALLISTIC: u64 = 0x2000;
    /// A trick was asked for in this flight: the landing restarts the cool-down.
    pub const TRICKED: u64 = 0x8_0000;
    /// Set and cleared tick by tick by the controller: with it up the feelers are not swept at riders.
    pub const NO_RIDER_SWEEP: u64 = 0x80;
    /// Set on Free06 and Free10 only, when a freeride attaches its AI. With [`NO_RIDER_SWEEP`] also up,
    /// no feeler fan is built at all.
    pub const QUIET_AREA: u64 = 0x40;
    /// Cleared by every avoidance pass. What sets it was not traced.
    pub const AVOID_DUE: u64 = 0x100;
    /// Push back toward the track's path when the point ahead is off it.
    pub const KEEP_ON_TRACK: u64 = 0x20000;
    /// Leave out the riders whose own flag word has bit 4 set.
    pub const SKIP_MARKED: u64 = 0x100000;
}

const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;
const HALF_PI: f32 = 1.5707964;
const INV_TWO_PI: f32 = 0.15915494;
/// The curve table's step and index scale, as the game keeps them beside the table.
pub const CURVE_STEP: f32 = 0.0010473721;
pub const CURVE_INV_STEP: f32 = 954.77045;

/// `a` less whole turns, then held to −π..π, with the original's tests: `<= π` first, `< −π` only if
/// that passed.
fn wrap_turn(a: F) -> F {
    let a = a - F(TWO_PI) * F(fpu::from_i32((F(INV_TWO_PI) * a).to_i32()));
    if !(a <= PI) {
        a - TWO_PI
    } else if a < -PI {
        F(TWO_PI) + a
    } else {
        a
    }
}

/// The curve table entry under `a` (wrapped twice, as the game does) and what is left over.
fn curve_entry(curve: &CurveTable, a: F) -> (F, F, F) {
    let s = (F(PI) + wrap_turn(wrap_turn(a))) * CURVE_INV_STEP;
    let i = s.to_i32();
    let frac = s - F(fpu::from_i32(i));
    let [cos, sin] = curve.entries()[i as usize];
    (F(cos), F(sin), frac)
}

/// The controller's inline sine: the entry's sine plus the step times (cosine times leftover). The
/// shared [`CurveTable::sine`] multiplies the other way round.
fn sine(curve: &CurveTable, a: F) -> F {
    let (cos, sin, frac) = curve_entry(curve, a);
    sin + F(CURVE_STEP) * (cos * frac)
}

/// The inline cosine: the entry's cosine less the step times (sine times leftover).
fn cosine(curve: &CurveTable, a: F) -> F {
    let (cos, sin, frac) = curve_entry(curve, a);
    cos - F(CURVE_STEP) * (sin * frac)
}

/// `x` held inside −1..1, `<= 1` tested first.
fn clamp_unit(x: F) -> F {
    if !(x <= 1.0) {
        F::ONE
    } else if x < -1.0 {
        F(-1.0)
    } else {
        x
    }
}

/// `x` held inside 0..1, `<= 1` tested first.
fn clamp_01(x: F) -> F {
    if !(x <= 1.0) {
        F::ONE
    } else if x < F::ZERO {
        F::ZERO
    } else {
        x
    }
}

/// `x` held inside −π/2..π/2, `< −π/2` tested first.
fn clamp_quarter(x: F) -> F {
    if x < -HALF_PI {
        F(-HALF_PI)
    } else if !(x <= HALF_PI) {
        F(HALF_PI)
    } else {
        x
    }
}

/// The game's arc cosine helper: `x` clamped twice, then the fine table from whichever end is
/// nearer.
fn acos(fine: &FineAcos, x: F) -> F {
    let x = clamp_unit(clamp_unit(x));
    if x < F::ZERO {
        F(PI) + F(-1.0) * fine.at(x + F::ONE)
    } else {
        F::ZERO + F::ONE * fine.at(F::ONE - x)
    }
}

/// `a·b` on the FPU's accumulator, `x` then `y` then `z`.
fn dot(a: Vf, b: Vf) -> F {
    F(fpu::madd(fpu::madda(fpu::mula(a.x(), b.x()), a.y(), b.y()), a.z(), b.z()))
}

/// The signed angle whose cosine is `along` over the length of (`across`, `along`), negative when
/// `across` is.
fn signed_angle(fine: &FineAcos, across: F, along: F) -> F {
    let r = F(fpu::rsqrt(1.0, fpu::madd(fpu::mula(across.0, across.0), along.0, along.0)));
    let a = acos(fine, along * r);
    if across < F::ZERO { -a } else { a }
}

/// One tick of an AI rider's ground controller: the bar, the lean, the throttle and the brake that
/// take the bike the way and at the speed the route follower asks.
///
/// * The steer is the sine of the heading error — the angle between the flattened forward axis and
///   the target direction, signed by which side the target is on — times the steering gain, held to a
///   quarter turn each way, less the yaw rate times the oversteer gain if that correction is on.
/// * In the air the lean aims the nose: high up, the sine of the roll rate against the aim error; near
///   the ground, a filtered command that turns the ground normal toward the landing attitude (the
///   landing pitch angle up from the forward axis). On the ground it is zero.
/// * Under the target speed by more than the slow tolerance the throttle is wide open; over it by
///   more than the fast tolerance the brake is on, fully or graded; in between it coasts or cruises.
pub fn controls(tables: &Tables, at: &Situation, tune: &Tuning) -> Controls {
    let curve = &tables.curve;
    let target_speed = if at.target_speed < at.min_speed { at.min_speed } else { at.target_speed };

    let flat = Vf([at.forward.x(), 0.0, at.forward.z(), at.forward.w()]);
    let sq = flat.mul(flat);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    let p = flat.merge(Dest::XYZ, flat.mul_q(vu::rsqrt(1.0, len2)));
    let side = Vf([p.z(), 0.0, fpu::neg(p.x()), 0.0]);

    let heading = tune.steering_gain * signed_angle(&tables.fine, dot(at.target_dir, side), dot(at.target_dir, p));
    let off = at.target_dir.sub(at.forward);
    let aim = -tune.aim_gain * signed_angle(&tables.fine, dot(off, at.up), dot(off, at.forward));
    let roll = tune.steering_gain * signed_angle(&tables.fine, dot(at.up, side), F(at.up.y()));

    let bar = if at.flags & flag::OVERSTEER_CORRECTION != 0 {
        heading - tune.oversteer_gain * F(at.spin.y())
    } else {
        heading
    };
    let steer = sine(curve, clamp_quarter(bar));

    let pitch = if at.flags & flag::ON_GROUND != 0 {
        F::ZERO
    } else if !(at.height - at.ground <= tune.pitch_distance) {
        sine(curve, clamp_quarter(tune.oversteer_gain * F(at.spin.x()) - aim))
    } else {
        let s = sine(curve, tune.landing_pitch);
        let c = cosine(curve, tune.landing_pitch);
        let lifted = at.up.mul(Vf([s.0; 4]));
        let level = at.forward.mul(Vf([c.0; 4]));
        let want = level.add(lifted);
        let push = tune.pitch_gain * -dot(at.ground_normal, want);
        let k = tune.pitch_filter;
        clamp_unit(F(fpu::madd(fpu::mula(at.pitch.0, (F::ONE - k).0), k.0, push.0)))
    };

    let steer = clamp_unit(steer * tune.steer_scale);
    let pitch = clamp_unit(pitch * tune.pitch_scale);

    let (t, v) = (target_speed, at.speed);
    let (throttle, brake) = if v < t - tune.slow_tolerance {
        (F::ONE, F::ZERO)
    } else if !(v <= t + tune.fast_tolerance) {
        let brake = if at.flags & flag::GRADED_BRAKE != 0 { F::ONE - tune.brake_gain * (v - t) } else { F::ONE };
        (F::ZERO, brake)
    } else if at.flags & flag::CRUISE != 0 {
        (F(fpu::madd(fpu::mula(t.0, tune.cruise_base.0), tune.cruise_gain.0, (t - v).0)), F::ZERO)
    } else {
        (F::ZERO, F::ZERO)
    };

    Controls {
        steer,
        pitch,
        throttle: clamp_01(throttle),
        brake: clamp_01(brake),
        heading,
        aim,
        roll,
        flat_forward: p,
        target_speed,
    }
}

/// The route node the AI rider is heading for, as far as the trick decision reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jump {
    /// The node's flag word: [`JUMP`] marks a take-off.
    pub flags: u32,
    /// How long the route's jump keeps the bike in the air.
    pub airtime: F,
}

/// A route node's flag bit: the node is a jump's take-off.
pub const JUMP: u32 = 0x20;

/// An AI rider's trick clocks and the dice it rolls against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrickPlan {
    /// The shortest time from one trick's landing to the next roll (10 s from the AI's set-up).
    pub cooldown: F,
    /// The time since the last trick's landing. Only runs on the frames the rider cannot roll.
    pub since: F,
    /// The chance of a trick each frame a roll is made, in percent (10 from the AI's set-up).
    pub chance: F,
    /// The air time the last roll expected.
    pub estimate: F,
    /// The slot the chooser last picked. A chooser that finds nothing leaves it alone.
    pub slot: i32,
    /// The time in the air in this flight.
    pub air: F,
}

/// What the trick decision reads off the rider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    /// The bike's vertical speed.
    pub vertical_speed: F,
    /// The bike's height, and the ground's under it.
    pub height: F,
    pub ground: F,
    /// The route node it is heading for, if it has one.
    pub jump: Option<Jump>,
}

/// The rider's "do this trick" call: any of the 24 gesture slots is asked for at once, as though its
/// gesture had been made, and the gesture is disarmed. Anything else is refused.
pub fn ask_for(tricks: &mut Tricks, slot: i32) -> bool {
    if !(0..SLOTS as i32).contains(&slot) {
        return false;
    }
    tricks.requested = Trick(slot as u8);
    tricks.restart = true;
    tricks.gesture.armed = false;
    true
}

/// Whether the AI rider's trick request holds without reading the pad: an AI-driven rider with a trick
/// asked for and not yet started, or one running.
pub fn gate_holds(ai_driven: bool, tricks: &Tricks) -> bool {
    ai_driven && (tricks.restart || tricks.in_progress)
}

/// The AI rider's trick request: [`gate_holds`], or the human rider's [`Tricks::request`] from the pad.
/// Returns whether a trick is wanted or running.
pub fn gesture_gate(ai_driven: bool, tricks: &mut Tricks, set: &Set, bike: Bike, params: Params) -> bool {
    gate_holds(ai_driven, tricks) || tricks.request(set, bike, params)
}

/// One frame of the AI rider's trick decision.
///
/// A rider (`rider` is `None` when the AI has none attached) rolls when it may trick, is in the air
/// with [`flag::TRICK_READY`] set and nothing holding tricks off, and is either over a jump's take-off
/// or estimating ballistically:
///
/// * with less than `cooldown` since the last trick's landing, it gives up for this flight;
/// * the estimate is the route jump's air time less the time already in the air, or — ballistic —
///   a sixteenth of the vertical speed while rising, and while falling a thirty-second of it plus the
///   root of a sixteenth of the height over the ground;
/// * `100 × unit()` over `chance` gives up for this flight; otherwise the chooser gets the estimate
///   less half a second, and a trick it picks is asked for and marks the flight [`flag::TRICKED`].
///
/// A frame that does not roll runs `since`, and on the ground re-readies the next flight. Last, the air
/// clock runs off the ground; on it, it stops, and a tricked flight's landing zeroes `since`.
///
/// A roll that picks a trick leaves the flight ready, so the next frame in the air rolls again and may
/// ask for another — the original's, and kept.
pub fn plan_trick(
    dt: F,
    flags: &mut u64,
    plan: &mut TrickPlan,
    at: Flight,
    rider: Option<(&Set, &mut Tricks)>,
    rng: &mut Rng,
) {
    let f = *flags;
    let over_jump = at.jump.is_some_and(|j| j.flags & JUMP != 0);
    let rolls = rider.is_some()
        && f & flag::TRICK_READY != 0
        && f & flag::TRICKS != 0
        && f & (flag::NO_TRICKS | flag::ON_GROUND) == 0
        && (over_jump || f & flag::BALLISTIC != 0);
    if !rolls {
        plan.since = plan.since + dt;
        if *flags & flag::ON_GROUND != 0 {
            *flags |= flag::TRICK_READY;
        }
    } else if plan.since < plan.cooldown {
        *flags &= !flag::TRICK_READY;
    } else {
        plan.estimate = if f & flag::BALLISTIC == 0 {
            // Only reached over a jump: the ballistic flag is the other way in.
            at.jump.map_or(F::ZERO, |j| j.airtime) - plan.air
        } else if at.vertical_speed <= F::ZERO {
            let fall = (F(0.0625) * (at.height - at.ground)).sqrt();
            F(0.03125) * at.vertical_speed + fall
        } else {
            F(0.0625) * at.vertical_speed
        };
        let roll = F(100.0) * rng.unit();
        if !(roll <= plan.chance) {
            *flags &= !flag::TRICK_READY;
        } else if let Some((set, tricks)) = rider {
            if let Some(slot) = set.ai_choice(plan.estimate - F(0.5), rng) {
                plan.slot = slot as i32;
                ask_for(tricks, plan.slot);
                *flags |= flag::TRICKED;
            }
        }
    }

    if *flags & flag::ON_GROUND == 0 {
        plan.air = plan.air + dt;
    } else {
        if *flags & flag::TRICKED != 0 {
            plan.since = F::ZERO;
            *flags &= !flag::TRICKED;
        }
        plan.air = F::ZERO;
    }
}

// --- avoidance (S12c) -------------------------------------------------------------------------------

/// How wide the motocross fan is, edge to edge: a quarter turn's half, as the original's literal.
/// (The generic vehicle AI fans twice as wide; that path is not ported.)
pub const MX_FAN_SPREAD: f32 = 0.7853982;

/// The avoidance's numbers. Each one is a key of the rider's AI character file where it has one, read
/// into the controller over the constructor's value given by [`Default`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvoidTuning {
    /// The square root of `NearCollisionRange`. Twice this is the reach of the near avoidance.
    pub near_range: F,
    /// The square root of `FarCollisionRange`: the reach of the far avoidance. Neither constructor
    /// writes it, so [`Default`] has 0.
    pub far_range: F,
    /// `FarCollisionAvoidanceRadius`: how far to the side a rider ahead still counts.
    pub far_radius: F,
    /// `FarCollisionAvoidanceResponseScale`.
    pub far_response: F,
    /// `FarCollisionBrakingApproachRateLimit`: seconds to reach a closing rider under which it brakes.
    pub far_approach_limit: F,
    /// `FarCollisionMinBrakingSpeed`: the far brake's speed, and the least speed it brakes from.
    pub far_min_braking_speed: F,
    /// `FarCollisionBrakingResponse`, scaling [`AvoidTuning::far_min_braking_speed`].
    pub far_braking_response: F,
    /// `NearCollisionSeperationRate`, a tenth of the file's value.
    pub near_separation: F,
    /// The near response between `MinNearCollisionAvoidanceResponseScale`, `Max…` and `OverDrive…`
    /// by the event's difficulty.
    pub near_response: F,
    /// `NearCollisionBrakingResponse`: the near brake aims for the rider ahead's speed times this.
    pub near_braking_response: F,
    /// `PassingSpeed`: a rider ahead this much slower or more is passed, not braked for.
    pub passing_speed: F,
    /// `NearCollisionMinBrakingSpeed`: a rider ahead slower than this is not braked for.
    pub near_min_braking_speed: F,
    /// `FrontLateralAvoidanceWindow` and `RearLateralAvoidanceWindow`: the stretch along the heading,
    /// rear bound first, in which a rider is avoided sideways.
    pub front_window: F,
    pub rear_window: F,
    /// How much of a rider's sideways drift is matched (no key; 0 from the constructor).
    pub drift_match: F,
    /// The far brake is for riders heading more across than this cosine (0.707).
    pub far_heading_cos: F,
    /// How far ahead the off-the-track test looks, and how hard it pushes back.
    pub edge_look_ahead: F,
    pub edge_push: F,
    /// The feeler fan: how many, how high above the rider they start, and how long they are.
    pub feelers: i32,
    pub feeler_height: F,
    pub feeler_length: F,
}

impl Default for AvoidTuning {
    /// What the vehicle AI's and the motocross AI's constructors leave before the file is read.
    fn default() -> Self {
        AvoidTuning {
            near_range: F(12.0),
            far_range: F(0.0),
            far_radius: F(7.0),
            far_response: F(0.125),
            far_approach_limit: F(2.0),
            far_min_braking_speed: F(20.0),
            far_braking_response: F(1.0),
            near_separation: F(0.2),
            near_response: F(0.1),
            near_braking_response: F(0.0),
            passing_speed: F(3.0),
            near_min_braking_speed: F(40.0),
            front_window: F(12.0),
            rear_window: F(12.0),
            drift_match: F(0.0),
            far_heading_cos: F(0.707),
            edge_look_ahead: F(10.0),
            edge_push: F(1.0),
            feelers: 11,
            feeler_height: F(3.0),
            feeler_length: F(100.0),
        }
    }
}

/// The AI rider itself, as the avoidance reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Me {
    pub position: Vf,
    /// A unit heading.
    pub heading: Vf,
    pub velocity: Vf,
    /// The speed handed in with the heading. Nothing runs unless it is above 0; the sideways terms are
    /// divided by it.
    pub speed: F,
    /// The controller's own speed, which the far brake tests.
    pub held_speed: F,
    /// Its bike's race progress, compared with the others' to tell who is ahead.
    pub progress: F,
}

/// Another rider in the race's list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Other {
    pub active: bool,
    /// Bit 4 of its flag word, which [`flag::SKIP_MARKED`] leaves out.
    pub marked: bool,
    pub position: Vf,
    pub velocity: Vf,
    pub heading: Vf,
    pub speed: F,
    pub progress: F,
}

/// What the avoidance hands back: the direction to steer, and the speed to aim for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steer {
    /// In: the direction the route asks for. Out: bent away from the riders, flattened and made unit.
    /// `w` is left as it came.
    pub direction: Vf,
    /// Written only when a rider calls for braking; otherwise left as it came.
    pub speed: F,
}

/// Which probing the flags allow this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    /// Build the feeler fan and walk the scene.
    pub fan: bool,
    /// Sweep the fan at the riders the walk finds.
    pub riders: bool,
    /// Take in the placed scenery groups the walk finds.
    pub scenery: bool,
}

/// The probing gate: the fan is skipped only when both [`flag::NO_RIDER_SWEEP`] and
/// [`flag::QUIET_AREA`] are up, the riders need the first down, and the scenery needs both down.
pub fn look(flags: u64) -> Look {
    let fan = flags & flag::NO_RIDER_SWEEP == 0 || flags & flag::QUIET_AREA == 0;
    Look {
        fan,
        riders: fan && flags & flag::NO_RIDER_SWEEP == 0,
        scenery: fan && flags & (flag::NO_RIDER_SWEEP | flag::QUIET_AREA) == 0,
    }
}

/// One look-ahead feeler: a ray from `origin` along `direction` (unit; `w` 0 here, whatever was in
/// the original's buffer there), `length` long.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feeler {
    pub origin: Vf,
    pub direction: Vf,
    pub length: F,
}

/// The fan of `tuning.feelers` rays spread evenly over `spread` either side of straight ahead, each
/// the heading turned about the vertical (its `y` kept) and made unit again, starting
/// `feeler_height` above `position`.
pub fn feeler_fan(tuning: &AvoidTuning, spread: F, position: Vf, heading: Vf, curve: &CurveTable) -> Vec<Feeler> {
    let n = tuning.feelers;
    let count = F(fpu::from_i32(n));
    let (hx, hy, hz) = (F(heading.x()), F(heading.y()), F(heading.z()));
    (0..n)
        .map(|i| {
            let angle = spread * (F(0.5) + (F(fpu::from_i32(i)) - F(0.5) * count)) / count;
            let (cos, sin) = turn(angle, curve);
            let x = Acc::mula(hx, cos).madd(hz, sin);
            let z = Acc::mula(hz, cos).msub(hx, sin);
            let direction = unit(Vf::new(x.0, hy.0, z.0, 0.0));
            let mut origin = position;
            origin.0[1] = (F(position.y()) + tuning.feeler_height).0;
            Feeler { origin, direction, length: tuning.feeler_length }
        })
        .collect()
}

/// One avoidance pass of the motocross AI. `others` is the race's rider list in its own order, with
/// this rider's own bike left out. `edge` answers the off-the-track test (only asked with
/// [`flag::KEEP_ON_TRACK`] up): given the point [`AvoidTuning::edge_look_ahead`] ahead it says `None`
/// when that point is on the track's path (or the path cannot place it), else the vector to push
/// back along — the path's unit direction at the bike's node, or the point itself when that node has
/// none.
///
/// With `me.speed` not above 0 nothing happens and `None` comes back. Otherwise the steer is bent and
/// [`flag::AVOID_DUE`] cleared, and the feeler fan comes back when [`look`] allows it.
pub fn avoid(
    tuning: &AvoidTuning, me: &Me, flags: &mut u64, others: &[Other],
    edge: Option<&mut dyn FnMut(Vf) -> Option<Vf>>, steer: &mut Steer, curve: &CurveTable,
) -> Option<Vec<Feeler>> {
    if me.speed <= F(0.0) {
        return None;
    }
    let fan = look(*flags).fan.then(|| feeler_fan(tuning, F(MX_FAN_SPREAD), me.position, me.heading, curve));

    let t = tuning;
    let h = me.heading;
    let mut push = Vf::new(0.0, 0.0, 0.0, 1.0);
    let (mut pushes, mut brakes, mut brake_sum) = (F(0.0), F(0.0), F(0.0));
    for r in others {
        if !r.active {
            continue;
        }
        let d = sub(r.position, me.position);
        let dist = dot(d, d).sqrt();
        if *flags & flag::SKIP_MARKED != 0 && r.marked {
            continue;
        }
        let along = dot(d, h);
        let side = sub(d, scale(h, along));
        let side_len = dot(side, side).sqrt();
        let side_dir = side.merge(Dest::XYZ, side.mul_q(vu::div(1.0, side_len.0)));
        let rel = sub(r.velocity, me.velocity);
        let closing = dot(rel, h);
        let drift = sub(rel, scale(h, closing));
        let drift_out = dot(drift, side_dir);
        let in_window = !(along <= t.rear_window) && along < t.front_window;
        let per_speed = |v: Vf| v.merge(Dest::XYZ, v.mul_q(vu::div(1.0, me.speed.0)));

        if dist < F(2.0) * t.near_range {
            if in_window {
                if drift_out < F(0.0) {
                    push = add(push, scale(per_speed(drift), t.near_response));
                }
                push = sub(push, scale(side_dir, t.near_separation));
                pushes = pushes + F(1.0);
            }
            if side_len < t.near_range
                && in_window
                && !(me.progress <= r.progress)
                && me.speed - r.speed < t.passing_speed
                && !(r.speed <= t.near_min_braking_speed)
            {
                brakes = brakes + F(1.0);
                brake_sum = brake_sum + r.speed * t.near_braking_response;
            }
        }
        if dist < t.far_range {
            if !(along <= F(0.0)) && side_len < t.far_radius && closing < F(0.0) {
                push = add(push, scale(per_speed(scale(side_dir, closing)), t.far_response));
                pushes = pushes + F(1.0);
                if !(me.held_speed <= F(0.0))
                    && -along / closing <= t.far_approach_limit
                    && !(me.held_speed < t.far_min_braking_speed)
                    && dot(h, r.heading) < t.far_heading_cos
                {
                    brakes = brakes + F(1.0);
                    brake_sum = brake_sum + t.far_min_braking_speed * t.far_braking_response;
                }
            }
            if in_window {
                push = add(push, scale(per_speed(drift), t.drift_match));
                pushes = pushes + F(1.0);
            }
        }
    }
    if !(pushes <= F(0.0)) {
        steer.direction = add(steer.direction, push.merge(Dest::XYZ, push.mul_q(vu::div(1.0, pushes.0))));
    }
    if !(brakes <= F(0.0)) {
        steer.speed = brake_sum / brakes;
    }

    if *flags & flag::KEEP_ON_TRACK != 0 {
        if let Some(edge) = edge {
            let ahead = add(me.position, scale(h, t.edge_look_ahead));
            if let Some(q) = edge(ahead) {
                let across = Acc::mula(F(h.x()), F(q.z())).msub(F(h.z()), F(q.x()));
                if !(across <= F(0.0)) {
                    steer.direction = sub(steer.direction, scale(scale(q, across), t.edge_push));
                }
            }
        }
    }
    steer.direction.0[1] = 0.0;
    steer.direction = unit(steer.direction);
    *flags &= !flag::AVOID_DUE;
    fan
}

/// `cos` and `sin` of `angle` off the curve table the way the fan reads it: the entry below, each
/// corrected by the step times the other times what is left over — multiplied in that order, which
/// is not the order [`CurveTable::at`] uses.
fn turn(angle: F, curve: &CurveTable) -> (F, F) {
    let a = wrap_turn(wrap_turn(angle));
    let s = (F(PI) + a) * F(CURVE_INV_STEP);
    let left = s - F(fpu::from_i32(s.to_i32()));
    let (cos, sin) = curve.coarse(angle);
    (cos - F(CURVE_STEP) * (sin * left), sin + F(CURVE_STEP) * (cos * left))
}

/// `vsub.xyz` / `vadd.xyz`: `w` stays `a`'s.
fn sub(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

fn add(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `v.xyz * s` off a `qmtc2`'d register's `x`; `w` stays `v`'s.
fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::new(s.0, s.0, s.0, s.0).bc(Field::X)))
}

/// The VU0 normalise: `(x² + y²) + 1·z²`, `Q = 1/sqrt` by `vrsqrt`, `xyz · Q`.
fn unit(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, len2)))
}

// --- the rider-to-brain copy (S12e) ------------------------------------------------------------------

/// The brain's second flag word: the bits [`feed`] writes.
pub mod bump {
    /// Set together by [`feed`]'s roll when the bike leaves a bump fast enough and the roll comes in
    /// under the rider's chance, and cleared together otherwise. The state-3 branch of the AI's update
    /// reads [`B`] (S12e journal); what either one does to the riding is not traced.
    pub const A: u64 = 0x1;
    pub const B: u64 = 0x2;
}

/// What [`feed`] reads off the AI rider and its bike's body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rider {
    /// The bike's frame as four rows — the side axis, up, forward and the position — copied whole.
    pub frame: [Vf; 4],
    /// The body's velocity in the world, its acceleration, and its turn rates in its own frame.
    pub velocity: Vf,
    pub acceleration: Vf,
    pub spin: Vf,
    /// The body's speed along its own forward axis.
    pub forward_speed: F,
    /// Some wheel is on the ground.
    pub on_ground: bool,
    /// The rider's trick is running.
    pub tricking: bool,
    /// How far ahead, in seconds of the velocity across the ground, the ground's facing is looked up.
    pub look_ahead: F,
    /// The chance, in percent, that a bump sets the [`bump`] bits.
    pub bump_chance: F,
}

/// The brain's copy of its rider, which [`controls`], [`plan_trick`] and [`avoid`] read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Senses {
    /// The position from the frame before.
    pub last_position: Vf,
    /// [`Rider::frame`], as copied.
    pub frame: [Vf; 4],
    pub velocity: Vf,
    pub acceleration: Vf,
    pub spin: Vf,
    /// The length of the velocity.
    pub speed: F,
    /// The ground's facing at the look-ahead point.
    pub ground_normal: Vf,
    /// The ground's height under the bike.
    pub ground: F,
    /// The brain's flag word ([`flag`]); [`feed`] writes [`flag::ON_GROUND`] and [`flag::NO_TRICKS`].
    pub flags: u64,
    /// The brain's second flag word ([`bump`]).
    pub bumps: u64,
}

/// One frame of the rider-to-brain copy.
///
/// The position from the frame before is kept, then the bike's frame, velocity, acceleration and spin
/// are copied in and the speed is the velocity's length (`x·x + y·y + z·z` on the accumulator). The
/// ground is asked twice: its facing at the position moved [`Rider::look_ahead`] along the velocity
/// across the ground (the height is the bike's own), and its height straight under the bike. The on-the-
/// ground bit follows the rider's, the no-tricks bit its running trick.
///
/// On the ground, rising faster than 5 and going forward faster than 30, `100 × unit()` at or under
/// [`Rider::bump_chance`] sets both [`bump`] bits; any other way clears them. The draw is made only
/// past the three tests.
pub fn feed(rider: &Rider, brain: &mut Senses, world: &mut dyn crate::world::World, rng: &mut Rng) {
    brain.last_position = brain.frame[3];
    brain.frame = rider.frame;
    brain.velocity = rider.velocity;
    brain.acceleration = rider.acceleration;
    brain.spin = rider.spin;
    let v = rider.velocity;
    brain.speed = Acc::mula(v.x(), v.x()).madda(v.y(), v.y()).madd(v.z(), v.z()).sqrt();

    let at = brain.frame[3];
    let ahead = Vf::new(
        (F(at.x()) + F(v.x()) * rider.look_ahead).to_f32(),
        at.y(),
        (F(at.z()) + F(v.z()) * rider.look_ahead).to_f32(),
        at.w(),
    );
    brain.ground_normal = world.ground(ahead).1;
    if rider.on_ground {
        brain.flags |= flag::ON_GROUND;
    } else {
        brain.flags &= !flag::ON_GROUND;
    }
    brain.ground = F(world.ground(at).0.y());
    if rider.tricking {
        brain.flags |= flag::NO_TRICKS;
    } else {
        brain.flags &= !flag::NO_TRICKS;
    }

    let bumped = rider.on_ground
        && !(F(v.y()) <= F(5.0))
        && !(rider.forward_speed <= F(30.0))
        && F(100.0) * rng.unit() <= rider.bump_chance;
    if bumped {
        brain.bumps |= bump::B | bump::A;
    } else {
        brain.bumps &= !(bump::B | bump::A);
    }
}

// --- the track's AI path (S282) ----------------------------------------------------------------------

/// The track's AI path as the AI asks it things (S282): a chain of nodes down the racing line, each a
/// cross-section of the track — a centre and two edge points across it — with the length on to the
/// next node. Between a node and the next lies a six-sided patch of track (both cross-sections'
/// three points); [`segment_holds`] says whether a point is on that patch, [`locate`] finds the
/// stretch of line nearest a point, and [`node_direction`] is which way the line runs at a node.
///
/// Only the ground plane counts: every query reads `x` and `z` and leaves `y` alone.
///
/// [`off_track`] is the avoidance's keep-on-the-track test ([`avoid`]'s `edge`), made of the three.
/// The path's reading off the disc (S282c) and the follower that rides it (S282b) are not ported.
pub mod path {
    use super::{sub, unit};
    use ps2_float::fpu::Acc;
    use ps2_float::{F, Vf};

    /// One cross-section of the track.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Node {
        /// Where the racing line crosses it.
        pub centre: Vf,
        /// Its two ends, one either side of the centre (the original's first one, then the second).
        pub edges: [Vf; 2],
        /// The length on to [`Node::next`]; a stretch of nothing has 0.
        pub length: F,
        /// The node after this one, `None` at the path's end.
        pub next: Option<usize>,
    }

    /// The nodes of a track's AI path, linked by [`Node::next`].
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Net {
        pub nodes: Vec<Node>,
    }

    /// A point on the line: the stretch from `node` to its next, `along` that far through it (0..1).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Place {
        pub node: usize,
        pub along: F,
    }

    /// What [`locate`] finds.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Located {
        /// The nearest stretch, `None` when no stretch was taken (every one refused by `holding_only`).
        pub place: Option<Place>,
        /// How far the point is from it across the ground; −1 when nothing was taken, and the squared
        /// distance, not the distance, when that is not above 0 (the original takes the root only of a
        /// positive square).
        pub distance: F,
    }

    /// The nearest stretch of the line to `point`, walking from `first` down the chain. With `limit`
    /// not below 0 the walk stops once the stretches it has passed add up to `limit`; with
    /// `holding_only` a stretch counts only when [`segment_holds`] puts the point on it. Nearest is
    /// across the ground, to the stretch's centre line clamped to its ends; on a tie the earlier
    /// stretch keeps it. A path of one node places the point on that node, `along` 0.
    ///
    /// This is the lookup with no hint (the avoidance's): the original can also start from a place it
    /// found before, which is not ported (S282b).
    pub fn locate(net: &Net, first: usize, point: Vf, limit: F, holding_only: bool) -> Located {
        let n = &net.nodes;
        let (px, pz) = (F(point.x()), F(point.z()));
        if n[first].next.is_none() {
            let c = n[first].centre;
            let (dx, dz) = (px - F(c.x()), pz - F(c.z()));
            let distance = Acc::mula(dx, dx).madd(dz, dz).sqrt();
            return Located { place: Some(Place { node: first, along: F(0.0) }), distance };
        }
        let (mut best, mut place, mut walked) = (F(-1.0), None, F(0.0));
        let mut at = Some(first);
        while let Some(i) = at {
            let Some(j) = n[i].next else { break };
            if !(limit < F(0.0) || walked < limit) {
                break;
            }
            if !holding_only || segment_holds(net, point, i, j) {
                let (a, b) = (n[i].centre, n[j].centre);
                let (sx, sz) = (F(b.x()) - F(a.x()), F(b.z()) - F(a.z()));
                let (rx, rz) = (px - F(a.x()), pz - F(a.z()));
                let len = n[i].length;
                let t = Acc::mula(sx, rx).madd(sz, rz) / (len * len);
                let t = if !(t <= F(1.0)) {
                    F(1.0)
                } else if t < F(0.0) {
                    F(0.0)
                } else {
                    t
                };
                let (ox, oz) = (rx - sx * t, rz - sz * t);
                let d2 = Acc::mula(ox, ox).madd(oz, oz);
                if d2 < best || best < F(0.0) {
                    best = d2;
                    place = Some(Place { node: i, along: t });
                }
            }
            walked = walked + n[i].length;
            at = Some(j);
        }
        let distance = if best <= F(0.0) { best } else { best.sqrt() };
        Located { place, distance }
    }

    /// Whether `point` lies on the patch of track from node `seg` to node `next`: the six-sided outline
    /// through `seg`'s first edge, `next`'s first edge, centre and second edge, then `seg`'s second
    /// edge and centre, by counting the outline's sides it crosses going one way along `z`.
    pub fn segment_holds(net: &Net, point: Vf, seg: usize, next: usize) -> bool {
        let (a, b) = (&net.nodes[seg], &net.nodes[next]);
        let outline = [
            (a.edges[0], b.edges[0]),
            (b.edges[0], b.centre),
            (b.centre, b.edges[1]),
            (b.edges[1], a.edges[1]),
            (a.edges[1], a.centre),
            (a.centre, a.edges[0]),
        ];
        let (px, pz) = (F(point.x()), F(point.z()));
        let mut inside = false;
        for (from, to) in outline {
            let (fx, tx) = (F(from.x()), F(to.x()));
            let run = tx - fx;
            let s = run * (px - fx);
            if s < F(0.0) {
                continue;
            }
            let run2 = run * run;
            if !(s < run2) {
                continue;
            }
            let (fz, tz) = (F(from.z()), F(to.z()));
            if fz + s / run2 * (tz - fz) < pz {
                inside = !inside;
            }
        }
        inside
    }

    /// The unit direction from `node` on to its next node, written into `io`, when `node` has a next
    /// and a length above 0; otherwise `io` is left as it was and `false` comes back. The direction is
    /// the VU0's normalise of the difference, and its `w` is the next node's centre's.
    pub fn node_direction(net: &Net, node: Option<usize>, io: &mut Vf) -> bool {
        let Some(i) = node else { return false };
        let Some(j) = net.nodes[i].next else { return false };
        if net.nodes[i].length <= F(0.0) {
            return false;
        }
        *io = unit(sub(net.nodes[j].centre, net.nodes[i].centre));
        true
    }

    /// The avoidance's keep-on-the-track test, [`super::avoid`]'s `edge`: `ahead` placed on the path
    /// starting at `first`, then `None` when the stretch it is nearest, or the one after, holds it —
    /// or when the path ends within two nodes of that stretch — else the direction at the rider's own
    /// `node` ([`node_direction`]), or `ahead` itself when that node gives none.
    ///
    /// The original asks only when the race has a path net and the rider both a path and a node it
    /// is at; without them it does not push at all, so the caller hands [`super::avoid`] no `edge`.
    pub fn off_track(net: &Net, first: usize, node: usize, ahead: Vf) -> Option<Vf> {
        let seg = locate(net, first, ahead, F(-1.0), false).place?.node;
        let next = net.nodes[seg].next?;
        let after = net.nodes[next].next?;
        if segment_holds(net, ahead, seg, next) || segment_holds(net, ahead, next, after) {
            return None;
        }
        let mut io = ahead;
        node_direction(net, Some(node), &mut io);
        Some(io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The MXEvent default of 90 is the [4, 4] S14 read off the decompilation.
    #[test]
    fn the_default_difficulty_is_level_four() {
        assert_eq!(skill_range(90), (4, 4));
        assert_eq!(skill_range(100), (1, 1));
        assert_eq!(skill_range(75), (7, 9)); // 0.5 + 4 + 6 × 0.75 = 9 against the flat 7
    }

    /// The identities are all different and none comes from below the AI range.
    #[test]
    fn identities_never_repeat() {
        let mut rng = Rng::from_seed(7);
        let input = RosterIn {
            racers: 8,
            humans: 1,
            difficulty: 75,
            identities: 29,
            all_ai: false,
            ghost_race: false,
        };
        let roster = spawn_roster(&mut rng, input);
        let mut ids: Vec<i32> = roster.ai.iter().map(|r| r.identity).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 7);
        assert!(ids.iter().all(|&i| (FIRST_AI_IDENTITY..29).contains(&i)));
    }
}
