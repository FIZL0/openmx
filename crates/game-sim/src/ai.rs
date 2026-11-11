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
//!
//! [`update`] is the AI tick itself (S281). It runs the stuck clock; then, while the rider is riding,
//! it runs the copy, the route follower, [`controls`], the steep-climb rule and [`hand_off`] in that
//! order (the steps it calls go through [`Steps`]). [`hand_off`] gives the bar, lean, gas and brake to
//! the bike, with the back-up and swerve recovery taking over the levers when they are active.
//! [`assist`] is the route follower's last step: by the rider's two weights it pulls the body's yaw
//! rate toward the brain's heading and, on the ground, the body's velocity toward the aim.
//!
//! [`seek`] is how an AI rider with no path gets round a free-ride map (S282a): it picks a jump, rides
//! to its setup point, lines up, launches, and learns from where it landed how to take the jump next
//! time; each tick it hands the brain the heading and speed [`controls`] rides by.
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
///
/// A track has two nets, both race files read the same way ([`crate::race::TrackFile`]) out of the
/// track's second resource, and [`Net::from_track`] makes either one: the avoidance's edge asks the
/// race's own (`<track>.tdf`, a start run and the lap), and the path follower rides the AI's
/// (`<track>AI.tdf`, a web of branching lines). The follower itself (S282b) is not ported.
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

    /// The nodes of a track's AI path, linked by [`Node::next`], and the paths they make up.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Net {
        pub nodes: Vec<Node>,
        /// The paths, in the file's order; empty for a net made by hand.
        pub paths: Vec<Path>,
    }

    /// One path of a [`Net`]: a run of nodes from `first` down [`Node::next`], and the paths it
    /// branches into at its end.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Path {
        /// The path's flags ([`crate::race::path_flags`]), as the load leaves them.
        pub flags: u32,
        /// The sum of its nodes' lengths.
        pub length: F,
        /// Its first node, in [`Net::nodes`].
        pub first: usize,
        /// The paths it leads on to, by index into [`Net::paths`].
        pub branches: Vec<usize>,
    }

    impl Net {
        /// A loaded track's paths as one net: each path's nodes in turn, its centre the node's
        /// first point, its edges the second and third, its length the node's span on to the next,
        /// and its next the node after it on the same path (`None` on a path's last node: a path
        /// goes on only through its branches).
        ///
        /// The original snaps every point's height onto the ground once the file is read (a ray
        /// down from 1000 above it into the track's collision); these are the heights as filed.
        pub fn from_track(track: &crate::race::TrackPaths) -> Net {
            let mut net = Net::default();
            for p in &track.paths {
                let first = net.nodes.len();
                let last = first + p.nodes.len() - 1;
                for (k, n) in p.nodes.iter().enumerate() {
                    let at = first + k;
                    net.nodes.push(Node {
                        centre: n.points[0],
                        edges: [n.points[1], n.points[2]],
                        length: n.span,
                        next: (at < last).then_some(at + 1),
                    });
                }
                net.paths.push(Path { flags: p.flags, length: p.length, first, branches: p.links.clone() });
            }
            net
        }
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

// --- the character file (S283) ------------------------------------------------------------------------

/// A Min/Max pair out of the character file, as the rider keeps it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pair {
    pub min: F,
    pub max: F,
}

/// A Min/Max/OverDrive triple out of the character file.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Triple {
    pub min: F,
    pub max: F,
    pub overdrive: F,
}

/// A Min/Max/OverDrive triple out of the character file, and the value the event's difficulty puts
/// between them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spread {
    pub min: F,
    pub max: F,
    pub overdrive: F,
    pub value: F,
}

/// The adaptive level of difficulty (`DefaultALODSettings`): how the brain eases off or pushes on by
/// how far it is from the player. Each one is its [`Pair`] put between by the difficulty.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Alod {
    /// `ALODWindow`: the lead within which the speed is adjusted at all.
    pub window: F,
    /// `ALODLowerLimit` and `ALODUpperLimit`: the least and most the target speed is scaled by.
    pub lower_limit: F,
    pub upper_limit: F,
    /// `ALODSpeed`, read in miles an hour and kept in feet a second.
    pub speed: F,
    /// `ALODLeadWindow`.
    pub lead_window: F,
}

/// What the character file sets on the rider's side of the AI (the brain's owner), as opposed to the
/// brain's own [`Character`] fields. The raw Min/Max/OverDrive numbers are kept as well as the value
/// put between them, because the reader keeps them and a key the file lacks leaves the old one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Profile {
    /// The `DefaultALODSettings` pairs, which [`Alod`] is put between.
    pub alod_window: Pair,
    pub alod_lower_limit: Pair,
    pub alod_upper_limit: Pair,
    pub alod_speed: Pair,
    pub alod_lead_window: Pair,
    /// `LookAheadGain`.
    pub look_ahead_gain: F,
    /// `…NearCollisionAvoidanceResponseScale`. The value put between them is
    /// [`AvoidTuning::near_response`].
    pub near_response: Triple,
    /// `…PreloadSuccessProbability`.
    pub preload_success: Spread,
    /// `PullBackSpeed`, `PullBackAmount`.
    pub pull_back_speed: F,
    pub pull_back_amount: F,
    /// `MinHangTimeForLandingLag`, `…LandingLagProbability`, `MaxLandingLagTime`.
    pub landing_lag_hang_time: F,
    pub landing_lag: Spread,
    pub landing_lag_time: F,
    /// `…HorsePowerScale`.
    pub horse_power: Spread,
    /// `OverPowerPhysicsTurnRateTweak`, `OverPowerPhysicsVelocityTweak`.
    pub turn_rate_tweak: F,
    pub velocity_tweak: F,
    /// `…BrilliantMomentPercent`, `BrilliantMomentMinDuration`, `BrilliantMomentRandomDuration`.
    pub brilliant_moment: Spread,
    pub brilliant_min_duration: F,
    pub brilliant_random_duration: F,
    /// `PreloadPercentagePerMPHDifference`, kept per foot a second.
    pub preload_per_speed: F,
    /// `…GLoadWreckScale`.
    pub g_load_wreck: Spread,
    /// `RandomStartGateDelay`, already scaled by the reader's draw.
    pub start_gate_delay: F,
    /// `ClutchStartProbability`, `WheelieStartProbability`.
    pub clutch_start_probability: F,
    pub wheelie_start_probability: F,
    /// `WheelieStartMinDuration`; `WheelieStartMaxDuration`, already moved toward a half by the
    /// reader's second draw.
    pub wheelie_start_min: F,
    pub wheelie_start_max: F,
    /// `WheelieMin/MaxPullBackWithClutch`, `WheelieMin/MaxPullBackWithoutClutch`.
    pub wheelie_pull_back_clutch: Pair,
    pub wheelie_pull_back: Pair,
    /// `CrossUpProbability`, `CrossUpMinDuration`, `CrossUpMaxDuration`, `CrossUpMinHeight`.
    pub crossup_probability: F,
    pub crossup_min_duration: F,
    pub crossup_max_duration: F,
    pub crossup_min_height: F,
}

/// Everything an AI rider's character file sets: the controller's and the avoidance's tuning, the rest
/// of the brain's, and the rider's [`Profile`]. The reader updates one of these in place, so it starts
/// as the AI's constructors left it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    pub tuning: Tuning,
    pub avoid: AvoidTuning,
    pub alod: Alod,
    /// `SteeringJitterFilter`, `SteeringRateFilter`: four lanes each.
    pub steering_jitter_filter: Vf,
    pub steering_rate_filter: Vf,
    /// `MinRandomOffset`, `MaxRandomOffset`, `MaxDriftSize`, `MaxDesiredOffsetDriftRate`: the route
    /// follower's line across the track.
    pub min_random_offset: F,
    pub max_random_offset: F,
    pub max_drift_size: F,
    pub max_drift_rate: F,
    /// `NoTrackOffsetLookAhead`, `NoTrackOffsetMinTurnDP`.
    pub no_track_look_ahead: F,
    pub no_track_min_turn: F,
    /// `MinLookAheadDistance`, `MaxLookAheadDistance`.
    pub min_look_ahead: F,
    pub max_look_ahead: F,
    /// `RollGain`, `RollControlDistance`.
    pub roll_gain: F,
    pub roll_distance: F,
    /// `MinimumStuntInterval`, `StuntProbability`.
    pub min_stunt_interval: F,
    pub stunt_probability: F,
    pub profile: Profile,
}

/// Where the event's difficulty byte puts a Min/Max/OverDrive spread: `(byte − 70)·0.0333333` up to
/// 100 (Min to Max), `(byte − 100)·0.04` past it (Max to OverDrive). Returns the fraction and whether
/// the byte is in the lower half.
pub fn difficulty_fraction(difficulty: u8) -> (F, bool) {
    let d = F::from(i32::from(difficulty));
    let lower = d <= F(100.0);
    let t = if lower { F(0.0333333) * (d - F(70.0)) } else { F(0.04) * (d - F(100.0)) };
    (t, lower)
}

/// Reads an AI rider's character file (`Rider\AIChrctr.aip`) into `c`.
///
/// `file` is the parsed file, or `None` when it is not there, in which case nothing changes and no
/// draw is made. The `DefaultALODSettings` section, when the file has it, gives the five [`Alod`]
/// pairs. Then the section `AI_<skill>` is selected and everything else is read from it; in a file that
/// lacks it every key reads as missing (the defaults, the post-read steps and the two draws still run).
///
/// A key the file does not have leaves the old value, except the ones the original reads with a
/// default (the `FarCollision…` ones, `NearCollisionMinBrakingSpeed`, the horse power, overpower,
/// brilliant moment, preload and g-load keys), and the post-read steps run on whatever is there:
/// `Near/FarCollisionRange` are square-rooted, `NearCollisionSeperationRate` is taken a tenth of,
/// `LandingPitchAngle` turned to radians, `ALODSpeed` and `PreloadPercentagePerMPHDifference` from
/// miles an hour to feet a second.
///
/// Every value with a Min and Max (and OverDrive) is put between them as `mula(lo, 1−t) + madd(hi, t)`
/// with `t` from [`difficulty_fraction`]: Min to Max up to 100, Max to OverDrive past it. The ALOD
/// pairs have no OverDrive and take Max past 100.
///
/// Two `rand()` draws end it: `RandomStartGateDelay` is scaled by one [`Rng::unit`], and
/// `WheelieStartMaxDuration` becomes `0.5 + (max − 0.5)·unit`.
pub fn read_character(file: Option<&crate::phy::Ini>, skill: i32, difficulty: u8, rng: &mut Rng, c: &mut Character) {
    let Some(ini) = file else { return };
    let (t, lower) = difficulty_fraction(difficulty);
    let u = F(1.0) - t;
    let lerp = |lo: F, hi: F| Acc::mula(lo, u).madd(hi, t);
    let between = |min: F, max: F, overdrive: F| if lower { lerp(min, max) } else { lerp(max, overdrive) };
    let spread = |s: &mut Spread| s.value = between(s.min, s.max, s.overdrive);

    let defaults = ini.section("DefaultALODSettings");
    let p = &mut c.profile;
    if defaults.found() {
        let pair = |s: &crate::phy::Section, p: &mut Pair, min: &str, max: &str| -> F {
            p.min = s.float(min).unwrap_or(p.min);
            p.max = s.float(max).unwrap_or(p.max);
            if lower { lerp(p.min, p.max) } else { p.max }
        };
        c.alod.window = pair(&defaults, &mut p.alod_window, "MinALODWindow", "MaxALODWindow");
        c.alod.lower_limit = pair(&defaults, &mut p.alod_lower_limit, "MinALODLowerLimit", "MaxALODLowerLimit");
        c.alod.upper_limit = pair(&defaults, &mut p.alod_upper_limit, "MinALODUpperLimit", "MaxALODUpperLimit");
        c.alod.speed = pair(&defaults, &mut p.alod_speed, "MinALODSpeed", "MaxALODSpeed") * F(1.4666667);
        c.alod.lead_window = pair(&defaults, &mut p.alod_lead_window, "MinALODLeadWindow", "MaxALODLeadWindow");
    }
    // A file without the rider's own section reads nothing more: the select that misses leaves no
    // section selected rather than the ALOD one.
    let s = ini.section(&format!("AI_{skill}"));
    let get = |v: &mut F, key: &str| *v = s.float(key).unwrap_or(*v);
    let or = |key: &str, default: f32| s.float_or(key, F(default));
    let get3 = |v: &mut Spread, min: &str, max: &str, overdrive: &str| {
        get(&mut v.min, min);
        get(&mut v.max, max);
        get(&mut v.overdrive, overdrive);
    };
    let or3 = |v: &mut Spread, keys: [(&str, f32); 3]| {
        v.min = or(keys[0].0, keys[0].1);
        v.max = or(keys[1].0, keys[1].1);
        v.overdrive = or(keys[2].0, keys[2].1);
    };

    c.steering_jitter_filter = s.vector("SteeringJitterFilter", c.steering_jitter_filter);
    c.steering_rate_filter = s.vector("SteeringRateFilter", c.steering_rate_filter);
    let tu = &mut c.tuning;
    get(&mut tu.steering_gain, "SteeringGain");
    get(&mut c.min_random_offset, "MinRandomOffset");
    get(&mut c.max_random_offset, "MaxRandomOffset");
    get(&mut c.max_drift_size, "MaxDriftSize");
    get(&mut c.max_drift_rate, "MaxDesiredOffsetDriftRate");
    get(&mut c.no_track_look_ahead, "NoTrackOffsetLookAhead");
    get(&mut c.no_track_min_turn, "NoTrackOffsetMinTurnDP");
    get(&mut c.max_look_ahead, "MaxLookAheadDistance");
    get(&mut c.min_look_ahead, "MinLookAheadDistance");
    get(&mut p.look_ahead_gain, "LookAheadGain");
    get(&mut tu.oversteer_gain, "OversteerCorrectionGain");
    get(&mut tu.slow_tolerance, "SpeedMinTolerance");
    get(&mut tu.fast_tolerance, "SpeedMaxTolerance");

    let av = &mut c.avoid;
    let near = &mut p.near_response;
    get(&mut near.min, "MinNearCollisionAvoidanceResponseScale");
    get(&mut near.max, "MaxNearCollisionAvoidanceResponseScale");
    get(&mut near.overdrive, "OverDriveNearCollisionAvoidanceResponseScale");
    av.near_response = between(near.min, near.max, near.overdrive);
    get(&mut av.near_range, "NearCollisionRange");
    av.near_range = av.near_range.sqrt();
    get(&mut av.far_range, "FarCollisionRange");
    av.far_range = av.far_range.sqrt();
    av.far_radius = or("FarCollisionAvoidanceRadius", 7.0);
    av.far_response = or("FarCollisionAvoidanceResponseScale", 0.125);
    av.far_approach_limit = or("FarCollisionBrakingApproachRateLimit", 2.0);
    av.far_min_braking_speed = or("FarCollisionMinBrakingSpeed", 20.0);
    av.far_braking_response = or("FarCollisionBrakingResponseScale", 1.0);
    get(&mut av.near_separation, "NearCollisionSeperationRate");
    av.near_separation = av.near_separation * F(0.1);
    get(&mut av.near_braking_response, "NearCollisionBrakingResponse");
    get(&mut av.passing_speed, "PassingSpeed");
    av.near_min_braking_speed = or("NearCollisionMinBrakingSpeed", 20.0);
    get(&mut av.front_window, "FrontLateralAvoidanceWindow");
    get(&mut av.rear_window, "RearLateralAvoidanceWindow");

    get(&mut c.roll_gain, "RollGain");
    get(&mut c.roll_distance, "RollControlDistance");
    get(&mut tu.pitch_distance, "PitchControlDistance");
    get(&mut tu.pitch_gain, "PitchGain");
    get(&mut tu.pitch_filter, "PitchJitterDeadZone");
    get(&mut c.min_stunt_interval, "MinimumStuntInterval");
    get(&mut c.stunt_probability, "StuntProbability");
    get3(
        &mut p.preload_success,
        "MinPreloadSuccessProbability",
        "MaxPreloadSuccessProbability",
        "OverDrivePreloadSuccessProbability",
    );
    spread(&mut p.preload_success);
    get(&mut p.pull_back_speed, "PullBackSpeed");
    get(&mut p.pull_back_amount, "PullBackAmount");
    get(&mut tu.landing_pitch, "LandingPitchAngle");
    tu.landing_pitch = tu.landing_pitch * F(0.017453292);
    get(&mut p.landing_lag_hang_time, "MinHangTimeForLandingLag");
    get3(
        &mut p.landing_lag,
        "MinLandingLagProbability",
        "MaxLandingLagProbability",
        "OverDriveLandingLagProbability",
    );
    spread(&mut p.landing_lag);
    get(&mut p.landing_lag_time, "MaxLandingLagTime");
    or3(
        &mut p.horse_power,
        [("MinHorsePowerScale", 80.0), ("MaxHorsePowerScale", 98.0), ("OverDriveHorsePowerScale", 115.0)],
    );
    spread(&mut p.horse_power);
    p.turn_rate_tweak = or("OverPowerPhysicsTurnRateTweak", 0.5);
    p.velocity_tweak = or("OverPowerPhysicsVelocityTweak", 0.0001);
    or3(
        &mut p.brilliant_moment,
        [
            ("MinBrilliantMomentPercent", 25.0),
            ("MaxBrilliantMomentPercent", 50.0),
            ("OverDriveBrilliantMomentPercent", 100.0),
        ],
    );
    spread(&mut p.brilliant_moment);
    p.brilliant_min_duration = or("BrilliantMomentMinDuration", 2.5);
    p.brilliant_random_duration = or("BrilliantMomentRandomDuration", 4.0);
    p.preload_per_speed = or("PreloadPercentagePerMPHDifference", 10.0) * F(0.006818182);
    or3(
        &mut p.g_load_wreck,
        [("MinGLoadWreckScale", 80.0), ("MaxGLoadWreckScale", 100.0), ("OverDriveGLoadWreckScale", 102.0)],
    );
    spread(&mut p.g_load_wreck);
    get(&mut p.start_gate_delay, "RandomStartGateDelay");
    get(&mut p.clutch_start_probability, "ClutchStartProbability");
    get(&mut p.wheelie_start_probability, "WheelieStartProbability");
    get(&mut p.wheelie_start_min, "WheelieStartMinDuration");
    get(&mut p.wheelie_start_max, "WheelieStartMaxDuration");
    get(&mut p.wheelie_pull_back_clutch.min, "WheelieMinPullBackWithClutch");
    get(&mut p.wheelie_pull_back_clutch.max, "WheelieMaxPullBackWithClutch");
    get(&mut p.wheelie_pull_back.min, "WheelieMinPullBackWithoutClutch");
    get(&mut p.wheelie_pull_back.max, "WheelieMaxPullBackWithoutClutch");
    p.start_gate_delay = p.start_gate_delay * rng.unit();
    let r = rng.unit();
    p.wheelie_start_max = F(0.5) + (p.wheelie_start_max - F(0.5)) * r;
    get(&mut p.crossup_probability, "CrossUpProbability");
    get(&mut p.crossup_min_duration, "CrossUpMinDuration");
    get(&mut p.crossup_max_duration, "CrossUpMaxDuration");
    get(&mut p.crossup_min_height, "CrossUpMinHeight");
}

// --- the AI tick (S281) -------------------------------------------------------------------------------

/// The brain flag bits the tick itself reads or writes (beside [`flag`]'s).
pub mod tick_flag {
    /// Last tick's [`super::flag::ON_GROUND`]: the tick copies it here before the rider-to-brain copy
    /// brings the new one in.
    pub const WAS_ON_GROUND: u64 = 0x40_0000;
    /// Ask the bike to run its target-approach set-up (the interface's slot under this bit) after the
    /// controller. What sets it was not traced; recording2's free09 riders never have it.
    pub const APPROACH: u64 = 0x20_0000;
    /// Run the brain's own extra step after the controller. Not traced; free09's riders never have it.
    pub const EXTRA: u64 = 0x4000;
    /// At the end of the route follower, aim the bike's ground velocity along the route and hand it to
    /// the rider assist ([`assist`]). On for all of free09's riders.
    pub const ASSIST: u64 = 0x800;
}

/// The mode the brain is in. Only [`RIDE`] runs [`update`]'s riding order; every other mode goes to the
/// base AI's update instead.
pub const RIDE: i32 = 5;

/// The interface type that runs [`Climb`]: the motocross bike.
pub const MOTOCROSS: i32 = 1;

/// The stuck rescue: when a ground-going rider (interface types 1, 2, 3 and 6) holds the throttle open
/// below [`STUCK_SPEED`] for [`STUCK_TIME`] seconds, the bike is asked to rescue itself.
pub const STUCK_SPEED: f32 = 10.0;
pub const STUCK_TIME: f32 = 5.0;

/// The climb rule's clock: [`Climb::timer`] seconds nose-up and slow before the throttle is cut, and the
/// [`Climb::hold`] it is cut for.
pub const CLIMB_TIME: f32 = 5.0;

/// The motocross rider's give-up on a climb it cannot make: pointing up steeper than [`Climb::steep`]
/// and slower than [`Climb::speed`], it leans full forward, and after [`CLIMB_TIME`] of that it cuts the
/// throttle for as long again.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Climb {
    /// Seconds left of the cut throttle; 0 or under when it is not cut.
    pub hold: F,
    /// The forward axis's height above which the bike counts as climbing.
    pub steep: F,
    /// How long it has been climbing slowly.
    pub timer: F,
    /// Under this speed a climb counts as slow.
    pub speed: F,
}

/// The brain as [`update`] reads and writes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brain {
    /// The interface's type (1 the motocross bike), or `None` with no interface attached.
    pub kind: Option<i32>,
    /// The brain's mode; [`RIDE`] for a riding AI.
    pub mode: i32,
    /// The flag word ([`flag`], [`tick_flag`]).
    pub flags: u64,
    /// The bike's speed and its forward axis's height, as the rider-to-brain copy left them.
    pub speed: F,
    pub forward_y: F,
    /// The stuck clock.
    pub stuck: F,
    pub climb: Climb,
    /// What the controller hands the bike: the bar, the lean, the throttle and the brake.
    pub steer: F,
    pub pitch: F,
    pub throttle: F,
    pub brake: F,
}

/// What [`update`] calls out to, in its order. Each one is a piece of the AI ported on its own (or a part
/// of the bike's that is not yet); the tick decides only when each runs.
pub trait Steps {
    /// The bike's stuck rescue.
    fn rescue(&mut self, brain: &mut Brain);
    /// The rider-to-brain copy ([`feed`]): the speed, the frame and [`flag::ON_GROUND`] come in here.
    fn feed(&mut self, brain: &mut Brain);
    /// The route follower, which also plans tricks and, under [`tick_flag::ASSIST`], ends in
    /// [`assist_aim`] and [`assist`]. Its answer is ANDed into the tick's.
    fn route(&mut self, brain: &mut Brain, dt: F) -> bool;
    /// The controller: [`controls`] on the ground (the flying types have their own).
    fn controls(&mut self, brain: &mut Brain);
    /// The bike's target-approach set-up, under [`tick_flag::APPROACH`].
    fn approach(&mut self, brain: &mut Brain, dt: F);
    /// The brain's extra step, under [`tick_flag::EXTRA`].
    fn extra(&mut self, brain: &mut Brain, dt: F);
    /// The controls into the bike's levers ([`hand_off`]).
    fn hand_off(&mut self, brain: &mut Brain, dt: F);
    /// The brain's attached listeners. Its answer is ANDed into the tick's.
    fn listeners(&mut self, brain: &mut Brain, dt: F) -> bool;
    /// Any mode but [`RIDE`]: the base AI's update, which is the whole tick then.
    fn other_mode(&mut self, brain: &mut Brain, dt: F) -> bool;
}

/// One AI tick of a motocross brain.
///
/// First the stuck clock, on last tick's throttle and speed: a ground-going interface (types 1, 2, 3
/// and 6), the throttle open and the speed under [`STUCK_SPEED`] add `dt`; at [`STUCK_TIME`] the bike is
/// rescued and the clock starts again; anything else zeroes it.
///
/// Then, riding: last tick's ground bit is kept in [`tick_flag::WAS_ON_GROUND`], the rider-to-brain copy,
/// the route follower, the controller, the two flagged steps, and for the motocross bike the [`Climb`]
/// rule on what the controller asked for; last, the hand-off to the bike's levers and the listeners.
/// The bit copy and the copy itself are skipped with no interface; the hand-off is not (the original
/// would call through a null interface there: a riding brain always has one).
pub fn update(brain: &mut Brain, steps: &mut dyn Steps, dt: F) -> bool {
    let ground_going = matches!(brain.kind, Some(1 | 2 | 3 | 6));
    if !ground_going || op(brain.throttle) <= F(0.0) || !(op(brain.speed) < F(STUCK_SPEED)) {
        brain.stuck = F(0.0);
    } else {
        brain.stuck = brain.stuck + dt;
        if !(op(brain.stuck) < F(STUCK_TIME)) {
            steps.rescue(brain);
            brain.stuck = F(0.0);
        }
    }
    if brain.mode != RIDE {
        return steps.other_mode(brain, dt);
    }

    if brain.kind.is_some() {
        if brain.flags & flag::ON_GROUND != 0 {
            brain.flags |= tick_flag::WAS_ON_GROUND;
        } else {
            brain.flags &= !tick_flag::WAS_ON_GROUND;
        }
        steps.feed(brain);
    }
    let routed = steps.route(brain, dt);
    steps.controls(brain);
    if brain.kind.is_some() && brain.flags & tick_flag::APPROACH != 0 {
        steps.approach(brain, dt);
    }
    if brain.flags & tick_flag::EXTRA != 0 {
        steps.extra(brain, dt);
    }
    if brain.kind == Some(MOTOCROSS) {
        climb(brain, dt);
    }
    steps.hand_off(brain, dt);
    let heard = steps.listeners(brain, dt);
    routed & heard
}

/// A value as a compare reads it: a denormal is 0. The tick's values are copied raw, so they are kept
/// as they came and only their compares go through this.
fn op(x: F) -> F {
    F::from_bits(x.to_bits())
}

/// The [`Climb`] rule, after the controller: while the cut lasts it counts down and the throttle is
/// closed; otherwise a slow climb leans the rider full forward and runs the clock, which past
/// [`CLIMB_TIME`] starts a cut of as long; anything else zeroes the clock.
fn climb(brain: &mut Brain, dt: F) {
    let c = &mut brain.climb;
    if !(op(c.hold) <= F(0.0)) {
        c.hold = c.hold - dt;
        brain.throttle = F(0.0);
    } else if op(brain.forward_y) <= op(c.steep) || !(op(brain.speed) < op(c.speed)) {
        c.timer = F(0.0);
    } else {
        brain.pitch = F(1.0);
        c.timer = c.timer + dt;
        if !(op(c.timer) <= F(CLIMB_TIME)) {
            c.hold = F(CLIMB_TIME);
        }
    }
}

/// The bike's levers the hand-off writes, as the pad read writes them for a human: the stick across and
/// along (the bar and the lean), the gas, the brake and the reverse.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Levers {
    pub steer: F,
    pub lean: F,
    pub gas: F,
    pub brake: F,
    pub reverse: F,
}

/// The bike's own overrides of the AI's controls, which its hit reactions set and its tick counts down:
/// backing up, and swerving one way.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Recovery {
    /// Seconds left backing up.
    pub back_up: F,
    /// Seconds left swerving, and the bar it swerves on (±1).
    pub swerve: F,
    pub swerve_bar: F,
}

/// The AI's controls into its bike's levers, once a tick.
///
/// With no brain, or not `live` (the rider has no race card, or the score keeper's running bit is
/// down), the levers are let go — no bar, lean, gas or reverse, the brake full on — and the answer is
/// false. Otherwise the bar is the brain's (or the swerve's while it lasts) and the lean the brain's;
/// on the ground the gas and the brake are the brain's, or while backing up the reverse alone; in the
/// air the brake is let off and the gas and the reverse stay as they were.
pub fn hand_off(brain: Option<&Brain>, live: bool, recovery: &Recovery, levers: &mut Levers) -> bool {
    let Some(brain) = brain.filter(|_| live) else {
        *levers = Levers { brake: F(1.0), ..Levers::default() };
        return false;
    };
    levers.steer = if op(recovery.swerve) <= F(0.0) { brain.steer } else { recovery.swerve_bar };
    levers.lean = brain.pitch;
    if brain.flags & flag::ON_GROUND == 0 {
        levers.brake = F(0.0);
    } else if op(recovery.back_up) <= F(0.0) {
        levers.reverse = F(0.0);
        levers.gas = brain.throttle;
        levers.brake = brain.brake;
    } else {
        levers.reverse = F(1.0);
        levers.gas = F(0.0);
        levers.brake = F(0.0);
    }
    true
}

/// The route follower's last step: under [`tick_flag::ASSIST`], the ground velocity the bike is aimed
/// at, `x` and `z` — the route's direction times its target speed — which [`assist`] then pulls toward.
pub fn assist_aim(flags: u64, direction: Vf, target_speed: F) -> Option<(F, F)> {
    (flags & tick_flag::ASSIST != 0).then(|| (F(direction.x()) * target_speed, F(direction.z()) * target_speed))
}

/// The rider's two assist weights (the character's), each a blend toward the brain's wish per tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Weights {
    /// How much of the brain's heading angle the body's yaw rate takes.
    pub heading: F,
    /// How much of the brain's aim the body's ground velocity takes.
    pub aim: F,
}

/// The rider assist, the route follower's last call under [`tick_flag::ASSIST`]: the body's yaw rate is
/// pulled toward the brain's heading angle (the controller's from the tick before), and on the ground the
/// body's velocity `x` and `z` toward the [`assist_aim`] — `wish·w + own·(1 − w)` on the accumulator.
/// With both weights 0, as on free09, it changes nothing (bar a −0 yaw rate, which comes out +0).
pub fn assist(w: &Weights, heading: F, aim: (F, F), on_ground: bool, yaw_rate: &mut F, velocity: &mut Vf) {
    let keep_aim = F(1.0) - w.aim;
    *yaw_rate = Acc::mula(heading, w.heading).madd(*yaw_rate, F(1.0) - w.heading);
    if on_ground {
        let x = Acc::mula(aim.0, w.aim).madd(velocity.x(), keep_aim);
        let z = Acc::mula(aim.1, w.aim).madd(velocity.z(), keep_aim);
        *velocity = Vf::new(x.to_f32(), velocity.y(), z.to_f32(), velocity.w());
    }
}

// --- the free-ride target seeker (S282a) -------------------------------------------------------------

/// How an AI rider with no path to follow gets round a free-ride map (S282a): it picks a jump, rides
/// to the jump's setup point, lines up with the take-off, launches, lands, and learns from where it
/// came down how to take the jump next time.
///
/// The map's jumps come in sets ([`Set`]); a rider holds one set and one target out of it. A target
/// is a [`Jump`]: three [`Leg`]s, the setup point the run-in starts from, the take-off lip and the
/// landing, each with a point, a facing and — on the take-off — the velocity and aim point the rider
/// has learnt for it. The rider goes through three states: 1 riding to the setup point, 2 on the
/// run-in to the lip (once it is past the setup point, on the ground and facing the jump), 3 in the
/// air and landing (within 20 ft of the lip).
///
/// On landing it measures the miss from the landing point and nudges the take-off: the launch
/// direction turns a step towards the side the landing was missed on, the launch speed and the
/// learnt count go up or down a step with the miss along the jump, and every step halves when the
/// miss changes side. A learnt count past 100 re-aims the lip at random within 10 ft. Close enough a
/// landing, too many tries, or a target done drops the target and the next tick picks another.
///
/// Every state ends in the same hand-off ([`hand_off`]): a point and a direction on the ground, a
/// speed, and how far ahead of the point to steer ([`reach`]), which set the brain's aim point,
/// heading and target speed — what [`super::controls`] steers and throttles by.
///
/// The original keeps the targets three ways, and the port has each: a set's own pick list (one drawn
/// at random, up to 100 tries), a course ([`Course`]: the nearest target not done yet), and a fixed
/// sequence when the game has one ([`World::sequence`]). What builds a set's pick list when it has
/// none is not ported: [`Set::picks`] `None` is a set whose list is not built, and picks nothing.
///
/// Where the original reads memory it does not own — an empty list's first entry, a course's entry
/// before the first when nothing is nearest, a rider with no target in a state that needs one — the
/// port reads "no target", which is what the original reads when that memory is zero.
pub mod seek {
    use super::flag::ON_GROUND;
    use super::{Controls, add, scale, sub, unit};
    use crate::rng::Rng;
    use ps2_float::fpu::Acc;
    use ps2_float::{Dest, F, Vf, vu};

    /// The first flag word's bit for "past the point, closing on it from behind": [`hand_off`] keeps
    /// it, and the take-off sets it for the flight.
    pub const CLOSING: u64 = 0x0800_0000;
    /// The first flag word's bit that keeps a landing from being learnt from (not traced who sets it).
    pub const NO_LEARN: u64 = 0x40_0000;
    /// The second flag word's bit for "ride this jump flat out, do not learn": set or cleared by the
    /// course's chance roll.
    pub const HEADLONG: u64 = 0x1000;
    /// The second flag word's bit set when a leg is finished: on the take-off and after a landing.
    pub const LEG_DONE: u64 = 0x8;
    /// The miss a fresh target starts with: `y` 1 says "no landing yet", so nothing halves.
    pub const MISS_START: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);

    /// The fastest the seeker asks for, and the speed a leg with no learnt velocity is ridden at.
    const FLAT_OUT: f32 = 150.0;

    /// One part of a jump.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Leg {
        /// The velocity to ride it at (learnt on the take-off; 0 asks for [`FLAT_OUT`] along the jump).
        pub velocity: Vf,
        /// Where to aim for on it (the take-off's is learnt and re-aimed).
        pub aim: Vf,
        /// Its point.
        pub point: Vf,
        /// Which way it faces: the landing is crossed when the rider passes it this way.
        pub facing: Vf,
        /// The learnt count, 0..100.
        pub learnt: F,
    }

    /// A target's jump: the original's record of three legs, 0x60 bytes apart.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Jump {
        pub take_off: Leg,
        pub landing: Leg,
        pub setup: Leg,
    }

    /// A target: its jump, and per player whether it is done.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Target {
        pub jump: Option<usize>,
        pub done: Vec<bool>,
    }

    /// A course's targets, ridden nearest first, and the chance a target is learnt from rather than
    /// ridden flat out.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Course {
        pub targets: Vec<Option<usize>>,
        pub chance: F,
    }

    /// A set of targets: its pick list (`None` until built), and its course if it has one.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Set {
        pub picks: Option<Vec<Option<usize>>>,
        pub course: Option<Course>,
    }

    /// Everything the seeker looks up: the sets, targets and jumps by index, and the game's fixed
    /// sequence of targets when it has one (which also changes how many tries a jump gets and how big
    /// its learning steps start).
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct World {
        pub sets: Vec<Set>,
        pub targets: Vec<Target>,
        pub jumps: Vec<Jump>,
        pub sequence: Option<Vec<Option<usize>>>,
    }

    /// The seeker's part of the brain.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Seeker {
        /// The brain's two flag words ([`CLOSING`], [`NO_LEARN`], [`ON_GROUND`]; [`HEADLONG`],
        /// [`LEG_DONE`]).
        pub flags: u64,
        pub flags2: u64,
        /// The set it picks from, the target it rides and the one it rode before, the target's jump.
        pub set: Option<usize>,
        pub target: Option<usize>,
        pub last_target: Option<usize>,
        pub jump: Option<usize>,
        /// Where the course scan or the sequence is.
        pub index: i32,
        /// 0 none, 1 to the setup point, 2 the run-in, 3 the flight.
        pub state: i32,
        /// Landings on this target (counted elsewhere; the seeker reads and zeroes it).
        pub tries: i32,
        /// How near the landing point counts as landed, and as crossing it.
        pub tolerance: F,
        /// The learnt count it hands on, and its three learning steps: the launch direction's turn,
        /// the launch speed's and the learnt count's.
        pub learnt: F,
        pub aim_step: F,
        pub speed_step: F,
        pub learn_step: F,
        /// The miss before, and this landing's (`y` 0 once landed).
        pub last_miss: Vf,
        pub miss: Vf,
        /// How far ahead of the point the rider steers.
        pub lead: F,
        /// How far the rider is from the point it rides to.
        pub distance: F,
        /// The path follower's word, zeroed on a new target.
        pub follow: u32,
        /// What [`hand_off`] writes: the point steered at, the unit heading to it, the target speed.
        pub aim: Vf,
        pub heading: Vf,
        pub speed: F,
    }

    /// The steer-ahead [`reach`] works out: `base + gain × off-line distance`, at most `max`.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Reach {
        pub base: F,
        pub max: F,
        pub gain: F,
    }

    /// The rider's own bike, as the seeker reads it.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Mount {
        /// Its flag word: without 2 (in a race) or 0x100 (in a race's phase 4) the AI does not ride.
        pub flags: u64,
        /// Above 0.5 a landing is learnt from (what the value is is not traced).
        pub settle: F,
    }

    /// What the seeker reads off the rider, its interface and the race.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Ride {
        pub position: Vf,
        pub last_position: Vf,
        pub forward: Vf,
        pub speed: F,
        /// The slowest target speed [`hand_off`] and the learning hand on.
        pub min_speed: F,
        /// The interface's scale on a learnt speed, and the radius of the setup point inside which
        /// the speed is `slow_speed`.
        pub speed_scale: F,
        pub slow_radius: F,
        pub slow_speed: F,
        /// The rider's player number, which a target's done flags are kept by.
        pub player: usize,
        pub reach: Reach,
        /// The race's phase when there is a race.
        pub race_phase: Option<i32>,
        pub bike: Option<Mount>,
        /// The race has an AI path net: a rider with no target follows it (S282b).
        pub path_net: bool,
    }

    /// How a tick of [`seek`] ended.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Seek {
        /// The race holds the rider: the four controls are zeroed and nothing else is done.
        Still,
        /// No target or no jump: the original returns 0 here.
        Lost,
        /// No state, and a path net to follow: the path follower takes over (S282b, not ported).
        Path,
        /// The heading and speed are written; the original goes on to its tail (S282b).
        Steered,
    }

    #[derive(Clone, Copy)]
    enum Part {
        TakeOff,
        Landing,
        Setup,
    }

    fn part(j: &Jump, p: Part) -> &Leg {
        match p {
            Part::TakeOff => &j.take_off,
            Part::Landing => &j.landing,
            Part::Setup => &j.setup,
        }
    }

    /// `y` stored as 0.
    fn flat(v: Vf) -> Vf {
        Vf::new(v.x(), 0.0, v.z(), v.w())
    }

    /// `a.x·b.x + a.y·b.y + a.z·b.z` on the FPU's accumulator.
    fn dot(a: Vf, b: Vf) -> F {
        Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
    }

    fn length(v: Vf) -> F {
        dot(v, v).sqrt()
    }

    /// The steer-ahead for a point and a direction (the original's vt+0x80): the rider's distance off
    /// the line through `point` along `dir`, times `gain`, on `base`, and at most `max`.
    pub fn reach(position: Vf, point: Vf, dir: Vf, r: &Reach) -> F {
        let d = sub(point, position);
        let off = add(scale(dir, dot(d, dir)), d);
        let out = r.base + length(off) * r.gain;
        if out <= r.max { out } else { r.max }
    }

    /// The hand-off (the original's vt+0x94): `point` and the unit `dir` on the ground, the target
    /// `speed`, the rider's `distance` from the point, the `lead` and the [`reach`].
    ///
    /// Behind the point (along `dir`) clears [`CLOSING`]; at or past `lead` short of it sets it. The aim
    /// point is `lead` back from the point along `dir`, or with [`CLOSING`] the rider's own distance
    /// along — at most `lead + reach` — less the reach. The heading is the flat unit way to the aim
    /// point. Nearer than `lead` the speed comes down towards the slowest as the heading turns off
    /// the rider's forward.
    #[allow(clippy::too_many_arguments)]
    pub fn hand_off(s: &mut Seeker, ride: &Ride, point: Vf, dir: Vf, speed: F, distance: F, lead: F, reach: F) {
        let along = dot(flat(sub(point, ride.position)), dir);
        if along < F(0.0) {
            s.flags &= !CLOSING;
        } else if !(along < lead) {
            s.flags |= CLOSING;
        }
        let most = lead + reach;
        let along = if along <= most { along } else { most };
        let back = if s.flags & CLOSING != 0 { along - reach } else { lead };
        s.aim = sub(point, scale(dir, back));
        s.heading = unit(flat(sub(s.aim, ride.position)));
        s.speed = if distance < lead {
            ride.min_speed + (speed - ride.min_speed) * dot(s.heading, ride.forward).abs()
        } else {
            speed
        };
    }

    /// Whether the rider crossed `leg`'s point this tick (the original's crossing test): it
    /// is past the point along the leg's facing now and was not before, and where its step crossed
    /// the point's line is within `tolerance` of the point on the ground.
    pub fn crossed(position: Vf, last_position: Vf, tolerance: F, leg: &Leg) -> bool {
        let a = sub(leg.point, position);
        let now = dot(a, leg.facing);
        let then = dot(sub(leg.point, last_position), leg.facing);
        if !(now < F(0.0)) || then < F(0.0) {
            return false;
        }
        let step = sub(position, last_position);
        let moved = dot(step, leg.facing);
        if moved <= F(0.0) {
            return false;
        }
        length(flat(sub(scale(step, now / moved), a))) <= tolerance
    }

    /// One draw off a set's pick list: `None`, without a draw, when the list is not built.
    fn pick(set: &Set, rng: &mut Rng) -> Option<usize> {
        let list = set.picks.as_ref()?;
        let i = rng.index(list.len() as i32);
        usize::try_from(i).ok().and_then(|i| list.get(i).copied().flatten())
    }

    /// The course's chance roll: at or under the chance learns, over it rides flat out.
    fn roll(s: &mut Seeker, chance: F, rng: &mut Rng) {
        if rng.unit() <= chance {
            s.flags2 &= !HEADLONG;
        } else {
            s.flags2 |= HEADLONG;
        }
    }

    /// One tick of the seeker (the free-ride part of the original's ai_tick_flag).
    ///
    /// A race holding the rider zeroes `out`'s steer, pitch, throttle and brake and stops there.
    /// Otherwise a rider with no target, or a done one on a course, picks one — from the sequence, the
    /// course or the set — takes its jump, and starts it over (state 0, a fresh [`MISS_START`]); a
    /// take-off with no velocity gets [`FLAT_OUT`] along the jump. Then state 0 becomes 1 (unless a
    /// path net takes the rider: [`Seek::Path`]), the state moves on as the rider gets there, a
    /// landing is learnt from, and the state's point, direction and speed go through [`reach`] and
    /// [`hand_off`]. With no state left the heading is the rider's forward and the speed its own.
    pub fn seek(s: &mut Seeker, ride: &Ride, world: &mut World, rng: &mut Rng, out: &mut Controls) -> Seek {
        if let Some(phase) = ride.race_phase {
            let held = phase == 4 && ride.bike.map_or(0, |b| b.flags) & 0x100 == 0
                || ride.bike.is_some_and(|b| b.flags & 2 == 0);
            if held {
                out.steer = F(0.0);
                out.pitch = F(0.0);
                out.throttle = F(0.0);
                out.brake = F(0.0);
                return Seek::Still;
            }
        }

        let sequenced = world.sequence.is_some();
        let player = ride.player;
        let done = |w: &World, t: usize| w.targets[t].done.get(player).copied().unwrap_or(false);
        let on_course = s.set.is_some_and(|i| world.sets[i].course.is_some());
        let retarget = match s.target {
            None => true,
            Some(t) => on_course && done(world, t),
        };
        if retarget {
            if s.target.is_some() {
                s.last_target = s.target;
            }
            if let Some(si) = s.set {
                if let Some(seq) = &world.sequence {
                    let mut tries = 0;
                    while s.target.is_none() && tries < 100 {
                        s.target = usize::try_from(s.index).ok().and_then(|i| seq.get(i).copied().flatten());
                        s.index = s.index.wrapping_add(1);
                        if s.index >= seq.len() as i32 {
                            s.index = 0;
                        }
                        tries += 1;
                    }
                    if let Some(j) = s.target.and_then(|t| world.targets[t].jump) {
                        let jump = &mut world.jumps[j];
                        jump.take_off.aim = jump.take_off.point;
                    }
                } else if let Some(course) = &world.sets[si].course {
                    s.index = 0;
                    let (mut best, mut nearest) = (-1, F(0.0));
                    while s.index < course.targets.len() as i32 {
                        s.target = course.targets[s.index as usize];
                        if let Some(t) = s.target.filter(|&t| !done(world, t)) {
                            if let Some(j) = world.targets[t].jump {
                                let d = length(flat(sub(world.jumps[j].setup.point, ride.position)));
                                if best == -1 || d < nearest {
                                    best = s.index;
                                    nearest = d;
                                }
                            }
                        }
                        s.index += 1;
                    }
                    s.target = usize::try_from(best).ok().and_then(|b| course.targets[b]);
                    let chance = course.chance;
                    roll(s, chance, rng);
                } else {
                    let mut tries = 0;
                    while s.target.is_none() && tries < 100 {
                        s.target = pick(&world.sets[si], rng);
                        tries += 1;
                    }
                }
                let Some(t) = s.target else { return Seek::Lost };
                s.jump = world.targets[t].jump;
                let Some(j) = s.jump else { return Seek::Lost };
                let jump = &mut world.jumps[j];
                if length(jump.take_off.velocity) <= F(0.0) {
                    let along = unit(flat(sub(jump.landing.point, jump.take_off.point)));
                    jump.take_off.velocity = scale(along, F(FLAT_OUT));
                    jump.take_off.aim = jump.take_off.point;
                }
                s.tries = 0;
            }
            s.follow = 0;
            if !sequenced {
                s.state = 0;
            }
            s.last_miss = MISS_START;
        }

        if let Some(j) = s.jump {
            let lip = &mut world.jumps[j].take_off;
            if F(lip.aim.x()) == F(0.0) || F(lip.aim.z()) == F(0.0) {
                lip.aim = lip.point;
            }
        }
        if s.state == 0 && ride.path_net {
            return Seek::Path;
        }
        let Some(j) = s.jump else { return Seek::Lost };
        if s.state == 0 {
            s.state = 1;
            s.flags &= !CLOSING;
            s.lead = F(20.0);
        }

        // the leg whose velocity sets the speed: none for a state the original has no leg for
        let mut leg = None;
        match s.state {
            1 => {
                let jump = &world.jumps[j];
                leg = Some(Part::Setup);
                let short = flat(sub(jump.setup.point, ride.position));
                let line = unit(flat(sub(jump.take_off.point, jump.setup.point)));
                let along = dot(short, line);
                if along < F(0.0)
                    && !(along <= F(-20.0))
                    && !(dot(ride.forward, line) < F(0.0))
                    && s.flags & ON_GROUND != 0
                {
                    s.state = 2;
                    s.lead = F(60.0);
                    leg = Some(Part::TakeOff);
                    s.flags2 |= LEG_DONE;
                }
            }
            2 => {
                leg = Some(Part::TakeOff);
                let to_lip = flat(sub(world.jumps[j].take_off.point, ride.position));
                if length(to_lip) < F(20.0) {
                    s.state = 3;
                    s.lead = F(0.0);
                    s.flags |= CLOSING;
                    leg = Some(Part::Landing);
                }
            }
            3 => {
                s.last_target = s.target;
                leg = Some(Part::Landing);
                let learns = s.flags & NO_LEARN == 0
                    && s.flags & ON_GROUND != 0
                    && ride.bike.is_some_and(|b| !(b.settle <= F(0.5)));
                if learns {
                    leg = land(s, ride, world, j, rng);
                } else if s.flags & ON_GROUND != 0
                    && crossed(ride.position, ride.last_position, s.tolerance, &world.jumps[j].landing)
                {
                    s.state = 1;
                    s.flags &= !CLOSING;
                    s.lead = F(20.0);
                    leg = Some(Part::Setup);
                }
            }
            _ => {}
        }

        if s.state == 0 {
            s.heading = ride.forward;
            s.speed = ride.speed;
            return Seek::Steered;
        }
        let jump = &world.jumps[j];
        let velocity = leg.map_or(Vf::new(0.0, 0.0, 0.0, 0.0), |p| part(jump, p).velocity);
        let mut speed = ride.speed_scale * length(velocity);
        let (lip, landing, setup) = (&jump.take_off, &jump.landing, &jump.setup);
        let (point, dir);
        if speed <= F(0.0) || s.flags2 & HEADLONG != 0 && !sequenced {
            speed = F(FLAT_OUT);
            match s.state {
                2 => {
                    (point, dir) = (lip.aim, sub(landing.point, lip.point));
                    s.learnt = F(100.0);
                }
                3 => {
                    let from = if s.flags & ON_GROUND == 0 { ride.position } else { lip.aim };
                    (point, dir) = (landing.point, sub(landing.point, from));
                    s.learnt = F(100.0);
                }
                _ => {
                    (point, dir) = (setup.point, sub(lip.point, setup.point));
                    s.learnt = F(0.0);
                }
            }
        } else {
            match s.state {
                2 => {
                    (point, dir) = (lip.aim, lip.velocity);
                    s.learnt = lip.learnt;
                }
                3 => {
                    (point, dir) = (landing.point, sub(landing.point, ride.position));
                    s.learnt = landing.learnt;
                }
                _ => {
                    (point, dir) = (setup.point, sub(lip.aim, setup.point));
                    s.learnt = F(0.0);
                }
            }
        }
        let dir = unit(flat(dir));
        s.distance = length(flat(sub(ride.position, point)));
        if s.state == 1 && s.distance <= ride.slow_radius {
            speed = ride.slow_speed;
        }
        let r = reach(ride.position, point, dir, &ride.reach);
        let (distance, lead) = (s.distance, s.lead);
        hand_off(s, ride, point, dir, speed, distance, lead, r);
        Seek::Steered
    }

    /// A landing on the ground with the bike settled: measure the miss, then either drop the target
    /// (near enough, out of tries, or done) or learn from it and go back to state 1. Gives the leg
    /// the speed is read off.
    fn land(s: &mut Seeker, ride: &Ride, world: &mut World, j: usize, rng: &mut Rng) -> Option<Part> {
        let sequenced = world.sequence.is_some();
        let course = s.set.and_then(|i| world.sets[i].course.as_ref()).map(|c| c.chance);
        let m = sub(world.jumps[j].landing.point, ride.position);
        s.miss = flat(m);
        let missed_by = length(s.miss);
        let done = s.target.is_some_and(|t| world.targets[t].done.get(ride.player).copied().unwrap_or(false));
        let out_of_tries = if sequenced { s.tries >= 10 } else { s.tries >= 3 };
        if missed_by < s.tolerance || out_of_tries || course.is_some() && done {
            s.target = None;
            s.follow = 0;
            s.state = 0;
            s.last_miss = MISS_START;
            if sequenced {
                s.speed_step = F(40.0);
                s.learn_step = F(40.0);
                s.aim_step = F::from_bits(0x3e4c_cccd); // 0.2
            } else {
                s.speed_step = F(4.0);
                s.learn_step = F(4.0);
                s.aim_step = F::from_bits(0x3ca3_d70a); // 0.02
            }
            return Some(Part::Landing);
        }
        match course {
            Some(chance) => roll(s, chance, rng),
            None => s.flags2 &= !HEADLONG,
        }
        if s.flags2 & HEADLONG == 0 || sequenced {
            learn(s, ride, &mut world.jumps[j], rng);
        }
        s.state = 1;
        s.flags &= !CLOSING;
        s.lead = F(20.0);
        s.flags2 |= LEG_DONE;
        Some(Part::Setup)
    }

    /// The take-off learnt from this landing's miss.
    fn learn(s: &mut Seeker, ride: &Ride, jump: &mut Jump, rng: &mut Rng) {
        let v = jump.take_off.velocity;
        let h = unit(flat(v));
        let mut speed = length(v);
        s.learnt = jump.take_off.learnt;
        let (mx, mz) = (s.miss.x(), s.miss.z());
        let (lx, lz) = (s.last_miss.x(), s.last_miss.z());
        let (hx, hz) = (F(h.x()), F(h.z()));
        let along = Acc::mula(mx, hx).madd(mz, hz);
        let across = Acc::mula(mx, hz).msub(mz, hx);
        let last_along = Acc::mula(lx, hx).madd(lz, hz);
        let last_across = Acc::mula(lx, hz).msub(lz, hx);

        // the jump's own line, lip aim to landing, on the ground
        let mut line = flat(sub(jump.landing.point, jump.take_off.aim));
        let span = length(line);
        if !(span <= F(0.0)) {
            line = line.merge(Dest::XYZ, line.mul_q(vu::div(1.0, span.to_f32())));
        }

        // turn the launch a step towards the side the landing was missed on
        let step = s.aim_step;
        let (nx, nz) = if !(across <= F(0.0)) {
            let nx = hx + step * hz;
            (nx, hz - step * nx)
        } else {
            let nx = hx - step * hz;
            (nx, hz + step * nx)
        };
        let mut h = unit(Vf::new(nx.0, h.y(), nz.0, h.w()));
        // at most 45° off the jump's line
        let cos45 = F::from_bits(0x3f35_04f3);
        let c = Acc::mula(h.x(), line.x()).madd(h.z(), line.z());
        if c <= cos45 {
            h = scale(add(unit(sub(h, scale(line, c))), line), cos45);
        }

        if F(s.last_miss.y()) <= F(0.0) {
            if last_along * along < F(0.0) {
                s.speed_step = s.speed_step * F(0.5);
                s.learn_step = s.learn_step * F(0.5);
            }
            if last_across * across < F(0.0) {
                s.aim_step = s.aim_step * F(0.5);
            }
        }
        let long = !(along <= F(0.0));
        speed = if long { speed + s.speed_step } else { speed - s.speed_step };
        if !(speed <= F::from_bits(0x4315_e666)) {
            // 149.9
            speed = F(FLAT_OUT);
        } else if speed < ride.min_speed {
            speed = ride.min_speed;
        }
        s.learnt = if long { s.learnt + s.learn_step } else { s.learnt - s.learn_step };
        let lip = jump.take_off.point;
        let aim = &mut jump.take_off.aim;
        if !(s.learnt <= F(100.0)) {
            s.learnt = F(100.0);
            let x = F(lip.x()) + F(20.0) * rng.unit() - F(10.0);
            *aim = Vf::new(x.0, aim.y(), aim.z(), aim.w());
            let z = F(lip.z()) + F(20.0) * rng.unit() - F(10.0);
            *aim = Vf::new(aim.x(), aim.y(), z.0, aim.w());
        } else if s.learnt < F(0.0) {
            *aim = Vf::new(lip.x(), aim.y(), lip.z(), aim.w());
            s.learnt = F(0.0);
        }
        s.last_miss = s.miss;
        jump.take_off.learnt = s.learnt;
        jump.take_off.velocity = scale(h, speed);
    }
}

// --- the race path follower's pieces and the tick's tail (S282b) -----------------------------------

/// What the motocross AI's tick does after the seeker or the path follower has set the heading and
/// target speed, and the track query the race path follower places a rider with.
///
/// The race path follower itself (the original's ai_tick_flag with no seeker state and a path net:
/// the path's look-ahead, the route follower's wobble and drift, the hand-over to the seeker when
/// the path runs out) is not ported yet; see S282b's journal.
pub mod follow {
    use super::{add, scale, sub, unit};
    use crate::race::{Spot, TrackPaths, path_flags, ring_holds};
    use ps2_float::fpu::Acc;
    use ps2_float::{F, Vf, fpu, vu};

    /// The brain's first flag word: run the avoidance on the heading (the 45° cone first).
    pub const AVOIDS: u64 = 0x1;
    /// The brain's first flag word: [`super::flag::NO_RIDER_SWEEP`] follows the bike's own word
    /// [`Tail::bike_word`] each tick (up while it is non-zero).
    pub const SWEEP_BY_BIKE: u64 = 0x1000;
    /// The brain's first flag word: hand the heading times the speed to the rider assist each tick.
    pub const ASSIST: u64 = 0x800;

    /// `cos 45°`, the turn the avoidance gets the heading within.
    const CONE: f32 = 0.70710677;

    /// The nearest place on the track's paths to `point` across the ground (the original's path spot
    /// at a point).
    ///
    /// The paths are walked depth first from path 0 along their links, one route-pool entry per path
    /// on the walk's stack (a linked path already opened, or kept off, is not stacked; path 0 is,
    /// whatever its flags). On each path a lone first node counts as a place by itself, `along` 0;
    /// every stretch counts by the nearest point of its first-point line, clamped to its ends — with
    /// `holding_only`, only a stretch whose ring holds the point ([`ring_holds`]). The nearer one
    /// wins, a tie keeps the first found; `out` gets it and stays as it was when there is none.
    /// `distance` gets the ground distance, 0 when exactly on the line, and stays as it was when
    /// nothing was found.
    ///
    /// `false` with no paths or an empty pool at the start, `out` untouched. When the pool runs dry
    /// halfway the answer is `false` too, as the original's, and the walk's entries and its
    /// [`path_flags::OPENED`] marks stay where they are, `out` holding what was found so far.
    pub fn spot(t: &mut TrackPaths, point: Vf, holding_only: bool, out: &mut Spot, distance: Option<&mut F>) -> bool {
        if t.paths.is_empty() || t.free_routes == 0 {
            return false;
        }
        let (px, pz) = (F(point.x()), F(point.z()));
        let mut best = F(-1.0);
        t.free_routes -= 1;
        let mut stack = vec![0];
        while let Some(&p) = stack.last() {
            if t.paths[p].flags & path_flags::OPENED != 0 {
                stack.pop();
                t.free_routes += 1;
                continue;
            }
            t.paths[p].flags |= path_flags::OPENED;
            let nodes = &t.paths[p].nodes;
            if nodes.len() == 1 {
                let c = nodes[0].points[0];
                let (dx, dz) = (px - F(c.x()), pz - F(c.z()));
                let d2 = Acc::mula(dx, dx).madd(dz, dz);
                if d2 < best || best < F(0.0) {
                    *out = Spot::on(p, 0, F(0.0));
                    best = d2;
                }
            }
            for i in 0..nodes.len().saturating_sub(1) {
                let (a, b) = (&nodes[i], &nodes[i + 1]);
                if holding_only && !ring_holds(point, a, b) {
                    continue;
                }
                let (cx, cz) = (F(a.points[0].x()), F(a.points[0].z()));
                let (nx, nz) = (F(b.points[0].x()), F(b.points[0].z()));
                let (dx, dz) = (nx - cx, nz - cz);
                let mut t_along = F(0.0);
                if dx != F(0.0) || dz != F(0.0) {
                    let num = Acc::mula(dx, px - cx).madd(dz, pz - cz);
                    t_along = num / Acc::mula(dx, dx).madd(dz, dz);
                }
                if !(t_along <= F(1.0)) {
                    t_along = F(1.0);
                } else if t_along < F(0.0) {
                    t_along = F(0.0);
                }
                let d2 = if t_along == F(1.0) {
                    let (ox, oz) = (px - nx, pz - nz);
                    Acc::mula(ox, ox).madd(oz, oz)
                } else if t_along == F(0.0) {
                    let (ox, oz) = (px - cx, pz - cz);
                    Acc::mula(ox, ox).madd(oz, oz)
                } else {
                    let oz = Acc::suba(pz, cz).msub(dz, t_along);
                    let ox = Acc::suba(px, cx).msub(dx, t_along);
                    Acc::mula(ox, ox).madd(oz, oz)
                };
                if d2 < best || best < F(0.0) {
                    *out = Spot::on(p, i, t_along);
                    best = d2;
                }
            }
            for k in 0..t.paths[p].links.len() {
                let l = t.paths[p].links[k];
                if t.paths[l].flags & (path_flags::OPENED | path_flags::KEEP_OFF) != 0 {
                    continue;
                }
                if t.free_routes == 0 {
                    return false;
                }
                t.free_routes -= 1;
                stack.push(l);
            }
        }
        for path in &mut t.paths {
            path.flags &= !path_flags::OPENED;
        }
        if let Some(d) = distance {
            if best > F(0.0) {
                *d = best.sqrt();
            } else if best == F(0.0) {
                *d = F(0.0);
            }
        }
        true
    }

    /// The brain's fields the tail reads and writes.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Brain {
        /// The first flag word ([`AVOIDS`], [`SWEEP_BY_BIKE`], [`ASSIST`],
        /// [`super::flag::NO_RIDER_SWEEP`], [`super::flag::ON_GROUND`]).
        pub flags: u64,
        /// The heading and target speed the seeker or the path follower set.
        pub heading: Vf,
        pub speed: F,
        /// What [`ASSIST`] hands the rider assist: the heading's `x` and `z` times the speed.
        pub assist_x: F,
        pub assist_z: F,
    }

    /// What the tail reads off the rider and its interface.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Tail {
        pub forward: Vf,
        pub velocity: Vf,
        /// The rider's own speed (the velocity's length, as the brain's copy has it).
        pub own_speed: F,
        /// The interface's cosine of the widest an airborne rider's heading may be off its velocity.
        pub air_cone: F,
        /// The bike's word [`SWEEP_BY_BIKE`] reads, `None` with no bike.
        pub bike_word: Option<u32>,
    }

    /// The tick's tail (the original's ai_tick_flag from where the seeker's hand-off joins it).
    ///
    /// 1. With [`SWEEP_BY_BIKE`] and a bike, [`super::flag::NO_RIDER_SWEEP`] goes up when the bike's
    ///    word is non-zero and down when it is 0.
    /// 2. With [`AVOIDS`]: a heading more than 45° off the forward (by `x` and `z` only) is turned to
    ///    45° off it — the part across the forward made unit (`y` too) and the sum with the forward
    ///    scaled by `cos 45°` in `x` and `z` — and then `avoid` runs (the original's vt+0x74, the
    ///    avoidance, which bends the heading and the speed).
    /// 3. When the rider moves and is not on the ground, a heading more than the interface's cone off
    ///    the velocity's direction is put on the cone's edge toward it; its `w` becomes the velocity's.
    /// 4. `plan` runs (vt+0x84, the trick plan).
    /// 5. With [`ASSIST`]: the heading's `x`/`z` times the speed go to the brain's assist pair and
    ///    `assist` runs (the interface's vt+0x18, the rider assist).
    pub fn tail(
        b: &mut Brain, ride: &Tail, avoid: &mut dyn FnMut(&mut Brain), plan: &mut dyn FnMut(&mut Brain),
        assist: &mut dyn FnMut(&mut Brain),
    ) {
        if b.flags & SWEEP_BY_BIKE != 0 {
            if let Some(word) = ride.bike_word {
                if word != 0 {
                    b.flags |= super::flag::NO_RIDER_SWEEP;
                } else {
                    b.flags &= !super::flag::NO_RIDER_SWEEP;
                }
            }
        }
        if b.flags & AVOIDS != 0 {
            let (f, h) = (ride.forward, b.heading);
            let d = Acc::mula(h.z(), f.z()).madd(h.x(), f.x());
            if d <= F(CONE) {
                let x = F(h.x()) - F(f.x()) * d;
                let z = F(h.z()) - F(f.z()) * d;
                let h = unit(Vf::new(x.0, h.y(), z.0, h.w()));
                let x = F(CONE) * (F(h.x()) + F(f.x()));
                let z = F(CONE) * (F(h.z()) + F(f.z()));
                b.heading = Vf::new(x.0, h.y(), z.0, h.w());
            }
            avoid(b);
        }
        if F(0.0) < ride.own_speed {
            let vel = ride.velocity;
            let v = vel.merge(ps2_float::Dest::XYZ, vel.mul_q(vu::div(1.0, ride.own_speed.0)));
            let h = b.heading;
            let d = Acc::mula(v.x(), h.x()).madda(v.y(), h.y()).madd(v.z(), h.z());
            let cone = ride.air_cone;
            if b.flags & super::flag::ON_GROUND == 0 && d < cone {
                let across = unit(sub(h, scale(v, d)));
                let across = scale(across, (F(1.0) - cone * cone).sqrt());
                b.heading = add(scale(v, cone), across);
            }
        }
        plan(b);
        if b.flags & ASSIST != 0 {
            b.assist_x = F(b.heading.x()) * b.speed;
            b.assist_z = F(b.heading.z()) * b.speed;
            assist(b);
        }
    }

    /// A float word as the FPU's compares read it ([`fpu::operand`]): a denormal is 0, Inf/NaN the
    /// saturated maximum.
    fn seen(x: F) -> F {
        F(fpu::operand(x.0.to_bits()))
    }

    /// The nearest place to `point` on one path, `path`, across the ground (the original's path locate,
    /// with a start hint): `out` gets the path, and the stretch and `along` of the nearest stretch's
    /// clamped first-point line, as in [`spot`]; `distance` the ground distance to it.
    ///
    /// Without a `hint` the walk starts at the path's first node. A hint on this path starts it at the
    /// hint's node, and when the path links to itself the walk goes on from the first node once it
    /// reaches the last; a hint on another path starts at the first node, the walked length starting
    /// at the way along the paths from the hint ([`TrackPaths::distance`], which runs the track's
    /// route search when the hint is not behind on one path). With `limit` not below 0 the walk stops
    /// once the length walked, counted from the hint, reaches it. With `holding_only` only a stretch
    /// whose ring holds the point counts ([`ring_holds`]).
    ///
    /// A start node with no next one (a lone node, or a hint on the last) is the place itself, `along`
    /// 0. When no stretch is taken `out` keeps its stretch and `along`, and `distance` gets −1; the
    /// distance is the squared one when that is not above 0.
    ///
    /// `false`, nothing written, when `refuse` and the path is [`path_flags::KEEP_OFF`]. A hint on
    /// another path has to name a path and a node.
    #[allow(clippy::too_many_arguments)]
    pub fn locate_on(
        t: &mut TrackPaths, limit: F, path: usize, hint: Option<Spot>, point: Vf, refuse: bool, holding_only: bool,
        out: &mut Spot, distance: Option<&mut F>,
    ) -> bool {
        if refuse && t.paths[path].flags & path_flags::KEEP_OFF != 0 {
            return false;
        }
        out.path = path as u32 + 1;
        let (mut node, mut walked, mut wraps) = (Some(0), F(0.0), false);
        match hint {
            Some(h) if h.path == out.path => {
                node = h.piece.checked_sub(1).map(|n| n as usize);
                wraps = t.paths[path].links.contains(&path);
                if let Some(n) = node {
                    walked = F(fpu::neg(h.along.0)) * t.paths[path].nodes[n].span;
                }
            }
            Some(h) => walked = t.distance(h, Spot::on(path, 0, F(0.0)), refuse),
            None => {}
        }
        let nodes = &t.paths[path].nodes;
        let next = |n: usize| (n + 1 < nodes.len()).then_some(n + 1);
        let (px, pz) = (F(point.x()), F(point.z()));
        if let Some(n) = node
            && next(n).is_none()
        {
            out.piece = n as u32 + 1;
            out.along = F(0.0);
            if let Some(d) = distance {
                let c = nodes[n].points[0];
                let (dx, dz) = (px - F(c.x()), pz - F(c.z()));
                *d = Acc::mula(dx, dx).madd(dz, dz).sqrt();
            }
            return true;
        }
        let mut best = F(-1.0);
        while let Some(i) = node {
            let Some(j) = next(i) else { break };
            if !(seen(limit) < F(0.0) || seen(walked) < seen(limit)) {
                break;
            }
            if !holding_only || ring_holds(point, &nodes[i], &nodes[j]) {
                let (a, b) = (nodes[i].points[0], nodes[j].points[0]);
                let (sx, sz) = (F(b.x()) - F(a.x()), F(b.z()) - F(a.z()));
                let (rx, rz) = (px - F(a.x()), pz - F(a.z()));
                let len = nodes[i].span;
                let along = Acc::mula(sx, rx).madd(sz, rz) / (len * len);
                let along = if !(along <= F(1.0)) {
                    F(1.0)
                } else if along < F(0.0) {
                    F(0.0)
                } else {
                    along
                };
                let (ox, oz) = (rx - sx * along, rz - sz * along);
                let d2 = Acc::mula(ox, ox).madd(oz, oz);
                if d2 < best || best < F(0.0) {
                    best = d2;
                    out.piece = i as u32 + 1;
                    out.along = along;
                }
            }
            walked = walked + nodes[i].span;
            node = Some(j);
            if wraps && next(j).is_none() {
                node = Some(0);
                wraps = false;
            }
        }
        if let Some(d) = distance {
            *d = if best <= F(0.0) { best } else { best.sqrt() };
        }
        true
    }

    /// The nearest place to `point` on any path of `route` (the original's route locate): each path in
    /// turn through [`locate_on`] with the same arguments, a path [`path_flags::KEEP_OFF`] left out when
    /// `refuse`. The first answer is taken whatever it is, a later one when its distance is not below
    /// 0 and is below the one taken — or when the one taken is still −1 (nothing placed). `out` gets
    /// the place, `distance` its distance.
    ///
    /// The places come through one scratch spot the paths take turns writing, so a path that places
    /// nothing hands on its own path with the stretch and `along` the path before it left. The
    /// original's scratch, and the place it gives when every path is left out, are words on its stack
    /// it never set: here they start as [`Spot::default`] (see the journal).
    ///
    /// `false` on an empty route, or when [`locate_on`] refuses a path (it cannot here).
    #[allow(clippy::too_many_arguments)]
    pub fn route_locate(
        t: &mut TrackPaths, limit: F, route: &[usize], hint: Option<Spot>, point: Vf, refuse: bool, holding_only: bool,
        out: Option<&mut Spot>, distance: Option<&mut F>,
    ) -> bool {
        let unset = (Spot::default(), Spot::default());
        route_locate_over(t, limit, route, hint, point, refuse, holding_only, out, distance, unset)
    }

    /// [`route_locate`] with its unset words given: the place it starts with and its scratch spot.
    #[allow(clippy::too_many_arguments)]
    fn route_locate_over(
        t: &mut TrackPaths, limit: F, route: &[usize], hint: Option<Spot>, point: Vf, refuse: bool, holding_only: bool,
        out: Option<&mut Spot>, distance: Option<&mut F>, (mut took, mut scratch): (Spot, Spot),
    ) -> bool {
        if route.is_empty() {
            return false;
        }
        let mut best = F(-1.0);
        for &p in route {
            if refuse && t.paths[p].flags & path_flags::KEEP_OFF != 0 {
                continue;
            }
            let mut d = F(0.0);
            if !locate_on(t, limit, p, hint, point, refuse, holding_only, &mut scratch, Some(&mut d)) {
                return false;
            }
            if seen(best) == F(-1.0) || (!(seen(d) < F(0.0)) && seen(d) < seen(best)) {
                took = scratch;
                best = d;
            }
        }
        if let Some(o) = out {
            *o = took;
        }
        if let Some(d) = distance {
            *d = best;
        }
        true
    }

    /// The path `from` leads on to, as the look-ahead picks it; `None` where the original reads past
    /// its link list or never returns (see [`look_ahead`]).
    ///
    /// With path flag `0x8`, the link whose `+0xc` word, read as a speed, is nearest `speed`. Else
    /// with flag `0x20`, link `count × draw` (truncated). Else by [`TrackPaths::pick`]: the shortest
    /// (3), the shortest but a checkpoint path the longest (2), the longest (1), or the longest
    /// checkpoint path (anything else). With `refuse` a [`path_flags::KEEP_OFF`] link is passed over;
    /// when every link is, the first is taken. A tie keeps the first found.
    fn pick_link(t: &TrackPaths, from: usize, speed: F, draw: F, refuse: bool) -> Option<usize> {
        let path = &t.paths[from];
        let links = &path.links;
        let first = *links.first()?;
        let off = |l: usize| refuse && t.paths[l].flags & path_flags::KEEP_OFF != 0;
        if path.flags & 0x8 != 0 {
            let (mut best, mut pick) = (F(-1.0), first);
            for &l in links.iter().filter(|&&l| !off(l)) {
                let gap = (F(f32::from_bits(t.paths[l].word_0c)) - speed).abs();
                if seen(gap) < seen(best) || seen(best) == F(-1.0) {
                    (best, pick) = (gap, l);
                }
            }
            return Some(pick);
        }
        if path.flags & 0x20 != 0 {
            let k = (F(fpu::from_i32(links.len() as i32)) * draw).to_i32();
            let l = *links.get(usize::try_from(k).ok()?)?;
            return (!off(l)).then_some(l);
        }
        let (mut best, mut pick) = (F(-1.0), first);
        for &l in links.iter().filter(|&&l| !off(l)) {
            let len = seen(t.paths[l].length);
            let checkpoint = t.paths[l].flags & path_flags::CHECKPOINT != 0;
            let longest = match t.pick {
                3 => false,
                2 => checkpoint,
                1 => true,
                _ if checkpoint => true,
                _ => continue,
            };
            let better = if longest { !(len <= seen(best)) } else { len < seen(best) };
            if better || seen(best) == F(-1.0) {
                (best, pick) = (len, l);
            }
        }
        Some(pick)
    }

    /// The place `reach` on along the paths from `spot` (the vehicle brain's path look-ahead, its
    /// vt+0x88 with no route given), into `out`; and, with `near`, the nearest place to `point` on the
    /// paths that took ([`route_locate`] with `limit`, hinted at `spot`; its unset spots are the ones
    /// the measure to the path's end left on the stack, `spot` the scratch and the path's last stretch
    /// end the place it starts with).
    ///
    /// From `spot` to the end of its path is the way along it to its last stretch's end
    /// ([`TrackPaths::distance`]). While that, and the length of each path picked after it, add up to
    /// less than `reach`, the way goes on to the path [`pick_link`] picks at the end of the last one
    /// (`speed`, the first of the brain's ten `draws`, `refuse`). The place is then found on the last
    /// path, from `spot` or the path's start, stretch by stretch: `along` the part of the stretch the
    /// rest of `reach` covers, over the stretch's span. Each path the way takes holds a route-pool
    /// entry, all handed back at the end.
    ///
    /// `false`, nothing written and no entry kept, when `refuse` and `spot`'s path is
    /// [`path_flags::KEEP_OFF`], when the pool runs dry, or when the paths end before `reach` (`out` not
    /// written then either); and when the route locate fails (`out` written). Also `false` where the
    /// original does not answer at all: `spot` on a one-node path (its measure to the path's end
    /// recurses without end), a fork with no links, a flag-`0x20` draw outside the links, or a refused
    /// drawn link (the original spins for ever).
    #[allow(clippy::too_many_arguments)]
    pub fn look_ahead(
        t: &mut TrackPaths, reach: F, speed: F, limit: F, spot: Spot, point: Vf, draws: &[F; 10], refuse: bool,
        holding_only: bool, out: &mut Spot, near: Option<&mut Spot>,
    ) -> bool {
        let Some((first, from)) = spot.at() else { return false };
        if refuse && t.paths[first].flags & path_flags::KEEP_OFF != 0 {
            return false;
        }
        let last = t.paths[first].nodes.len() - 1;
        if last == 0 {
            return false;
        }
        let end = Spot::on(first, last - 1, F(1.0));
        let to_end = t.distance(spot, end, false);
        if t.free_routes == 0 {
            return false;
        }
        t.free_routes -= 1;
        let mut route = vec![first];
        let (mut path, mut node, mut along) = (first, from, spot.along);
        let (mut total, mut length) = (to_end, to_end);
        if seen(total) < seen(reach) {
            loop {
                let Some(next) = pick_link(t, path, speed, draws[0], refuse) else {
                    t.free_route(&mut route);
                    return false;
                };
                (path, node, along, length) = (next, 0, F(0.0), t.paths[next].length);
                if t.free_routes == 0 {
                    t.free_route(&mut route);
                    return false;
                }
                t.free_routes -= 1;
                total = total + length;
                route.push(next);
                if !(seen(total) < seen(reach)) {
                    break;
                }
            }
        }
        total = total - length;
        loop {
            if !(seen(total) < seen(reach)) {
                break;
            }
            total = total + (F(1.0) - along) * t.paths[path].nodes[node].span;
            if !(seen(total) < seen(reach)) {
                break;
            }
            node += 1;
            along = F(0.0);
            if node == t.paths[path].nodes.len() {
                t.free_route(&mut route);
                return false;
            }
        }
        let span = t.paths[path].nodes[node].span;
        *out = Spot::on(path, node, (span + (reach - total)) / span);
        let found = match near {
            // The route locate's unset words lie where the measure to the path's end copied its two
            // spots.
            Some(near) => {
                route_locate_over(t, limit, &route, Some(spot), point, refuse, holding_only, Some(near), None, (end, spot))
            }
            None => true,
        };
        t.free_route(&mut route);
        found
    }

    // --- the path follower (S309) ---------------------------------------------------------------

    /// The brain's first flag word: the path reach looks [`Follower::look`] ahead along the paths and
    /// drops the drift where the way there turns away from the path.
    pub const LOOKS_AHEAD: u64 = 0x0100_0000;
    /// The brain's first flag word: the follower drifts across its path.
    pub const DRIFTS: u64 = 0x0080_0000;
    /// The brain's first flag word: the target speed blends towards the next node's along the stretch.
    pub const BLENDS_SPEED: u64 = 0x8000;
    /// A path node's `+4` word: no drift on this stretch.
    pub const NO_DRIFT: u32 = 0x10;
    /// A node's speed word (`+0x44`) that asks for none of its own: [`DEFAULT_SPEED`] instead.
    const NO_SPEED: f32 = -1.0;
    /// The speed a node with no speed of its own is ridden at.
    pub const DEFAULT_SPEED: f32 = 150.0;
    /// What the follower leaves in the controller's three angles (the brain's `+0x1f0`).
    pub const ANGLES_REST: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

    /// The path follower's part of the brain: what the path branch, its reach and its route follower
    /// read and write.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Follower {
        /// The first flag word ([`LOOKS_AHEAD`], [`DRIFTS`], [`BLENDS_SPEED`],
        /// [`super::seek::CLOSING`]).
        pub flags: u64,
        /// The seeker's state: 0 while the path takes the rider, 1 once it hands over.
        pub state: i32,
        /// The rider's position and forward, and its own speed (the brain's copies).
        pub position: Vf,
        pub forward: Vf,
        pub own_speed: F,
        /// Where on the net the rider is (path 0 until the net search places it).
        pub spot: Spot,
        /// Ten draws in 0..1, drawn again each time the rider changes path; the look-ahead reads
        /// the first at a drawn fork.
        pub draws: [F; 10],
        /// The point aimed for, the heading and the target speed the controller rides to.
        pub aim: Vf,
        pub heading: Vf,
        pub speed: F,
        /// The controller's three angles, set to [`ANGLES_REST`].
        pub angles: Vf,
        /// How far across the path the aim sits (`+` to the right of the heading), and the side
        /// step that holds it for a while: the step, how long it lasts, how fast it dies away and
        /// its clock.
        pub drift: F,
        pub kick: F,
        pub kick_time: F,
        pub kick_decay: F,
        pub kick_clock: F,
        /// The drift it moves to, the range that is drawn from, how fast it moves and the most
        /// it may be.
        pub drift_target: F,
        pub drift_low: F,
        pub drift_high: F,
        pub drift_rate: F,
        pub drift_max: F,
        /// The clock to the next draw, the range its period is drawn from, and the period.
        pub drift_clock: F,
        pub period_low: F,
        pub period_high: F,
        pub period: F,
        /// The steering reach: `base + gain ·` the flat distance to the spot, at most `max`.
        pub reach: super::seek::Reach,
        /// The target speed when the route follower is told not to follow the nodes' speeds.
        pub cruise: F,
        /// How far ahead [`LOOKS_AHEAD`] looks, and the cosine the way there may be off the path.
        pub look: F,
        pub look_cone: F,
    }

    /// The point the seeker heads for once the path hands it over (the target jump's setup leg),
    /// and its own place on the net, found once.
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Goal {
        pub point: Vf,
        pub spot: Spot,
    }

    /// A compare as the FPU's `c.olt.s` makes it.
    fn lt(a: F, b: F) -> bool {
        seen(a) < seen(b)
    }

    /// `c.ole.s`.
    fn le(a: F, b: F) -> bool {
        seen(a) <= seen(b)
    }

    /// `v` with its `y` 0.
    fn level(v: Vf) -> Vf {
        Vf([v.x(), 0.0, v.z(), v.w()])
    }

    /// The flat distance from `p` to `q`, by `mula`/`madd` and `sqrt.s`.
    fn flat_distance(p: Vf, q: Vf) -> F {
        let (dx, dz) = (F(p.x()) - F(q.x()), F(p.z()) - F(q.z()));
        Acc::mula(dx, dx).madd(dz, dz).sqrt()
    }

    /// `p` moved `drift` across the way `heading` points: `x + drift·heading.z`, `z − drift·heading.x`.
    fn across(p: Vf, drift: F, heading: Vf) -> Vf {
        let x = F(p.x()) + drift * F(heading.z());
        let z = F(p.z()) - drift * F(heading.x());
        Vf([x.0, p.y(), z.0, p.w()])
    }

    /// The path and node a spot names, where its path has that node. The route locate can hand back a
    /// spot on one path with another path's node (see [`route_locate`]); the port's spot cannot say
    /// which, and reads it as naming nothing.
    fn node_of(t: &TrackPaths, spot: Spot) -> Option<(usize, usize)> {
        spot.at().filter(|&(p, n)| p < t.paths.len() && n < t.paths[p].nodes.len())
    }

    /// The `+4` word of the node a spot is on (0 for a spot on no node: the original reads its null).
    fn node_word(t: &TrackPaths, spot: Spot) -> u32 {
        node_of(t, spot).map_or(0, |(p, n)| t.paths[p].nodes[n].word_04)
    }

    /// A node's speed word as the follower takes it: [`DEFAULT_SPEED`] for −1.
    fn node_speed(word: u32) -> F {
        let w = F(f32::from_bits(word));
        if seen(w) == F(NO_SPEED) { F(DEFAULT_SPEED) } else { w }
    }

    /// How far ahead the rider steers along its path (the vehicle brain's vt+0x78, the path reach),
    /// `a` and `b` its two floats (the path branch passes 0 and 0), `follow` its flag word (1 from
    /// the path branch).
    ///
    /// With a spot and `follow`: under [`LOOKS_AHEAD`], the place [`Follower::look`] on along the
    /// paths ([`look_ahead`], no nearest place) becomes the aim, made the level unit way there from
    /// the spot's point; when that is more than [`Follower::look_cone`] off the level unit way the
    /// path runs at the spot, the drift is dropped. Then the reach is `base + gain ·` the flat
    /// distance from the rider to the spot's point moved [`Follower::drift`] across the heading, at
    /// most `max`.
    ///
    /// Otherwise the spot is looked for afresh ([`spot`]): at the rider when it has none, else at
    /// `start`'s point 20 back along its direction when `a < b` and there is one (the race file's
    /// `+0x5c` object, its `+0x40` and `+0x50`: what it is was not traced), else at `point`. The
    /// reach is `base + gain ·` the flat distance to the spot's point, at most 0 when `a < b` and
    /// `b − base` when not.
    ///
    /// Points the original takes off its stack where a query does not answer are 0 here (see the
    /// journal); the queries always answer on a placed spot of a net as loaded.
    #[allow(clippy::too_many_arguments)]
    pub fn reach(f: &mut Follower, t: &mut TrackPaths, a: F, b: F, follow: bool, point: Vf, start: Option<(Vf, Vf)>) -> F {
        if f.spot.path != 0 && follow {
            if f.flags & LOOKS_AHEAD != 0 {
                let mut ahead = Spot::default();
                let draws = f.draws;
                if look_ahead(t, f.look, f.own_speed, f.look, f.spot, f.position, &draws, false, false, &mut ahead, None) {
                    t.spot_point(&mut ahead, &mut f.aim);
                    let mut way = Vf::default();
                    t.path_ahead(f.spot, &mut way);
                    let mut here = Vf::default();
                    t.spot_point(&mut f.spot, &mut here);
                    f.aim = unit(level(sub(f.aim, here)));
                    let way = unit(level(way));
                    if lt(Acc::mula(f.aim.x(), way.x()).madd(f.aim.z(), way.z()), f.look_cone) {
                        f.drift = F(0.0);
                    }
                }
            }
            let mut here = Vf::default();
            t.spot_point(&mut f.spot, &mut here);
            let r = f.reach.base + f.reach.gain * flat_distance(across(here, f.drift, f.heading), f.position);
            return if le(r, f.reach.max) { r } else { f.reach.max };
        }
        if f.spot.path == 0 {
            spot(t, f.position, false, &mut f.spot, None);
        } else {
            let at = match start {
                Some((at, dir)) if lt(a, b) => sub(at, scale(dir, F(20.0))),
                _ => point,
            };
            spot(t, at, false, &mut f.spot, None);
        }
        let mut here = Vf::default();
        t.spot_point(&mut f.spot, &mut here);
        let r = f.reach.base + f.reach.gain * flat_distance(here, f.position);
        let most = if lt(a, b) { F(0.0) } else { b - f.reach.base };
        if le(r, most) { r } else { most }
    }

    /// The vehicle brain's path look-ahead with its route word (vt+0x88): without a `goal`, the
    /// [`look_ahead`]; with one, the place `reach` on from `spot` along the route search's way to the
    /// goal ([`TrackPaths::route`] keeping off [`path_flags::KEEP_OFF`] paths when `refuse`, then
    /// [`TrackPaths::route_walk`]), and the nearest place to `point` on that route
    /// ([`route_locate`] with `limit`, hinted at `spot`). The route's entries go back to the pool.
    /// `speed` and `draws` are not read on the route.
    ///
    /// On the route all three run whatever the others answer, and the answer is the bitwise and of
    /// theirs (the route search's −1 counts as found). The route locate's unset words are the goal
    /// (the place it starts with: the route search's copy of it) and `spot` (its scratch: the walk's
    /// copy), left where its frame lies.
    ///
    /// `false`, nothing run, for a spot or goal that names no node (the original reads its null).
    #[allow(clippy::too_many_arguments)]
    pub fn route_look_ahead(
        t: &mut TrackPaths, reach: F, speed: F, limit: F, spot: Spot, point: Vf, draws: &[F; 10], refuse: bool,
        holding_only: bool, goal: Option<Spot>, out: &mut Spot, near: Option<&mut Spot>,
    ) -> bool {
        let Some(goal) = goal else {
            return look_ahead(t, reach, speed, limit, spot, point, draws, refuse, holding_only, out, near);
        };
        if node_of(t, spot).is_none() || node_of(t, goal).is_none() {
            return false;
        }
        let mut route = Vec::new();
        let found = t.route(spot, goal, Some(&mut route), None, refuse);
        let walked = t.route_walk(reach, spot, &route, out);
        let near_found = route_locate_over(t, limit, &route, Some(spot), point, refuse, holding_only, near, None, (goal, spot));
        t.free_route(&mut route);
        found & i32::from(walked) & i32::from(near_found) != 0
    }

    /// The vehicle brain's route follower (vt+0x8c): `reach` on along the paths ([`route_look_ahead`]
    /// from the rider's spot with the brain's speed and `max` reach as its limit, the goal's spot as
    /// its route) gives the aim, the heading, the spot and the target speed. `follow` is its flag word
    /// (1 from the path branch). Each `rand()` is one [`Rng::unit`](crate::rng::Rng::unit).
    ///
    /// When the look-ahead finds nothing the rider holds its own: speed its own, aim its position,
    /// heading its forward. Otherwise:
    /// - a nearest place on another path than the spot's draws the ten [`Follower::draws`] afresh;
    /// - the drift clock runs; at its period a new drift target is drawn between `drift_low` and
    ///   `drift_high` (one more draw below ½ flips its sign) and a new period between `period_low`
    ///   and `period_high`;
    /// - the nearest place becomes the spot, the place ahead the aim (left as it was where the place
    ///   names the last node with `along` off 0) and the way the path runs there the heading (left
    ///   where it runs nowhere);
    /// - on a [`NO_DRIFT`] node, or without [`DRIFTS`], the drift is 0; else it moves towards its
    ///   target by at most `drift_rate · dt` and is held within `drift_max`;
    /// - with no race, or a race word with bit 0, the side step takes over while its clock is within
    ///   its time (the drift is the step, which dies away by `kick_decay · dt`), and is 0 after;
    /// - the aim moves the drift across the heading, the heading turns a twentieth of the way to the
    ///   aim, level and unit;
    /// - the speed is the node's ([`DEFAULT_SPEED`] for −1; [`Follower::cruise`] without `follow`),
    ///   blended along the stretch to the next node's under [`BLENDS_SPEED`].
    ///
    /// The angles are [`ANGLES_REST`] either way.
    #[allow(clippy::too_many_arguments)]
    pub fn route_follow(
        f: &mut Follower, t: &mut TrackPaths, dt: F, reach: F, follow: bool, goal: Option<Spot>, race_word: Option<u32>,
        rng: &mut crate::rng::Rng,
    ) {
        let (mut ahead, mut near) = (Spot::default(), Spot::default());
        let draws = f.draws;
        let found =
            route_look_ahead(t, reach, f.own_speed, f.reach.max, f.spot, f.position, &draws, false, false, goal, &mut ahead, Some(&mut near));
        if !found {
            f.speed = f.own_speed;
            f.aim = f.position;
            f.heading = f.forward;
            f.angles = ANGLES_REST;
            return;
        }
        if near.path != f.spot.path {
            for d in &mut f.draws {
                *d = rng.unit();
            }
        }
        f.drift_clock = f.drift_clock + dt;
        if !lt(f.drift_clock, f.period) {
            let r = rng.unit();
            f.drift_target = f.drift_low + (f.drift_high - f.drift_low) * r;
            if lt(rng.unit(), F(0.5)) {
                f.drift_target = -f.drift_target;
            }
            let r = rng.unit();
            f.period = f.period_low + (f.period_high - f.period_low) * r;
            f.drift_clock = F(0.0);
        }
        f.spot = near;
        t.spot_point(&mut ahead, &mut f.aim);
        t.path_ahead(ahead, &mut f.heading);
        // The original also takes the way the path runs at the new spot, into a word it never reads.
        if node_word(t, f.spot) & NO_DRIFT != 0 || f.flags & DRIFTS == 0 {
            f.drift = F(0.0);
        } else {
            let step = f.drift_rate * dt;
            let mut by = f.drift_target - f.drift;
            if le(by, step) {
                if lt(by, -step) {
                    by = -step;
                }
            } else {
                by = step;
            }
            f.drift = f.drift + by;
            if le(f.drift, f.drift_max) {
                if lt(f.drift, -f.drift_max) {
                    f.drift = -f.drift_max;
                }
            } else {
                f.drift = f.drift_max;
            }
        }
        if race_word.is_none_or(|w| w & 1 != 0) {
            if le(f.kick_clock, f.kick_time) {
                f.kick_clock = f.kick_clock + dt;
                f.drift = f.kick;
                f.kick = f.kick - f.kick_decay * dt;
            } else {
                f.kick = F(0.0);
            }
        }
        f.aim = across(f.aim, f.drift, f.heading);
        let turn = scale(sub(f.aim, f.position), F(0.05));
        f.heading = unit(level(add(f.heading, turn)));
        if !follow {
            f.speed = f.cruise;
        } else if let Some((p, n)) = node_of(t, f.spot) {
            let nodes = &t.paths[p].nodes;
            f.speed = node_speed(nodes[n].word_44);
            if f.flags & BLENDS_SPEED != 0
                && let Some(next) = nodes.get(n + 1)
            {
                let along = f.spot.along;
                let next_speed = F(f32::from_bits(next.word_44));
                f.speed = if seen(next_speed) == F(NO_SPEED) {
                    Acc::mula(f.speed, F(1.0) - along).madd(F(DEFAULT_SPEED), along)
                } else {
                    Acc::mula(f.speed, F(1.0) - along).madd(along, next_speed)
                };
            }
        }
        f.angles = ANGLES_REST;
    }

    /// The tick's path branch (the original's ai_tick_flag when the seeker has no state and the brain
    /// a net, [`super::seek::Seek::Path`]); the caller goes on to [`tail`].
    ///
    /// The goal's spot is looked for once ([`spot`] at its point, while it has none), then the rider's
    /// the same way. The [`reach`] (0, 0, following, at the rider) goes to the [`route_follow`]
    /// (`dt`, following, the goal's spot as its route). With a goal, once the way along the paths
    /// from the rider's spot to the goal's ([`TrackPaths::distance`], keeping off) is no longer than
    /// the straight one from the rider to the goal's point, the seeker takes over: state 1 and
    /// [`super::seek::CLOSING`] cleared.
    ///
    /// The goal's spot has to name a node by the measure; the port leaves the hand-over out where it
    /// does not (the original reads its null).
    pub fn path(
        f: &mut Follower, t: &mut TrackPaths, dt: F, mut goal: Option<&mut Goal>, race_word: Option<u32>,
        rng: &mut crate::rng::Rng,
    ) {
        if let Some(g) = goal.as_deref_mut()
            && g.spot.path == 0
        {
            spot(t, g.point, false, &mut g.spot, None);
        }
        if f.spot.path == 0 {
            spot(t, f.position, false, &mut f.spot, None);
        }
        let position = f.position;
        let r = reach(f, t, F(0.0), F(0.0), true, position, None);
        route_follow(f, t, dt, r, true, goal.as_ref().map(|g| g.spot), race_word, rng);
        let Some(g) = goal else { return };
        let d = sub(f.position, g.point);
        let straight = Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madd(d.z(), d.z()).sqrt();
        if node_of(t, f.spot).is_none() || node_of(t, g.spot).is_none() {
            return;
        }
        if le(t.distance(f.spot, g.spot, true), straight) {
            f.state = 1;
            f.flags &= !super::seek::CLOSING;
        }
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
