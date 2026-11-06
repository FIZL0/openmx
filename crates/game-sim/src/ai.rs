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
//! # Avoidance (S12c)
//!
//! Every tick a motocross AI rider takes the direction its route asks for and bends it away from the
//! other riders near it, and may lower the speed it aims for: [`avoid`]. That is the swerve and the
//! brake an opponent shows on free09 when it comes up on another rider. It also builds a fan of
//! look-ahead feelers ([`feeler_fan`]) and sweeps it through the scene's objects when the flags allow
//! ([`look`]) — but the motocross update throws the sweep's answer away: nothing it steers or brakes by
//! reads the feelers. They are kept in the port because the original builds them and the flags gate
//! them; the scene sweep itself (a sweep of the fan against each solid object) is not ported.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::camera::CurveTable;
use crate::rng::Rng;
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

// --- avoidance (S12c) -------------------------------------------------------------------------------

/// The bits of the AI controller's flag word that the avoidance reads or writes.
pub mod flag {
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

/// How wide the motocross fan is, edge to edge: a quarter turn's half, as the original's literal.
/// (The generic vehicle AI fans twice as wide; that path is not ported.)
pub const MX_FAN_SPREAD: f32 = 0.7853982;

const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;
const INV_TWO_PI: f32 = 0.15915494;
/// The curve table's spacing and its inverse, as the startup code leaves them in RAM.
const CURVE_STEP: f32 = 0.0010473721;
const CURVE_INV_STEP: f32 = 954.77045;

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
    let a = wrap(wrap(angle));
    let s = (F(PI) + a) * F(CURVE_INV_STEP);
    let left = s - F(fpu::from_i32(s.to_i32()));
    let (cos, sin) = curve.coarse(angle);
    (cos - F(CURVE_STEP) * (sin * left), sin + F(CURVE_STEP) * (cos * left))
}

/// Whole turns off by truncation, then one correction into `-pi..pi` (anything not at or under pi,
/// NaN included, loses a turn).
fn wrap(angle: F) -> F {
    let a = angle - F(TWO_PI) * F(fpu::from_i32((angle * F(INV_TWO_PI)).to_i32()));
    if !(a <= F(PI)) {
        a - F(TWO_PI)
    } else if a < F(-PI) {
        a + F(TWO_PI)
    } else {
        a
    }
}

/// `mula`/`madda`/`madd` on the FPU: `(a.x·b.x + a.y·b.y) + a.z·b.z` through the accumulator.
fn dot(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
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
