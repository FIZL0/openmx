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
//!   snaps to the ground speed the frame the stick comes back to centre. Below `HighSpeedAero` the
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

use crate::biplane::Body;
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
    /// The aerodynamics' `HighSpeedAero` ([`Aero::high_speed_aero`]): below this ground speed the stick
    /// asks for pitch directly.
    pub high_speed_aero: F,
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
        let pitch = if ground < at.high_speed_aero {
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

// --- the rotor -------------------------------------------------------------------------------------

/// Thrust per blade, per unit of everything else: `rho / 4` at sea level, as the original writes it.
const BLADE_THRUST: f32 = f32::from_bits(0x3a1b_c5c4);
/// The share of the blade's speed that the thrust's pitch term takes.
const BLADE_PITCH: f32 = f32::from_bits(0x3f2a_c083);

/// `{cos, sin}` of half an angle, through two wraps and the curve table.
fn half(angle: F, table: &CurveTable) -> (F, F) {
    curve(wrap(wrap(F(0.5) * angle)), table)
}

/// The rotation by three angles (`x` first, then `y`, then `z`) as a quaternion `(x, y, z, w)`: each
/// half angle's cosine and sine off the curve table, multiplied out on the FPU's accumulator.
pub fn euler_quat(angles: [F; 3], table: &CurveTable) -> Vf {
    let (c1, s1) = half(angles[0], table);
    let (c2, s2) = half(angles[1], table);
    let (c3, s3) = half(angles[2], table);
    let (cc, ss) = (c2 * c3, s2 * s3);
    let w = Acc::mula(c1, cc).madd(s1, ss);
    let x = Acc::mula(s1, cc).madd(c1, ss);
    let y = Acc::mula(s3, -s1 * c2).madd(c3, c1 * s2);
    let z = Acc::mula(s3, c1 * c2).msub(c3, s1 * s2);
    Vf::new(x.to_f32(), y.to_f32(), z.to_f32(), w.to_f32())
}

/// The `[Main Rotor]` section, with `[Tail Rotor]`'s pitch angle and `MaxRotorMoment`. Angles are in
/// radians (the file's degrees times the reader's 0.01745329).
///
/// The reader also keeps the main rotor's `Inertia`, `Position`, `MinAltitude` and `MaxAltitude` and
/// the tail rotor's own blade figures; the step reads none of them. The PC exe's step reads no
/// altitude either, so there is no altitude fade in the rotor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotor {
    /// `MaxLateralCyclicPitchAngle`: the disc's roll tilt at full lateral cyclic.
    pub max_lateral_cyclic: F,
    /// `MaxLongitudinalCyclicPitchAngle`: the disc's pitch tilt at full longitudinal cyclic.
    pub max_longitudinal_cyclic: F,
    /// `MaxCollectivePitchAngle`.
    pub max_collective: F,
    /// `[Tail Rotor] MaxPitchAngle`: the tail blades' pitch at full pedal.
    pub max_tail_pitch: F,
    /// `LiftCurveSlope`.
    pub lift_curve_slope: F,
    /// `BladeChord`.
    pub blade_chord: F,
    /// `NumberOfBlades`.
    pub blades: F,
    /// `Radius`.
    pub radius: F,
    /// `Radius` squared, as the reader keeps it.
    pub radius_squared: F,
    /// `AngularRate`.
    pub angular_rate: F,
    /// `MaxRotorMoment`: the body-frame moment at full longitudinal cyclic, pedal and lateral cyclic.
    pub max_moment: [F; 3],
}

/// What the rotor keeps from its last step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RotorState {
    /// The main blades' pitch.
    pub collective_pitch: F,
    /// The tail blades' pitch.
    pub tail_pitch: F,
    /// The disc's tilt: longitudinal (`x`), nothing (`y`, which the step never writes) and lateral (`z`).
    pub tilt: [F; 3],
    /// The thrust along the shaft: only `y` is written; `x`, `z` and `w` stay as they were made (0, 0, 1).
    pub thrust: Vf,
    /// The thrust turned by the disc's tilt: the body-frame force the step hands the body.
    pub force: Vf,
}

impl Rotor {
    /// One step of the rotor: the controller's commands into blade pitches and the disc's tilt, the
    /// thrust from the collective, tilted. Returns the body-frame force and moment it hands the body,
    /// in that order. The moment is `MaxRotorMoment` times the cyclic and the pedal; its `w` is stack
    /// garbage in the original, and nothing reads it.
    ///
    /// The original also works the hub's velocity (the body's rates crossed with the hub's position,
    /// plus the body-frame velocity) and turns it by the tilt, then never reads the result.
    pub fn step(&self, c: &Commands, s: &mut RotorState, table: &CurveTable) -> (Vf, Vf) {
        s.tilt[2] = c.lateral * self.max_lateral_cyclic;
        s.tilt[0] = c.longitudinal * self.max_longitudinal_cyclic;
        s.collective_pitch = c.collective * self.max_collective;
        s.tail_pitch = c.pedal * self.max_tail_pitch;
        let q = euler_quat(s.tilt, table);

        let k = self.lift_curve_slope * (F(BLADE_THRUST) * self.blades);
        let pitch = F(BLADE_PITCH) * self.angular_rate * self.radius;
        let k = self.radius_squared * (self.angular_rate * (self.blade_chord * k));
        let thrust = k * ((F::ZERO + s.collective_pitch * pitch) - F::ZERO);
        s.thrust = Vf::new(s.thrust.x(), thrust.to_f32(), s.thrust.z(), s.thrust.w());
        s.force = crate::chassis::turn(q, s.thrust);

        let [mx, my, mz] = self.max_moment;
        let moment = Vf::new((c.longitudinal * mx).to_f32(), (c.pedal * my).to_f32(), (c.lateral * mz).to_f32(), 0.0);
        (s.force, moment)
    }
}

// --- the aerodynamics --------------------------------------------------------------------------------

/// The sideslip the coefficients see is held to 15 degrees either way.
const MAX_BETA: f32 = f32::from_bits(0x3e86_0a91);
/// The longitudinal stick past which the slow-speed drag is left off (forward) or scaled by the
/// angle of attack (back).
const DRAG_STICK: f32 = f32::from_bits(0x3e99_999a);

/// The `[Aerodynamics]` section, as far as the step reads it. Speeds are in feet per second (the
/// file's miles an hour times the reader's 1.46666).
///
/// The reader also keeps `AspectRatio`, `DefaultAngleOfAttack`, the `CL` shape keys, `CLq`, `CD1`,
/// `CYp`, `CYr`, `Clr`, `Cnp`, the three `Pedal` keys, `DragHighSpeed`, `HighSpeedDrag` and the
/// turbulence; the step reads none of them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aero {
    /// The air's density (slugs per cubic foot); the model's own, not the file's.
    pub density: F,
    /// `Chord`: the pitch moment's length, and the pitch rate's damping.
    pub chord: F,
    /// `Span`: the roll and yaw moments' length, and the roll rate's damping.
    pub span: F,
    /// `SurfaceArea`.
    pub area: F,
    /// `CLalpha`.
    pub cl_alpha: F,
    /// `CD0`.
    pub cd0: F,
    /// `CYbeta`.
    pub cy_beta: F,
    /// `Clbeta`, `Clp`, `Clda`.
    pub cl_beta: F,
    pub cl_p: F,
    pub cl_da: F,
    /// `CM0`, `CMalpha`, `CMq`, `CMde`.
    pub cm0: F,
    pub cm_alpha: F,
    pub cm_q: F,
    pub cm_de: F,
    /// `Cnbeta`, `Cnr`, `Cndr`.
    pub cn_beta: F,
    pub cn_r: F,
    pub cn_dr: F,
    /// `LowSpeedDrag`: the drag on the level velocity while slow.
    pub low_speed_drag: F,
    /// `LowSpeedAero` and `HighSpeedAero`: the ground speeds over which the aerodynamics fade in.
    pub low_speed_aero: F,
    pub high_speed_aero: F,
}

/// The coefficients the step leaves behind.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AeroState {
    /// How far between `LowSpeedAero` and `HighSpeedAero` the ground speed is, 0..1.
    pub fade: F,
    pub lift: F,
    pub drag: F,
    pub side: F,
    pub roll: F,
    pub pitch: F,
    pub yaw: F,
}

/// What the aerodynamics read off the helicopter: the measure's last airflow and the body.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Airflow {
    /// The speed, the angle of attack and the sideslip, as the measure left them.
    pub speed: F,
    pub alpha: F,
    pub beta: F,
    /// The body's world velocity, orientation and body-frame rates.
    pub velocity: Vf,
    pub quat: Vf,
    pub rates: Vf,
}

impl Aero {
    /// One step of the body's aerodynamics. Returns the body-frame force and moment it hands the body.
    ///
    /// Everything is dynamic pressure times area times the fade times a coefficient: side force
    /// `CYbeta·beta`, lift `CLalpha·alpha` and drag `CD0` against the direction of travel; the roll and
    /// pitch moments have their rate damping scaled by span or chord over twice the speed (none at or
    /// under 1 ft/s), the yaw moment's is not scaled at all, and each moment takes the raw stick or
    /// rudder (not the controller's commands). Below `HighSpeedAero`, with the stick not pushed
    /// forward, the level velocity in the body's frame is dragged by `LowSpeedDrag`; at any speed, with
    /// the stick pulled back, so is it, scaled by the angle of attack.
    pub fn step(&self, pad: &Pad, air: &Airflow, s: &mut AeroState) -> (Vf, Vf) {
        use crate::contact::{minus, scaled, unturn};
        let beta = air.beta.max(F(-MAX_BETA)).min(F(MAX_BETA));
        let v = air.velocity;
        let ground = (F(v.x()) * F(v.x()) + F(v.z()) * F(v.z())).sqrt();
        let speed = air.speed.abs();
        let level = unturn(air.quat, Vf::new(v.x(), 0.0, v.z(), v.w()));

        s.fade = if self.low_speed_aero == self.high_speed_aero {
            F::ZERO
        } else {
            let t = (ground - self.low_speed_aero) / (self.high_speed_aero - self.low_speed_aero);
            Acc::adda(F::ZERO, F::ZERO).madd(t, F::ONE - F::ZERO).max(F::ZERO).min(F::ONE)
        };
        let q = self.area * (speed * (F(0.5) * self.density * speed));
        let (pitch_k, roll_k) = if speed <= 1.0 {
            (F::ZERO, F::ZERO)
        } else {
            let h = F(0.5) / speed;
            (self.chord * h, self.span * h)
        };
        let rates = air.rates;
        s.lift = self.cl_alpha * air.alpha;
        s.drag = self.cd0;
        s.roll = Acc::adda(self.cl_beta * beta, roll_k * (self.cl_p * F(rates.z()))).madd(self.cl_da, pad.lateral);
        s.pitch = Acc::adda(self.cm0 + self.cm_alpha * air.alpha, pitch_k * (self.cm_q * F(rates.x())))
            .madd(self.cm_de, pad.longitudinal);
        s.side = self.cy_beta * beta;
        s.yaw = Acc::adda(self.cn_beta * beta, self.cn_r * F(rates.y())).madd(self.cn_dr, pad.pedal);

        let sign = if air.speed < F::ZERO { F(-1.0) } else { F::ONE };
        let mut force = Vf::new(
            (q * (s.fade * s.side)).to_f32(),
            (q * (s.fade * s.lift)).to_f32(),
            (q * (s.drag * (-s.fade * sign))).to_f32(),
            1.0,
        );
        let moment = Vf::new(
            (self.chord * (q * (s.fade * s.pitch))).to_f32(),
            (self.span * (q * (s.fade * s.yaw))).to_f32(),
            (self.span * (q * (s.fade * s.roll))).to_f32(),
            1.0,
        );
        let drag = || scaled(level, self.low_speed_drag * (F(0.5) * self.density * self.area));
        if speed < self.high_speed_aero && pad.longitudinal < DRAG_STICK {
            force = minus(force, drag());
        } else if pad.longitudinal <= -DRAG_STICK {
            force = minus(force, scaled(drag(), air.alpha.abs()));
        }
        (force, moment)
    }
}

// --- the listeners -----------------------------------------------------------------------------------

/// What the controller and the aerodynamics read off the helicopter besides the body: the measure's
/// last figures and the two flags.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Readings {
    /// See [`Flight::flown`] and [`Flight::pilot_flag`].
    pub flown: bool,
    pub pilot_flag: bool,
    /// The forward speed in the body's frame ([`Flight::airspeed`]), the pitch and the roll.
    pub airspeed: F,
    pub pitch: F,
    pub roll: F,
    /// See [`Flight::gravity_scale`].
    pub gravity_scale: F,
    /// The speed, the angle of attack and the sideslip ([`Airflow`]).
    pub speed: F,
    pub alpha: F,
    pub beta: F,
}

/// The helicopter's flight: the controller, the rotor and the aerodynamics with their state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kiowa {
    pub controller: Controller,
    pub rotor: Rotor,
    pub aero: Aero,
    pub pad: Pad,
    pub commands: Commands,
    pub control: Loop,
    pub rotor_state: RotorState,
    pub aero_state: AeroState,
    /// Set by the wreck: while it is, the controller, the rotor and the aerodynamics are skipped and
    /// only the body's own force runs.
    pub wrecked: bool,
}

impl Kiowa {
    /// **The preforce listener**, once a frame: the body wrapper's clear of the shock peak. The
    /// helicopter's own slot after it does nothing.
    pub fn preforce(&mut self, body: &mut Body) {
        body.clear_shock_peak();
    }

    /// **The force listener**, every substep with the substep's `h`: the controller, then the body
    /// wrapper's own force (gravity and the contact resolution, handed in as `wrapper`, as for the
    /// plane's [`crate::biplane::collision_force`]), then the rotor's and the aerodynamics' forces and
    /// moments onto the body, in that order. A wrecked helicopter runs only the wrapper.
    pub fn force(&mut self, r: &Readings, body: &mut Body, table: &CurveTable, h: F, wrapper: impl FnOnce(&mut Body)) {
        if !self.wrecked {
            let at = Flight {
                flown: r.flown,
                pilot_flag: r.pilot_flag,
                height: F(body.position.y()),
                velocity: [F(body.velocity.x()), F(body.velocity.y()), F(body.velocity.z())],
                rates: body.rates,
                airspeed: r.airspeed,
                pitch: r.pitch,
                roll: r.roll,
                gravity_scale: r.gravity_scale,
                high_speed_aero: self.aero.high_speed_aero,
            };
            self.controller.step(&self.pad, &at, &mut self.control, &mut self.commands, table, h);
        }
        wrapper(body);
        if self.wrecked {
            return;
        }
        let (f, m) = self.rotor.step(&self.commands, &mut self.rotor_state, table);
        body.add_force_local(f);
        body.add_moment_local(m);
        let air = Airflow {
            speed: r.speed,
            alpha: r.alpha,
            beta: r.beta,
            velocity: body.velocity,
            quat: body.quat,
            rates: body.rates,
        };
        let (f, m) = self.aero.step(&self.pad, &air, &mut self.aero_state);
        body.add_force_local(f);
        body.add_moment_local(m);
    }
}
