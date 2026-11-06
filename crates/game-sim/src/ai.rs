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
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::camera::CurveTable;
use crate::ragdoll::{FineAcos, Tables};
use crate::rng::Rng;
use ps2_float::{Dest, F, Vf, fpu, vu};

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
