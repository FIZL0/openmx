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
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::{Air, Chassis, Params as AirParams, Step as AirStep};
use crate::r#move::{Engine, Params, Ride, Step};
use ps2_float::{Dest, F};

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
