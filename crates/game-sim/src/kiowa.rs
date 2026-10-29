//! The Kiowa helicopter's own flight: what the machine works out for itself before the rigid body it
//! rides turns it into motion.
//!
//! Like the biplane, the helicopter is one body in the engine's generic dynamics world, and its update
//! is a set of listeners the world calls back into. This module ports them one at a time, starting
//! with the piece the pad goes through first:
//!
//! 1. [`Controller::step`] — the flight controller, once a frame. The player never moves the rotor
//!    directly. The left stick asks for a ground speed (forward and back) and a bank (left and right),
//!    the rudder buttons for a yaw rate, and the climb and descend buttons for a climb rate. The
//!    controller turns those into body rates and chases them with the cyclic and the pedal, and holds
//!    the height it was left at with the collective.
//!
//! Every field is named by the `.phy` key it is loaded from, where it has one (the `[Flight
//! Controller]` section of `helicopter\kiowa.phy`). Until the reader lands, the proofs read the values
//! off a save state's live helicopter (`tests/kiowa.rs`).
//!
//! # The controller's three loops
//!
//! * **Pitch.** The speed command moves at `SpeedCommandRate` while the stick is forward or back, and
//!   snaps to the ground speed the frame the stick comes back to centre. Below `LowSpeedAero` the
//!   stick asks for a pitch angle directly; above it the speed error does, through `SpeedGain`. A
//!   banked helicopter fast enough (`MinPitchRateFeedForwardSpeed`) also asks for the pitch rate a
//!   level turn needs, scaled between the two feed-forward scales by ground speed. The pitch-angle
//!   error times `PitchGain`, plus that, is the pitch-rate command.
//! * **Roll.** The stick asks for a bank up to `MaxRollCommand`; the bank error times `RollGain` is the
//!   roll-rate command, limited to `MaxRateCommand` while the stick is held and to the smaller
//!   `MinRateCommand` when it is let go.
//! * **Yaw.** The rudder is a yaw-rate command straight off the pad.
//!
//! The three rate errors against the body's rates, times the three gains, are the cyclic and pedal,
//! each held to -1..1. The collective climbs at `ClimbRateGain` while climb is held below
//! `MaxAltitude`, descends at `DescentRateGain` while descend is held, and otherwise holds the height
//! it settled at once the hold delay has run out. It is a proportional-integral loop on the climb
//! rate, with the integral restarting at 0.3 whenever climb or descend is pressed.

#![deny(clippy::float_arithmetic)]

use crate::camera::CurveTable;
use ps2_float::fpu::{self, Acc};
use ps2_float::{F, Vf};

const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;
const INV_TWO_PI: f32 = 0.15915494;
/// Entries per radian of the curve table.
const CURVE_INV_STEP: f32 = 954.77045;
/// Gravity, feet per second squared.
const G: f32 = 32.2;
/// Where the collective's integral restarts when climb or descend is pressed.
const INTEGRAL_RESTART: f32 = f32::from_bits(0x3e99_999a);
/// The collective filter's target while the height is held.
const HOLD_TARGET: f32 = 0.5;

/// One wrap into `-pi..pi`: the whole turns taken off, then one correction either way.
fn wrap(angle: F) -> F {
    let a = angle - F(TWO_PI) * F(fpu::from_i32((angle * INV_TWO_PI).to_i32()));
    if a <= PI {
        if a < -PI { F(TWO_PI) + a } else { a }
    } else {
        a - TWO_PI
    }
}

/// `{cos, sin}` of an angle already in `-pi..pi`, read straight out of the curve table's entry below it.
fn curve(angle: F, table: &CurveTable) -> (F, F) {
    let i = ((F(PI) + angle) * CURVE_INV_STEP).to_i32();
    let [c, s] = table.entries()[i as usize];
    (F(c), F(s))
}

/// The pad as the helicopter keeps it, each -1..1 (the buttons 0 or 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    /// The climb button.
    pub climb: F,
    /// The descend button.
    pub descend: F,
    /// The stick left (-) and right (+): bank.
    pub lateral: F,
    /// The stick forward and back: ground speed.
    pub longitudinal: F,
    /// The rudder buttons: yaw rate.
    pub pedal: F,
}

/// What the controller leaves for the rotor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Commands {
    /// The collective, `MinCollective..1`.
    pub collective: F,
    /// The lateral cyclic, -1..1.
    pub lateral: F,
    /// The longitudinal cyclic, -1..1.
    pub longitudinal: F,
    /// The pedal, -1..1.
    pub pedal: F,
    /// The collective's filtered target (1 climbing, 0.5 holding, 0 descending), which the sound reads.
    pub filtered: F,
    /// The height as a fraction of `MaxAltitude`, written only while the collective is not flown.
    pub altitude_fraction: F,
}

/// The `[Flight Controller]` section, angles in radians and speeds in feet per second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controller {
    /// `LateralCyclicGain`: on the roll-rate error.
    pub lateral_cyclic_gain: F,
    /// `LongitudinalCyclicGain`: on the pitch-rate error.
    pub longitudinal_cyclic_gain: F,
    /// `PedalGain`: on the yaw-rate error.
    pub pedal_gain: F,
    /// `MaxAltitude`: no climbing above it.
    pub max_altitude: F,
    /// How long after climb or descend is let go the held height keeps following the helicopter.
    pub hold_delay: F,
    /// `CollectiveGain`: on the climb-rate error.
    pub collective_gain: F,
    /// `ClimbRateGain`: the climb rate climb asks for.
    pub climb_rate: F,
    /// `DescentRateGain`: the climb rate descend asks for.
    pub descent_rate: F,
    /// `AltitudeHoldGain`: the climb rate per foot below the held height.
    pub altitude_hold_gain: F,
    /// `CollectiveIntegratorGain`.
    pub integrator_gain: F,
    /// `MinCollective`.
    pub min_collective: F,
    /// `MaxPitchCommand`.
    pub max_pitch: F,
    /// `MaxRollCommand`.
    pub max_roll: F,
    /// `PitchGain`.
    pub pitch_gain: F,
    /// `RollGain`.
    pub roll_gain: F,
    /// `SpeedCommandRate`: how fast the stick moves the speed command.
    pub speed_command_rate: F,
    /// `MinSpeedCommand`: below it a centred stick asks for a standstill.
    pub min_speed_command: F,
    /// `MaxSpeedCommand`.
    pub max_speed_command: F,
    /// `SpeedGain`: pitch per foot per second of speed error.
    pub speed_gain: F,
    /// `MinPitchRateFeedForwardSpeed`: below this airspeed there is no turn feed-forward.
    pub min_feed_forward_speed: F,
    /// `MinPitchRateFeedForwardScale`.
    pub min_feed_forward_scale: F,
    /// `MaxPitchRateFeedForwardSpeed`.
    pub max_feed_forward_speed: F,
    /// `MaxPitchRateFeedForwardScale`.
    pub max_feed_forward_scale: F,
    /// `MinRateCommand`: pitch, yaw and roll rate limits (only roll's is read: the stick let go).
    pub min_rate: [F; 3],
    /// `MaxRateCommand`: pitch, yaw and roll rate limits.
    pub max_rate: [F; 3],
}

/// The controller's own state, carried frame to frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Loop {
    /// The ground speed asked for.
    pub speed_command: F,
    /// Last frame's longitudinal stick.
    pub last_longitudinal: F,
    /// The height being held.
    pub hold_altitude: F,
    /// Time left in which the held height follows the helicopter.
    pub hold_timer: F,
    /// The collective's integral.
    pub integral: F,
    /// The first-order lag on the collective's target.
    pub filter: Filter,
}

/// A first-order lag: each frame `value` moves `min(dt, time) / time` of the way to its target.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Filter {
    pub value: F,
    pub time: F,
    /// The last frame's fraction.
    pub step: F,
}

impl Filter {
    pub fn step(&mut self, target: F, dt: F) -> F {
        self.step = dt.min(self.time) / self.time;
        self.value = self.value + self.step * (target - self.value);
        self.value
    }
}

/// What the controller reads off the rest of the helicopter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flight {
    /// The helicopter is being flown. While it is not, every command is zero.
    pub flown: bool,
    /// The pilot's flag that, while climb is not held, leaves the collective at zero. Its meaning is not
    /// read; in the live state it is set.
    pub pilot_flag: bool,
    /// The body's height.
    pub height: F,
    /// The body's world velocity.
    pub velocity: [F; 3],
    /// The body's rates (pitch, yaw, roll), as the rigid body keeps them.
    pub rates: Vf,
    /// The airspeed.
    pub airspeed: F,
    /// The pitch angle.
    pub pitch: F,
    /// The roll angle.
    pub roll: F,
    /// The scale on gravity in the turn feed-forward (1 in the live state).
    pub gravity_scale: F,
    /// The aerodynamics' `LowSpeedAero`: below this ground speed the stick asks for pitch directly.
    pub low_speed_aero: F,
}

impl Controller {
    /// One frame of the flight controller.
    pub fn step(&self, pad: &Pad, at: &Flight, state: &mut Loop, out: &mut Commands, table: &CurveTable, dt: F) {
        if !at.flown {
            out.lateral = F(0.0);
            out.longitudinal = F(0.0);
            out.pedal = F(0.0);
            out.collective = F(0.0);
            return;
        }
        let [vx, vy, vz] = at.velocity;
        let ground = (vx * vx + vz * vz).sqrt();

        // The speed command.
        let stick = pad.longitudinal;
        if stick != 0.0 || state.last_longitudinal.abs() <= 0.0 {
            state.speed_command = state.speed_command + dt * (stick * self.speed_command_rate);
        } else {
            state.speed_command = ground;
        }
        state.speed_command = state.speed_command.max(-self.max_speed_command).min(self.max_speed_command);
        state.last_longitudinal = stick;
        let command = state.speed_command;
        let error = if command.abs() < self.min_speed_command && stick == 0.0 { -ground } else { command - ground };

        // The pitch angle asked for, and the turn's pitch rate.
        let pitch = if ground < at.low_speed_aero {
            -stick * self.max_pitch
        } else {
            (self.speed_gain * error).max(-self.max_pitch).min(self.max_pitch)
        };
        let feed_forward = if self.min_feed_forward_speed < at.airspeed {
            let (cos, sin) = curve(wrap(wrap(at.roll)), table);
            let turn = F(G) * at.gravity_scale * sin / at.airspeed;
            let ff = -turn * (sin / cos);
            let (lo_speed, hi_speed) = (self.min_feed_forward_speed, self.max_feed_forward_speed);
            let (lo, hi) = (self.min_feed_forward_scale, self.max_feed_forward_scale);
            let scale = if lo_speed == hi_speed {
                lo
            } else {
                let t = (ground - lo_speed) / (hi_speed - lo_speed);
                Acc::adda(F(0.0), lo).madd(t, hi - lo).max(lo.min(hi)).min(lo.max(hi))
            };
            ff * scale
        } else {
            F(0.0)
        };

        // The three rate commands, then the errors against the body's.
        let q = (feed_forward + self.pitch_gain * (pitch - at.pitch)).max(-self.max_rate[0]).min(self.max_rate[0]);
        let y = -pad.pedal * self.max_rate[1];
        let r = -self.roll_gain * (-pad.lateral * self.max_roll - at.roll);
        let limit = if pad.lateral == 0.0 { self.min_rate[2] } else { self.max_rate[2] };
        let r = r.max(-limit).min(limit);
        let e = Vf::new(q.to_f32(), y.to_f32(), r.to_f32(), 0.0).sub(at.rates);
        out.lateral = (self.lateral_cyclic_gain * F(e.z())).max(-1.0).min(1.0);
        out.longitudinal = (self.longitudinal_cyclic_gain * F(e.x())).max(-1.0).min(1.0);
        out.pedal = (self.pedal_gain * F(e.y())).max(-1.0).min(1.0);

        // The collective.
        if at.pilot_flag && pad.climb == 0.0 {
            out.altitude_fraction = at.height / self.max_altitude;
            out.collective = F(0.0);
            return;
        }
        let (rate, target) = if pad.climb != 0.0 && at.height < self.max_altitude {
            state.hold_timer = self.hold_delay;
            state.integral = F(INTEGRAL_RESTART);
            (self.climb_rate, F(1.0))
        } else if pad.descend != 0.0 {
            state.hold_timer = self.hold_delay;
            state.integral = F(INTEGRAL_RESTART);
            (self.descent_rate, F(0.0))
        } else {
            if !(state.hold_timer <= 0.0) {
                state.hold_altitude = at.height;
                state.hold_timer = state.hold_timer - dt;
            }
            (self.altitude_hold_gain * (state.hold_altitude - at.height), F(HOLD_TARGET))
        };
        let e = rate - vy;
        let c = state.integral + self.collective_gain * e;
        out.collective = if !(c <= 1.0) {
            F(1.0)
        } else if c < self.min_collective {
            self.min_collective
        } else {
            state.integral = (state.integral + dt * (self.integrator_gain * e)).max(self.min_collective).min(1.0);
            c
        };
        out.filtered = state.filter.step(target, dt);
    }
}
