//! **MX vs. ATV's ground-step clauses (MD63)**: the ones SLUS-21104 changed from MX Unleashed's,
//! ported as they read there and proved against its own code (`tests/mxvatv_step.rs`).
//!
//! - The clutch's launch ramp ([`clutch`]): rearming it also clears the multiplier it started at and
//!   the launch's share of the centre-of-gravity travel, and the travel itself is gone — MX vs. ATV
//!   never moves the centre of gravity back on a launch (MX Unleashed's [`crate::r#move::clutch`]
//!   does). The multiplier is still computed and written to the engine as before.
//! - The preload ([`preload`]): a file whose min and max preload times are equal fires with
//!   `−min` (the branch's delay slot subtracts the minimum from the zero it loaded; MX Unleashed's
//!   gives zero), and a cue: the first substep the timer stands above zero the bike posts the
//!   preload's event (category 8, id 14) and holds a latch until the timer is back at or under zero.
//! - The input limiter ([`input_limiter`], MD63a): no squaring of the stick, a gas cut for the first
//!   frames the throttle is back on, and the gas held at 0 on the rider's balance stunt.
//! - The steering command ([`steer_command`], MD63a): `[Joystick]`'s input ramp brings part of the
//!   lock in over a time while the stick is held hard over, when its option is on.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::{Air, Chassis, Params as AirParams, Step as AirStep};
use crate::phy::Joystick;
use crate::r#move::{Engine, Params, Ride, Step};
use crate::ride::{Controls, Held, limit, rise, toward};
use crate::trick::Stick;
use ps2_float::{Acc, Dest, F};

/// `x` from `a..b` onto `out0..out1`, clamped to that range: `(x − a) / (b − a)` on the FPU's
/// accumulator lerp (`adda`, `madd`), then `max` with the low end and `min` with the high end.
fn map_clamp(x: F, a: F, b: F, out0: F, out1: F) -> F {
    let u = (x - a) / (b - a);
    out0.lerp(out1, u).max(out0.min(out1)).min(out0.max(out1))
}

/// One frame of MX vs. ATV's clutch ramp (SLUS-21104's, once a frame in the ground step's head, after
/// the gear choice). Its three cases are MX Unleashed's ([`crate::r#move::clutch`]): clutch in and
/// throttle pinned rearms it, clutch out runs it, anything else leaves the engine's multiplier where
/// it was. What differs is above (the module's doc). An empty range is skipped as `start − 0`, the
/// branch's delay slot, which is the start itself.
pub fn clutch(p: &Params, r: &mut Ride, eng: &mut Engine, s: &Step) {
    if s.clutch == F::ONE && s.gas == F::ONE {
        r.clutch_timer = F::ZERO;
        r.clutch_scale_start = F::ZERO;
        r.clutch_cofg_weight = F::ZERO;
        return;
    }
    if !(s.clutch == F::ZERO && r.clutch_timer <= p.clutch_torque_scale_time) {
        return;
    }
    if r.clutch_timer == F::ZERO {
        let k = p.max_clutch_torque_scale_speed;
        r.clutch_scale_start = if k == F::ZERO {
            p.max_clutch_torque_scale - F::ZERO
        } else {
            map_clamp(s.speed, F::ZERO, k, p.max_clutch_torque_scale, F::ONE)
        };
        // Still made, and read by nothing: the travel it weighed is gone.
        r.clutch_cofg_weight = if k == F::ZERO { F::ONE } else { map_clamp(s.speed, F::ZERO, k, F::ONE, F::ZERO) };
    }
    r.clutch_timer += s.dt;
    let t = p.clutch_torque_scale_time;
    eng.clutch_torque_scale = if t == F::ZERO {
        r.clutch_scale_start - F::ZERO
    } else {
        map_clamp(r.clutch_timer, F::ZERO, t, r.clutch_scale_start, F::ONE)
    };
}

/// The stick past which the preload arms (pulled back) and fires (pushed forward again).
const HELD: f32 = 0.8;

/// One substep of MX vs. ATV's preload (SLUS-21104's, between the roll/pitch control and the
/// backflip). The lip, the arming and the firing are MX Unleashed's ([`crate::air::preload`]) in the
/// original's own comparisons; what differs is the equal-times force and the cue (the module's doc).
/// `cue` is the latch; the answer is whether this substep posted the cue's event.
pub fn preload(p: &AirParams, air: &mut Air, chassis: &mut Chassis, step: &AirStep, cue: &mut bool) -> bool {
    if air.preload_firing {
        air.preload_timer = air.preload_timer - step.dt;
        if F::ZERO < air.preload_timer {
            chassis.force = chassis.force.merge(Dest::XYZ, chassis.force.add(air.preload_force));
        } else {
            air.preload_firing = false;
        }
    } else if !air.on_ground
        || chassis.vel.y() <= 5.0
        || air.contact_vel.z() <= 30.0
        || step.brake != F::ZERO
    {
        air.preload_ready = false;
    } else {
        air.preload_ready = true;
        let y = step.stick.1;
        if !(y <= F(HELD)) {
            air.preload_armed = true;
            air.preload_timer = F::ZERO;
        } else if y < F(-HELD) && air.preload_armed {
            let (from, to) = if step.dpad_y {
                (p.min_dpad_preload_time, p.max_dpad_preload_time)
            } else {
                (p.min_preload_time, p.max_preload_time)
            };
            let force = if from == to { F::ZERO - from } else { map_clamp(step.stick_hold.abs(), from, to, F::ZERO, p.max_preload) };
            air.preload_force = air.preload_force.merge(Dest::Y, ps2_float::Vf::splat(force.0));
            air.preload_timer = p.preload_time;
            air.preload_armed = false;
            air.preload_firing = true;
        } else if air.preload_armed {
            air.preload_timer += step.dt;
        }
    }
    if *cue {
        if air.preload_timer <= 0.0 {
            *cue = false;
        }
        false
    } else if air.preload_timer <= 0.0 {
        false
    } else {
        *cue = true;
        true
    }
}

// --- the input limiter (MD63a) ------------------------------------------------------------------------

/// MX vs. ATV's gas cut after the throttle comes on ([`input_limiter`]): the latch is set while the
/// throttle is off, and once it is back on the gas lever is held at 0 until it is let go or the timer
/// has run past [`GAS_CUT_TIME`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GasCut {
    pub latched: bool,
    pub timer: F,
}

/// How long the gas cut holds. The bike's reset zeroes the word and nothing in SLUS-21104 writes it
/// again, so a cut always lasts two frames: the one the throttle comes on, which counts the timer
/// from 0, and the next, which finds it past 0 and lets go.
pub const GAS_CUT_TIME: F = F::ZERO;

/// What MX vs. ATV's limiter reads beyond MX Unleashed's: the bike is in the air (its gas gate's second
/// way through with a reverse asked for), and how long the gas cut holds ([`GAS_CUT_TIME`] on a ride).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LimiterIn {
    pub airborne: bool,
    pub cut_time: F,
}

/// One frame of MX vs. ATV's input limiter (SLUS-21104's, the ground head's first call), over the
/// port's [`Held`]: what MX Unleashed's rate limit ([`Held::step`]) does, with these differences, in
/// the original's order:
///
/// - the start-gate penalty's cut ([`Controls::gate_cut`]) is the limiter's own first call here;
/// - then the gas cut ([`GasCut`]) runs on the gas lever, so the start-line latch below sees it cut;
/// - the stick is **not squared**: [`Held::stick`] is `stick` as it comes, and the lean walks to it;
/// - the gas's zero gate is the rider's current stunt being the bike's balance stunt (the two words
///   the balance and the stand-upright compare) in place of the no-hander, which the port reads off
///   [`Controls::hands_off`] as before;
/// - a reverse asked for holds the gas at 0 unless the bike is in the air (MX Unleashed's has the same
///   gate; its port leaves it to the pad read's [`Controls::reverse_rule`], which already cut the
///   lever).
///
/// The answer is the controls as the original leaves its lever words: the gas lever cut, the brake
/// lever full under the start-line latch. The flick ([`Held::flick`]) is the port's, as on MX
/// Unleashed's handling.
pub fn input_limiter(held: &mut Held, cut: &mut GasCut, c: &Controls, stick: Stick, j: &Joystick, dt: F, inp: &LimiterIn) -> Controls {
    let mut c = c.gate_cut();
    if held.lock.throttle {
        cut.latched = true;
        cut.timer = F::ZERO;
    } else if cut.latched {
        if c.gas == F::ZERO || inp.cut_time < cut.timer {
            cut.latched = false;
        }
        cut.timer += dt;
        c.gas = F::ZERO;
    }

    held.flick = held.ring.sample(stick.y, dt);
    held.digital = stick.digital;
    held.stick = (stick.x, stick.y);
    let rx = if stick.digital[0] { j.x_dpad_rate_limit } else { j.x_axis_rate_limit } * dt;
    let ry = if stick.digital[1] { j.y_dpad_rate_limit } else { j.y_axis_rate_limit } * dt;
    let lean_x = if held.lock.steer { F::ZERO } else { limit(held.lean.0, stick.x, rx) };
    held.lean = (lean_x, limit(held.lean.1, stick.y, ry));

    let l = &mut held.lock;
    if l.steer && !l.steer_was {
        l.latch = true;
    }
    if l.latch && (!l.steer || (!l.throttle && c.gas > F::ZERO && c.clutch == F::ZERO)) {
        l.latch = false;
    }
    if l.latch {
        c.brake = F::ONE;
        held.reverse = F::ZERO;
    }
    l.steer_was = l.steer;

    held.brake = rise(held.brake, c.brake, j.brake_rate_limit, dt);
    held.front_brake = rise(held.front_brake, c.front_brake.max(c.brake), j.brake_rate_limit, dt);
    held.rear_brake = rise(held.rear_brake, c.rear_brake.max(c.brake), j.brake_rate_limit, dt);
    held.gas = if held.lock.throttle || c.hands_off {
        F::ZERO
    } else if held.reverse == F::ZERO || inp.airborne {
        rise(held.gas, c.gas, j.gas_rate_limit, dt)
    } else {
        F::ZERO
    };
    let clutch_rate = if held.clutch < c.clutch { j.clutch_in_rate_limit } else { j.clutch_out_rate_limit };
    held.clutch = toward(held.clutch, c.clutch, clutch_rate, dt);
    c
}

// --- the steer ramp (MD63a) ---------------------------------------------------------------------------

/// The lean past which the steer ramp's clock runs: the stick held hard over.
const HARD_OVER: f32 = 0.987;

/// MX vs. ATV's steer ramp, `[Joystick]`'s `InputRampAmount` (read in degrees, kept in radians) and
/// `InputRampTime`, and the option that turns it on. The game keeps the option off in every image
/// read (a Free Ride and a quad's spawn) and nothing in SLUS-21104 was found setting it, so a ride
/// starts with it off, which is [`SteerRamp::default`]'s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SteerRamp {
    pub on: bool,
    pub amount: F,
    pub time: F,
}

/// The degrees `InputRampAmount` is read in, to radians: the reader's own constant.
const DEGREES: f32 = 0.01745329;

impl SteerRamp {
    /// The two keys as the file has them, the option off.
    pub fn of_file(amount_degrees: F, time: F) -> SteerRamp {
        SteerRamp { on: false, amount: amount_degrees * F(DEGREES), time }
    }
}

/// The front wheel's steering command in MX vs. ATV's ground head: with the ramp off, MX Unleashed's
/// ([`crate::r#move::steer_command`]); with it on, `amount` of the lock comes in over `time` while the
/// lean is held at [`HARD_OVER`] or past it (`timer` counts it, and is zeroed under it) and the speed
/// fade scales only the rest of the lock.
pub fn steer_command(ramp: &SteerRamp, lock: F, lean: F, fade: F, timer: &mut F, dt: F) -> F {
    if !ramp.on {
        return lock * (lean * fade);
    }
    *timer = if lean.abs() < F(HARD_OVER) { F::ZERO } else { *timer + dt };
    let t = *timer / ramp.time;
    let t = if F::ONE < t { F::ONE } else { t };
    Acc::mula(lean, ramp.amount * t).madd(lean * fade, lock - ramp.amount)
}
