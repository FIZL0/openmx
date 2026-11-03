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
//! 2. [`Kiowa::force`] — every substep: the controller, the rotor and the aerodynamics onto the body.
//! 3. [`Measure::flight`] — once a frame after the world step: the speed, alpha, beta and the
//!    orientation the next frame's force reads ([`Measure::readings`]); then [`Crash::step`], the
//!    throw back from the map's edge and the wrecks; then [`Measure::touchdown`].
//!
//! Every field is named by the `.phy` key it is loaded from, where it has one (the `[Flight
//! Controller]` section of `helicopter\kiowa.phy`). [`Phy::read`] is the file's reader: it hands back
//! the [`Controller`], [`Rotor`] and [`Aero`] the steps take, and the wreck's figures. The steps'
//! proofs still read their values off a save state's live helicopter (`tests/kiowa.rs`).
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

use crate::air::AcosTable;
use crate::biplane::{Area, Body, CRASHED, EDGE, THROWN, flow_angle};
use crate::camera::CurveTable;
use crate::math::{Mat4, quat_to_euler, quat_to_mat4};
use crate::rng::Rng;
use ps2_float::fpu::{self, Acc};
use ps2_float::{Dest, F, Field, Vf};

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
    /// The stick left (+) and right (-): bank.
    pub lateral: F,
    /// The stick forward (+) and back (-): ground speed.
    pub longitudinal: F,
    /// The rudder buttons: yaw rate.
    pub pedal: F,
}

/// The helicopter's commands in the binding table, numbered as the table numbers them. The d-pad's
/// four are the bike's and the biplane's ([`crate::bindings::Command::LEFT`] and on).
pub mod command {
    use crate::bindings::Command;
    /// Cross as shipped.
    pub const CLIMB: Command = Command(0x1a);
    /// Square as shipped.
    pub const DESCEND: Command = Command(0x1b);
    /// L1 as shipped: the pedal +1.
    pub const PEDAL_LEFT: Command = Command(0x1c);
    /// R1 as shipped: the pedal -1.
    pub const PEDAL_RIGHT: Command = Command(0x1d);
}

impl Pad {
    /// **The input step**, the helicopter's slot the player's per-tick input calls, given the player's
    /// two stick values and a command's value ([`crate::bindings::Table::value`]).
    ///
    /// In the original's order: climb and descend are their commands' values as they are (so a
    /// pressure, not a switch); the stick's `x` and `y`, negated and not shaped (the biplane cubes
    /// them), are the lateral and the longitudinal, so a centred stick is `-0.0`; the pedal is +1 while
    /// [`command::PEDAL_LEFT`] is held, else -1 while [`command::PEDAL_RIGHT`] is, else 0; then the
    /// d-pad overrides the stick, Left +1 / Right -1 on the lateral and Up +1 / Down -1 on the
    /// longitudinal, the first of each pair winning. "Held" is the original's test, a value not 0.0.
    pub fn from_sticks(stick: (F, F), value: impl Fn(crate::bindings::Command) -> F) -> Pad {
        use crate::bindings::Command;
        let held = |c: Command| value(c) != F::ZERO;
        let (x, y) = stick;
        let mut pad = Pad {
            climb: value(command::CLIMB),
            descend: value(command::DESCEND),
            lateral: -x,
            longitudinal: -y,
            pedal: F::ZERO,
        };
        if held(command::PEDAL_LEFT) {
            pad.pedal = F::ONE;
        } else if held(command::PEDAL_RIGHT) {
            pad.pedal = -F::ONE;
        }
        if held(Command::LEFT) {
            pad.lateral = F::ONE;
        } else if held(Command::RIGHT) {
            pad.lateral = -F::ONE;
        }
        if held(Command::UP) {
            pad.longitudinal = F::ONE;
        } else if held(Command::DOWN) {
            pad.longitudinal = -F::ONE;
        }
        pad
    }

    /// The input step on one pad through the shipped bindings, as the biplane's
    /// ([`crate::input::PlaneInput::from_pad`]): the left stick through
    /// [`crate::input::AxisRange::PLAYER`], each command its first binding's strength.
    pub fn from_pad(pad: &crate::input::Pad) -> Pad {
        use crate::input::{Axis, AxisRange};
        let stick =
            (pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), pad.axis_through(Axis::LeftY, &AxisRange::PLAYER));
        let table = &crate::bindings::Table::SHIPPED;
        Pad::from_sticks(stick, |c| table.value(pad, c))
    }
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
    /// The collision record's touching flag (S148 took it for a pilot's): while climb is not held and
    /// the helicopter is on something, the collective is left at zero. In the live state it is set.
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

// --- the measure ------------------------------------------------------------------------------------

/// Beta is held to `-pi..pi`, and a sideslip that flips sign behind the beam (past `pi/2`) by less
/// than this keeps last frame's value, so the tail-on wrap from `pi` to `-pi` does not jump.
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
const BETA_FLIP: f32 = f32::from_bits(0x3e32_b8c1);

/// What the measure listener works out off the body once a frame, before the crash test and for the
/// next frame's force listener ([`Measure::readings`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// `|velocity|`.
    pub speed: F,
    /// The angle of attack and the sideslip, radians, from the body-frame velocity.
    pub alpha: F,
    pub beta: F,
    /// The rates turned into the world.
    pub world_rates: Vf,
    /// The velocity in the body's frame: `z` is the speed along the nose.
    pub velocity: Vf,
    /// The orientation matrix's forward and up rows (`xyz`; `w` is left as it was).
    pub forward: Vf,
    pub up: Vf,
    /// The orientation as three angles (pitch `x`, roll `z`).
    pub euler: Vf,
    /// The orientation and the model's origin as a matrix.
    pub matrix: Mat4,
    /// The collision record's touching flag as the last measure left it.
    pub was_touching: bool,
}

impl Default for Measure {
    fn default() -> Measure {
        let zero = Vf::default();
        Measure {
            speed: F::ZERO,
            alpha: F::ZERO,
            beta: F::ZERO,
            world_rates: zero,
            velocity: zero,
            forward: zero,
            up: zero,
            euler: zero,
            matrix: Mat4([zero; 4]),
            was_touching: false,
        }
    }
}

/// One entry of the collision record, as the touchdown reads it: its first word (only a 0 counts) and
/// the float at `+0x24` (a contact counts below 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Touch {
    pub kind: u32,
    pub t: F,
}

impl Measure {
    /// **The measure listener**'s figures, after the body wrapper's own: the speed, the body-frame
    /// velocity and the world rates, alpha (the velocity's `y` against `z`) and beta (`x` against `z`)
    /// as the plane's flow angle, the Euler angles, and the matrix with the forward and up rows
    /// copied out, its origin the centre of mass less the turned [`Body::cofg`]. All of it is the
    /// plane's measure but beta, which is clamped to `-pi..pi` and kept at last frame's value when it
    /// changes sign past `pi/2` by under 10 degrees in size.
    ///
    /// The original then runs the crash test ([`Crash::step`]) and the touchdown
    /// ([`Measure::touchdown`]).
    pub fn flight(&mut self, body: &Body, table: &AcosTable) {
        use crate::chassis::turn;
        use crate::contact::unturn;
        let v = body.velocity;
        let (x, y, z) = (F(v.x()), F(v.y()), F(v.z()));
        self.speed = Acc::mula(x, x).madda(y, y).madd(z, z).sqrt();
        self.velocity = unturn(body.quat, v);
        self.world_rates = turn(body.quat, body.rates);
        let old = self.beta;
        let along = F(self.velocity.z());
        self.alpha = flow_angle(along, F(self.velocity.y()), table);
        let beta = flow_angle(along, F(self.velocity.x()), table).max(F(-PI)).min(F(PI));
        let sign = |b: F| if b < 0.0 { -1 } else { 1 };
        let flipped = F::from(sign(old) * sign(beta)) < 0.0;
        self.beta = if flipped && !(beta.abs() <= HALF_PI) && (beta.abs() - old.abs()).abs() < BETA_FLIP {
            old
        } else {
            beta
        };
        self.euler = quat_to_euler(body.quat, table);
        let p = body.position;
        let origin = p.merge(Dest::XYZ, p.sub(turn(body.quat, body.cofg)));
        self.matrix = quat_to_mat4(body.quat, origin);
        self.forward = self.forward.merge(Dest::XYZ, self.matrix.0[2]);
        self.up = self.up.merge(Dest::XYZ, self.matrix.0[1]);
    }

    /// The measure's last step: whether the helicopter has just come down on something — the
    /// collision record touching now but not at the last measure, with an entry of kind 0 below 1 —
    /// which is when the original plays its touchdown sound (if the helicopter has one). The flag is
    /// then remembered.
    pub fn touchdown(&mut self, touching: bool, entries: impl IntoIterator<Item = Touch>) -> bool {
        let down = !self.was_touching && touching && entries.into_iter().any(|e| e.kind == 0 && e.t < 1.0);
        self.was_touching = touching;
        down
    }

    /// What the force listener reads ([`Kiowa::force`]): this measure's figures, with the two flags
    /// and the gravity scale from the helicopter. `touching` is the collision record's flag, the one
    /// S148 called the pilot's.
    pub fn readings(&self, flown: bool, touching: bool, gravity_scale: F) -> Readings {
        Readings {
            flown,
            pilot_flag: touching,
            airspeed: F(self.velocity.z()),
            pitch: F(self.euler.x()),
            roll: F(self.euler.z()),
            gravity_scale,
            speed: self.speed,
            alpha: self.alpha,
            beta: self.beta,
        }
    }
}

// --- the crash test ---------------------------------------------------------------------------------

/// The throw back from the edge: as the plane's, on the helicopter's own constants — a fall of this
/// many feet sets its time, the speed toward the centre is capped here, and the upward part is
/// `16.1*k + 150/k` for `k = 2*sqrt(fall)`. The spins are random fractions of these.
const THROW_FALL: f32 = f32::from_bits(0x4115_117d);
const THROW_CAP: f32 = f32::from_bits(0x44e6_6667);
const THROW_UP: f32 = f32::from_bits(0x4180_cccd);
const THROW_LIFT: f32 = 150.0;
const THROW_SPIN: [f32; 3] = [0.5, f32::from_bits(0x3ecc_cccd), 0.5];
/// How long a thrown helicopter flies before it may count as down, and what is left of its velocity
/// when it is put back.
const THROWN_FOR: f32 = 2.5;
const RECOVER_KEEP: f32 = f32::from_bits(0x3e99_999a);

/// What the crash test reads off the helicopter besides the body.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CrashContact {
    /// The collision record's touching flag.
    pub touching: bool,
    /// The record's surface is one the helicopter's crash mask ignores for the speed test.
    pub ignored: bool,
    /// The measure's `|velocity|`.
    pub speed: F,
}

/// What the rotor's own collision object finds, asked only of a helicopter still flying.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Strike {
    /// A rotor sphere's centre is under the ground ([`rotor_in_ground`]).
    pub in_ground: bool,
    /// The rotor's contact search struck something its mask does not ignore.
    pub struck: bool,
}

/// The helicopter's crash state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Crash {
    /// 0 flying, [`CRASHED`] or [`THROWN`].
    pub reason: u32,
    /// A thrown helicopter has come down.
    pub down: bool,
    /// `MinWreckSpeed`, and `WreckTime`.
    pub min_wreck_speed: F,
    pub wreck_time: F,
    /// Time since the crash.
    pub clock: F,
}

/// What a crash-test call asked the helicopter to do: wreck itself, throw its blades (on coming down
/// after a throw), and put itself back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crashed {
    pub wreck: Option<u32>,
    pub debris: bool,
    pub recover: bool,
}

impl Crash {
    /// The wreck's own bookkeeping, the part this module needs: the reason, and not down. The clock
    /// runs on from where the last put-back left it. The rest (the body's damping, inertia and gravity,
    /// the sound, the model) is the caller's.
    fn wreck(&mut self, reason: u32, out: &mut Crashed) {
        self.reason = reason;
        self.down = false;
        out.wreck = Some(reason);
    }

    /// **The crash test**, the measure's wreck slot, once a frame after the measure's figures.
    ///
    /// Within [`EDGE`] of the area's edge, a helicopter not already thrown is thrown back as the plane
    /// is ([`crate::biplane::Crash::step`]): toward the centre at its own height, up, the scale at
    /// `throw_scale` halved, three random spins, and the [`THROWN`] wreck. A helicopter still flying
    /// then asks `strike` and is [`CRASHED`] by a rotor in the ground, a struck rotor, or a contact
    /// faster than `MinWreckSpeed` on a surface that counts (no upside-down test, unlike the plane).
    /// A wrecked helicopter's clock runs: a crashed one is put back after `WreckTime`; a thrown one
    /// first has to have flown 2.5 s and be touching, when it throws its blades and its clock restarts,
    /// and after `WreckTime` keeps 0.3 of its velocity and is put back.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        c: &CrashContact,
        area: Option<&Area>,
        position: Vf,
        velocity: &mut Vf,
        rates: &mut Vf,
        throw_scale: &mut F,
        rng: &mut Rng,
        dt: F,
        strike: impl FnOnce() -> Strike,
    ) -> Crashed {
        use crate::contact::{dot, minus, scaled};
        let mut out = Crashed::default();
        if let Some(area) = area.filter(|a| self.reason != THROWN && a.outside(position, F(EDGE))) {
            let half_width = F(0.5) * area.width;
            let target = Vf::new(half_width.to_f32(), position.y(), (F(0.5) * area.depth).to_f32(), 1.0);
            let mut toward = minus(target, position);
            let length = dot(toward, toward);
            if F::ZERO == length {
                toward = Vf::VF0;
            } else {
                let r = F::ONE.rsqrt(length);
                toward = Vf::new((F(toward.x()) * r).to_f32(), (F(toward.y()) * r).to_f32(), (F(toward.z()) * r).to_f32(), toward.w());
            }
            let k = F(2.0) * F(THROW_FALL).sqrt();
            let speed = F(THROW_CAP).min(half_width) / k;
            let lift = F(THROW_UP) * k + F(THROW_LIFT) / k;
            *velocity = scaled(toward, speed);
            velocity[1] = (F(velocity.y()) + lift).to_f32();
            *throw_scale = *throw_scale * 0.5;
            for (lane, spin) in THROW_SPIN.into_iter().enumerate() {
                rates[lane] = (F(spin) * rng.unit()).to_f32();
            }
            rates[3] = 1.0;
            self.wreck(THROWN, &mut out);
        }
        if self.reason == 0 {
            let s = strike();
            if s.in_ground || s.struck || (c.touching && !c.ignored && !(c.speed <= self.min_wreck_speed)) {
                self.wreck(CRASHED, &mut out);
            }
            return out;
        }
        self.clock = self.clock + dt;
        if self.reason == THROWN {
            if !self.down && !(self.clock <= THROWN_FOR) && c.touching {
                out.debris = true;
                self.down = true;
                self.clock = F::ZERO;
            }
            if self.down && !(self.clock <= self.wreck_time) {
                *velocity = scaled(*velocity, F(RECOVER_KEEP));
                out.recover = true;
            }
        } else if self.reason == CRASHED && !(self.clock <= self.wreck_time) {
            out.recover = true;
        }
        out
    }
}

/// Whether any of the rotor's collision spheres has its centre under the ground: each centre (in the
/// model's frame) through `matrix` — the translation row first, then `x`, `y` and `z` accumulated in
/// that order, `w` the sphere's — handed to `ground`, which gives the point back with the ground's
/// answer in it; a returned `y` not at or below the centre's is in the ground. Every sphere is asked.
pub fn rotor_in_ground(matrix: &Mat4, centres: &[Vf], mut ground: impl FnMut(Vf) -> Vf) -> bool {
    let m = &matrix.0;
    let mut hit = false;
    for &c in centres {
        let acc = m[3].add(Vf::VF0.bc(Field::X));
        let acc = Vf::madd(acc, m[0], c.bc(Field::X));
        let acc = Vf::madd(acc, m[1], c.bc(Field::Y));
        let p = c.merge(Dest::XYZ, Vf::madd(acc, m[2], c.bc(Field::Z)));
        if !(ground(p).y() <= p.y()) {
            hit = true;
        }
    }
    hit
}

// --- the MX helicopter's own measure -----------------------------------------------------------------

/// The main rotor's turn per second, radians.
const BLADE_RATE: f32 = 27.0;

/// The MX helicopter's first addition to the measure: the rotor's angle (which the main and tail
/// rotors' model nodes are then turned by) runs on at 27 radians a second, except while the rotor's node is
/// hidden (after the blades are thrown). Its other three additions only copy figures into the
/// presentation's records and are not ported.
pub fn spin_blades(angle: &mut F, hidden: bool, dt: F) {
    if !hidden {
        *angle = *angle + F(BLADE_RATE) * dt;
    }
}

// --- the frame ---------------------------------------------------------------------------------------

/// What the helicopter's frame reads and leaves besides [`Kiowa`]'s own: its body, the measure, the
/// crash state and the few words of the helicopter the listeners read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ride {
    pub body: Body,
    pub measure: Measure,
    pub crash: Crash,
    /// The player is flying it (the controller's flown flag).
    pub flown: bool,
    /// The scale the controller's climb loop reads, and the one the edge throw halves (the put-back
    /// doubles it back).
    pub gravity_scale: F,
    pub throw_scale: F,
    /// The body wrapper's gravity, the world-frame force it pushes every substep while the body is
    /// enabled (`(0, -32.2 m gravity_scale, 0)` as the helicopter builds it).
    pub gravity: Vf,
    /// The terrain's area the edge throw keeps it inside, if the scene has a terrain.
    pub area: Option<Area>,
    /// The helicopter has a touchdown sound with clips in its list: its pick draws one `rand()`.
    pub touchdown_sound: bool,
    /// The MX helicopter's rotor angle, and whether its rotor's model node is hidden.
    pub rotor_angle: F,
    pub rotor_hidden: bool,
}

/// What the frame asks of the engine round the helicopter: the collision record, the rotor's own
/// collision object, and what a crash test's answer does to the machine. None of it is the
/// helicopter's own flight.
pub trait Surroundings {
    /// The body wrapper's post, first in the post listener, with the body just stepped: the
    /// collision object gathers its nearby list where the body now stands ([`Grounded`]). Nothing for
    /// surroundings with no collision object.
    fn post(&mut self, body: &Body) {
        let _ = body;
    }
    /// The body wrapper's collision response, every substep after gravity.
    fn resolve(&mut self, body: &mut Body);
    /// The collision record's touching flag.
    fn touching(&self) -> bool;
    /// The record's surface is one the helicopter's crash mask ignores for the speed test.
    fn ignored(&self) -> bool;
    /// The record's entries, for the touchdown.
    fn entries(&self) -> Vec<Touch>;
    /// The rotor's strike test, asked by the crash test of a helicopter still flying, with the
    /// measure's matrix just made ([`rotor_in_ground`] and the rotor's contact search).
    fn strike(&mut self, matrix: &Mat4) -> Strike;
    /// What the crash test asked for, at the point the original does it (the wreck, the blades
    /// thrown, the put-back): inside the measure, after the crash test and before the touchdown, so
    /// any `rand()` it draws comes before the touchdown sound's.
    fn crashed(&mut self, asked: &Crashed, ride: &mut Ride, rng: &mut Rng);
    /// The bodies that share the helicopter's world (C21): the MX helicopter's thrown blades, which
    /// step with it, after it on every list. None by default: the helicopter alone in its world.
    fn among(&mut self) -> Option<Among<'_>> {
        None
    }
    /// How many of [`Surroundings::among`]'s are on `list`, as the world's step counts them.
    fn among_listed(&self, list: crate::biplane::List) -> i32 {
        let _ = list;
        0
    }
}

/// **The thrown blades in the helicopter's world** (C21), as its step reaches them: their
/// collection's lists, the tables their springs read and the movers their contacts name, and the
/// helicopter's own collision object and mass for the blades' contacts on its hull.
pub struct Among<'a> {
    pub blades: &'a mut Collection,
    pub tables: &'a Tables,
    pub movers: &'a [Mover],
    pub hull: &'a mut crate::contact::Collider,
    pub mass: F,
}

/// **The helicopter's lists with the blades after it** (C21): the original's world holds the
/// helicopter's body and listeners first in every list and the blades' after them, so each substep
/// pushes the helicopter, then the blades, then integrates the helicopter, then the blades. The
/// blades' posts are not on it: they run after the crash test's answer is acted on
/// ([`Blades::post`]).
struct Shared<'a, S: Surroundings> {
    lists: Lists<'a, S>,
}

impl<S: Surroundings> Shared<'_, S> {
    /// The blades' turn at `list`'s `i`th place (counted after the helicopter's), with the
    /// helicopter as the only other side their contacts name.
    fn blades(&mut self, f: impl FnOnce(&mut dyn crate::biplane::Dynamics)) {
        let Lists { ride, around, .. } = &mut self.lists;
        let Some(a) = around.among() else { return };
        let mut heli = crate::contact::Weighed { body: &mut ride.body, mass: a.mass };
        let mut others = [crate::contact::Other { collider: a.hull, side: crate::contact::Side::Body(&mut heli) }];
        let mut lists = a.blades.lists(a.tables, a.movers, &mut others);
        f(&mut lists);
    }
}

impl<S: Surroundings> crate::biplane::Dynamics for Shared<'_, S> {
    fn count(&self, list: crate::biplane::List) -> i32 {
        let blades = match list {
            crate::biplane::List::Post => 0,
            _ => self.lists.around.among_listed(list),
        };
        self.lists.count(list) + blades
    }
    fn preforce(&mut self, i: i32, dt: F) {
        if i == 0 { self.lists.preforce(0, dt) } else { self.blades(|d| d.preforce(i - 1, dt)) }
    }
    fn force(&mut self, i: i32, h: F) {
        if i == 0 { self.lists.force(0, h) } else { self.blades(|d| d.force(i - 1, h)) }
    }
    fn enabled(&self, i: i32) -> bool {
        if i == 0 {
            return self.lists.enabled(0);
        }
        // `among` needs the surroundings mutably: a blade's own flag is read in `integrate` and
        // `end_substep` instead, which then do nothing for an asleep body, as the world's loop would.
        true
    }
    fn integrate(&mut self, i: i32, h: F) {
        if i == 0 { self.lists.integrate(0, h) } else { self.blades(|d| if d.enabled(i - 1) { d.integrate(i - 1, h) }) }
    }
    fn end_substep(&mut self, i: i32) {
        if i == 0 { self.lists.end_substep(0) } else { self.blades(|d| if d.enabled(i - 1) { d.end_substep(i - 1) }) }
    }
    fn constrain(&mut self, i: i32, first: bool) {
        self.blades(|d| d.constrain(i, first));
    }
    fn post(&mut self, i: i32, dt: F) {
        self.lists.post(i, dt);
    }
}

/// The helicopter's lists in the dynamics world: one body, one listener of each kind, no constraints.
struct Lists<'a, S: Surroundings> {
    kiowa: &'a mut Kiowa,
    ride: &'a mut Ride,
    around: &'a mut S,
    curve: &'a CurveTable,
    acos: &'a AcosTable,
    rng: &'a mut Rng,
    asked: Crashed,
}

impl<S: Surroundings> crate::biplane::Dynamics for Lists<'_, S> {
    fn count(&self, list: crate::biplane::List) -> i32 {
        match list {
            crate::biplane::List::Constraints => 0,
            _ => 1,
        }
    }
    fn preforce(&mut self, _: i32, _: F) {
        self.kiowa.preforce(&mut self.ride.body);
    }
    fn force(&mut self, _: i32, h: F) {
        let r = self.ride.measure.readings(self.ride.flown, self.around.touching(), self.ride.gravity_scale);
        self.kiowa.wrecked = self.ride.crash.reason != 0;
        let (gravity, around) = (self.ride.gravity, &mut *self.around);
        self.kiowa.force(&r, &mut self.ride.body, self.curve, h, |b| {
            crate::biplane::collision_force(b, gravity, |b| around.resolve(b));
        });
    }
    fn enabled(&self, _: i32) -> bool {
        self.ride.body.enabled
    }
    fn integrate(&mut self, _: i32, h: F) {
        self.ride.body.integrate(h);
    }
    fn end_substep(&mut self, _: i32) {
        self.ride.body.end_substep();
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, _: i32, dt: F) {
        let ride = &mut *self.ride;
        self.around.post(&ride.body);
        ride.measure.flight(&ride.body, self.acos);
        let touching = self.around.touching();
        let c = CrashContact { touching, ignored: self.around.ignored(), speed: ride.measure.speed };
        let (around, matrix) = (&mut *self.around, ride.measure.matrix);
        let b = &mut ride.body;
        self.asked = ride.crash.step(&c, ride.area.as_ref(), b.position, &mut b.velocity, &mut b.rates, &mut ride.throw_scale, self.rng, dt, || {
            around.strike(&matrix)
        });
        self.around.crashed(&self.asked, ride, self.rng);
        let entries = self.around.entries();
        if ride.measure.touchdown(touching, entries) && ride.touchdown_sound {
            self.rng.rand();
        }
        spin_blades(&mut ride.rotor_angle, ride.rotor_hidden, dt);
    }
}

impl Kiowa {
    /// **One frame of the helicopter**, as the original runs it: the input step, when the player has
    /// the helicopter (`pad`, [`Pad::from_sticks`]), then the engine's dynamics world
    /// ([`crate::biplane::World::step`]) with the helicopter's three listeners on it —
    ///
    /// 1. *preforce*, once: [`Kiowa::preforce`];
    /// 2. *force*, every substep: [`Kiowa::force`] on the measure's last figures
    ///    ([`Measure::readings`]), the body wrapper's gravity and collision response between the
    ///    controller and the rotor, the whole skipped but the wrapper while the crash test has a
    ///    reason set;
    /// 3. the body integrated and emptied, every substep while it is enabled;
    /// 4. *post*, once: the MX measure — [`Measure::flight`], the crash test ([`Crash::step`]) and
    ///    what it asks for ([`Surroundings::crashed`]), the touchdown ([`Measure::touchdown`]; its sound's
    ///    pick draws one `rand()`), then the rotor's spin ([`spin_blades`]).
    ///
    /// Whatever shares the world ([`Surroundings::among`], the thrown blades) is after the helicopter
    /// on each of the lists but the posts.
    ///
    /// Returns what the crash test asked for.
    ///
    /// Against the original's frames (C14's capture), the world step flies the pad the last input
    /// step left: a replay passes `None` and sets [`Kiowa::pad`] after the step.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        ride: &mut Ride,
        world: &crate::biplane::World,
        pad: Option<Pad>,
        curve: &CurveTable,
        acos: &AcosTable,
        rng: &mut Rng,
        dt: F,
        around: &mut impl Surroundings,
    ) -> Crashed {
        if let Some(pad) = pad {
            self.pad = pad;
        }
        let lists = Lists { kiowa: self, ride, around, curve, acos, rng, asked: Crashed::default() };
        let mut shared = Shared { lists };
        world.step(dt, &mut shared);
        shared.lists.asked
    }
}

// --- the .phy reader ---------------------------------------------------------------------------------

use crate::biplane::Wrecks;
use crate::phy::{DEG, Ini, MPH, Part, Section};

/// The helicopter's file in `machine.res`.
pub const KIOWA_PHY: &str = "helicopter\\kiowa.phy";
/// The thrown blades' file, beside it: one `[PhysicsPart1]`, read into each of the four blades.
pub const WRECK_BLADE_PHY: &str = "helicopter\\wreckblade.phy";

/// What a quadword key the file leaves out reads as: the origin point.
const ORIGIN: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
/// The air's density, slugs per cubic foot: the aerodynamics model's constructor's, not a key.
const AIR_DENSITY: f32 = f32::from_bits(0x3b1b_c5c4);
/// The controller's hold delay ([`Controller::hold_delay`]): the helicopter's constructor's, not a key.
const HOLD_DELAY: f32 = 2.0;
/// The rotor's wreck rays: `WreckRotorRadius` when the key is missing.
const WRECK_ROTOR_RADIUS: f32 = 1.0;
/// The diagonal rays' share of the radius on each axis (0.707, not `sqrt(0.5)`).
const DIAGONAL: f32 = f32::from_bits(0x3f34_fdf4);

/// `helicopter\kiowa.phy`, as the helicopter's reader leaves it: the base helicopter's sections, then
/// the MX helicopter's `[Animation]`.
///
/// Every float the reader asks for has a default of zero and every quadword the origin point
/// (`WreckRotorRadius` alone defaults to 1). Angles are written in degrees and kept in radians
/// ([`DEG`]); speeds are written in miles an hour and kept in feet a second ([`MPH`]):
///
/// * `[Main Rotor]` the three cyclic and collective angles, `[Tail Rotor]` `MaxPitchAngle`;
/// * `[Aerodynamics]` `DefaultAngleOfAttack` in degrees; `DragHighSpeed`, `LowSpeedAero`,
///   `HighSpeedAero`, `TurbulenceMagnitude` and `SteadyWind`'s `x`, `y` and `z` in miles an hour (not
///   `LowSpeedDrag`, nor `HighSpeedDrag`);
/// * `[Flight Controller]` `MaxPitchCommand` and `MaxRollCommand` in degrees; `SpeedCommandRate`, the
///   two speed commands and the two feed-forward speeds in miles an hour (not the rate commands);
/// * `[Wrecks]` `MinWreckSpeed` in miles an hour.
///
/// `[Mass Properties]` is the shared rigid-body part's ([`Part::read`]): the file's `Inertia` key is
/// not one that reader takes, and the body's inertia comes from the model's box, as every part's does.
#[derive(Clone, Debug, PartialEq)]
pub struct Phy {
    pub mass_properties: Part,
    /// The two rotors, as the step reads them.
    pub rotor: Rotor,
    /// The rest of `[Main Rotor]` and `[Tail Rotor]`.
    pub main_rotor: MainRotor,
    pub tail_rotor: TailRotor,
    /// `[Aerodynamics]`, as the step reads it, and the rest.
    pub aero: Aero,
    pub aero_keys: AeroKeys,
    /// `[Flight Controller]`.
    pub controller: Controller,
    /// `[Wrecks]`: the five keys the plane's reader takes too, in the same order and units.
    pub wrecks: Wrecks,
    /// The rotor's wreck rays, from `WreckRotorRadius` and `WreckRotorHeight`.
    pub wreck_rotor: WreckRotor,
    /// `[Animation]`: the box the pilot's pelvis sits in.
    pub pelvis_box_position: Vf,
    pub pelvis_box_size: Vf,
}

/// The main rotor's keys the step does not read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MainRotor {
    /// `Inertia`.
    pub inertia: F,
    /// `Position`: the hub, in the body's frame.
    pub position: Vf,
    /// `MinAltitude`, `MaxAltitude` (the rotor's, not the controller's).
    pub min_altitude: F,
    pub max_altitude: F,
}

/// `[Tail Rotor]` but its `MaxPitchAngle` ([`Rotor::max_tail_pitch`]); the step reads none of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailRotor {
    pub lift_curve_slope: F,
    pub blade_chord: F,
    pub blades: F,
    pub radius: F,
    /// `Radius` squared, as the reader keeps it.
    pub radius_squared: F,
    pub gear_ratio: F,
    pub inertia: F,
    pub position: Vf,
}

/// `[Aerodynamics]`' keys the step does not read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AeroKeys {
    pub aspect_ratio: F,
    /// `DefaultAngleOfAttack`, radians.
    pub default_angle_of_attack: F,
    pub cl_round_off: F,
    pub cl_drop_off: F,
    pub cl_scale: F,
    pub cl_angle_scale: F,
    pub cl_q: F,
    pub cd1: F,
    pub cy_p: F,
    pub cy_r: F,
    pub cl_r: F,
    pub cn_p: F,
    pub cy_beta_pedal: F,
    pub cn_beta_pedal: F,
    pub cn_r_pedal: F,
    /// `DragHighSpeed`, feet a second.
    pub drag_high_speed: F,
    pub high_speed_drag: F,
    /// `TurbulenceMagnitude` (feet a second), `TurbulenceTimeScale`, `TurbulenceTimeOffset`.
    pub turbulence_magnitude: F,
    pub turbulence_time_scale: F,
    pub turbulence_time_offset: Vf,
    /// `SteadyWind`, feet a second (`w` as written).
    pub steady_wind: Vf,
}

/// The rotor's wreck collision: eight rays in the model's frame, each from the hub at
/// `(0, WreckRotorHeight, 0)` out to the disc's edge `WreckRotorRadius` away — along `+x`, `-x`, `+z`,
/// `-z`, then the four diagonals `(+,+)`, `(+,-)`, `(-,+)`, `(-,-)` (x, z) at 0.707 of the radius on
/// each axis. The crash test asks the ground under each ray's far end ([`rotor_in_ground`]). The
/// collision object keeps them in its tree's order, not this one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WreckRotor {
    /// `WreckRotorRadius`, `WreckRotorHeight`.
    pub radius: F,
    pub height: F,
    /// `(from, to)` per ray, in the reader's order.
    pub rays: [[Vf; 2]; 8],
}

impl WreckRotor {
    fn new(radius: F, height: F) -> WreckRotor {
        let hub = Vf::new(0.0, height.to_f32(), 0.0, 1.0);
        let at = |x: F, z: F| Vf::new(x.to_f32(), height.to_f32(), z.to_f32(), 1.0);
        let (zero, out, back) = (F::ZERO, radius, -radius);
        let (near, far) = (F(DIAGONAL) * radius, -F(DIAGONAL) * radius);
        let ends = [
            at(out, zero),
            at(back, zero),
            at(zero, out),
            at(zero, back),
            at(near, near),
            at(near, far),
            at(far, near),
            at(far, far),
        ];
        WreckRotor { radius, height, rays: ends.map(|end| [hub, end]) }
    }
}

// --- the rotor's collision object (C17) --------------------------------------------------------------

/// The rotor's collision object's category (`+0x84`) and the categories it never collides with (its
/// category list, mode 1), off C14's save state: not keys of the file.
pub const ROTOR_CATEGORY: u32 = 0x0008_0000;
pub const ROTOR_SPARED: [u32; 2] = [0x100, 0x0010_0000];
/// The crash test's struck mask (`+0x464`): what the rotor may strike and fly on. Off C14's save state.
pub const STRUCK_MASK: u32 = 0x0004_0000;

impl WreckRotor {
    /// **The rotor's collision object's ray probe** (C17): the eight rays built into a tree as the
    /// loader builds them ([`ray_tree`]), which leaves them in the tree's order, not the reader's.
    pub fn probe(&self) -> crate::flight_search::Rays {
        let segments: Vec<_> = self.rays.iter().map(|&[from, to]| crate::ray_probe::Segment { from, to }).collect();
        ray_tree(&segments)
    }
}

/// **A ray probe's tree, as the engine builds it off a list of segments** (C17): what the loaders hand
/// a probe's rays to (the rotor's, the bike's wheels', the car's sort rays). Each ray's centre is the
/// mean of its ends, on the vector unit. A list of one is a leaf. A longer one is a node boxed round
/// all its ends ([`crate::ride::ray_node`]), split where [`split`] says, the rays whose centre lies
/// above the split on its axis kept in order ahead of the rest, the two halves its children, the
/// first child first. The tree goes out depth first: a node's index is the order it is reached in, a
/// leaf's the order among leaves, which is the order the segments are kept in.
///
/// A split that leaves one side empty (no cut parts the centres: they tie, or lie within a twentieth
/// of the box of each other on every axis) the original answers with the halves of lists it never
/// wrote, which is no answer to port: here it is a panic ([`try_ray_tree`] gives `None`).
pub fn ray_tree(segments: &[crate::ray_probe::Segment]) -> crate::flight_search::Rays {
    try_ray_tree(segments).expect("a ray tree split that leaves a side empty")
}

/// [`ray_tree`], or `None` where a split leaves a side empty.
pub fn try_ray_tree(segments: &[crate::ray_probe::Segment]) -> Option<crate::flight_search::Rays> {
    enum Built {
        Leaf(crate::ray_probe::Segment),
        Node(crate::track::Node, Box<[Built; 2]>),
    }
    fn build(segments: &mut [crate::ray_probe::Segment], centres: &mut [Vf]) -> Option<Built> {
        if segments.len() == 1 {
            return Some(Built::Leaf(segments[0]));
        }
        let node = crate::ride::ray_node(&ends(segments));
        let (at, axis) = split(segments, centres);
        let (above, below) = partition(segments, centres, at, axis);
        if above.is_empty() || below.is_empty() {
            return None;
        }
        let a = above.len();
        for (k, (s, c)) in above.into_iter().chain(below).enumerate() {
            (segments[k], centres[k]) = (s, c);
        }
        let (first, second) = segments.split_at_mut(a);
        let (first_c, second_c) = centres.split_at_mut(a);
        Some(Built::Node(node, Box::new([build(first, first_c)?, build(second, second_c)?])))
    }
    fn flatten(b: &Built, nodes: &mut Vec<crate::track::Node>, out: &mut Vec<crate::ray_probe::Segment>) -> u16 {
        match b {
            Built::Leaf(s) => {
                out.push(*s);
                0x8000 | (out.len() - 1) as u16
            }
            Built::Node(node, children) => {
                let at = nodes.len();
                nodes.push(*node);
                let first = flatten(&children[0], nodes, out);
                let second = flatten(&children[1], nodes, out);
                nodes[at].children = [first, second];
                at as u16
            }
        }
    }
    let mut work = segments.to_vec();
    let half = Vf([0.5; 4]);
    let mut centres: Vec<Vf> = work.iter().map(|s| s.from.add(s.to).mul(half)).collect();
    let (mut nodes, mut out) = (Vec::new(), Vec::new());
    let root = if work.is_empty() { 0x8000 } else { flatten(&build(&mut work, &mut centres)?, &mut nodes, &mut out) };
    Some(crate::flight_search::Rays { root, nodes, segments: out })
}

/// Every ray's two ends, in order.
fn ends(segments: &[crate::ray_probe::Segment]) -> Vec<Vf> {
    segments.iter().flat_map(|s| [s.from, s.to]).collect()
}

/// The stable partition of rays by their centre's `axis` lane: those above `at` (in order), then the
/// rest (in order).
#[allow(clippy::type_complexity)]
fn partition(
    segments: &[crate::ray_probe::Segment],
    centres: &[Vf],
    at: F,
    axis: usize,
) -> (Vec<(crate::ray_probe::Segment, Vf)>, Vec<(crate::ray_probe::Segment, Vf)>) {
    segments.iter().copied().zip(centres.iter().copied()).partition(|(_, c)| at < F(c[axis]))
}

/// **Where a ray tree node splits** (C17): the box round every end (each end past the greatest raises
/// it, else one under the least lowers it), cut into twentieths along each axis. The first guess is
/// the middle of the longest axis (the first of equals; a box with no length the original leaves
/// unset, `x` here); then the cuts at 1 to 18 twentieths, cut by cut and `x`, `y`, `z` within each,
/// are tried, and one whose two
/// sides' boxes' volumes sum under the best so far is taken. A cut that leaves a side empty is not
/// taken; a first guess that does counts a hundred times the whole box's volume. Gives the cut and its
/// axis.
fn split(segments: &[crate::ray_probe::Segment], centres: &[Vf]) -> (F, usize) {
    let points = ends(segments);
    let lane = |p: Vf| [F(p.x()), F(p.y()), F(p.z())];
    let (mut lo, mut hi) = (lane(points[0]), lane(points[0]));
    for &p in &points[1..] {
        let p = lane(p);
        for k in 0..3 {
            if !(p[k] <= hi[k]) {
                hi[k] = p[k];
            } else if p[k] < lo[k] {
                lo[k] = p[k];
            }
        }
    }
    let extent = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let mut axis = 0;
    let mut longest = F::ZERO;
    for k in 0..3 {
        if longest < extent[k] {
            (axis, longest) = (k, extent[k]);
        }
    }
    let step = |k: usize| extent[k] / F(SPLIT_CUTS);
    let cost = |at: F, k: usize| {
        let (above, below) = partition(segments, centres, at, k);
        let volume = |side: &[(crate::ray_probe::Segment, Vf)]| {
            let s: Vec<_> = side.iter().map(|&(s, _)| s).collect();
            crate::ride::ray_node(&ends(&s)).volume
        };
        (!above.is_empty() && !below.is_empty()).then(|| volume(&above) + volume(&below))
    };
    let mut best = cost(lo[axis] + step(axis) * F(SPLIT_FIRST), axis)
        .unwrap_or_else(|| crate::ride::ray_node(&points).volume * F(SPLIT_UNSPLIT));
    let mut cut = SPLIT_FIRST_CUT;
    for i in 0..SPLIT_TRIED {
        for k in 0..3 {
            if let Some(c) = cost(lo[k] + F((i + 1) as f32) * step(k), k) {
                if !(best <= c) {
                    (best, cut, axis) = (c, i, k);
                }
            }
        }
    }
    (lo[axis] + F((cut + 1) as f32) * step(axis), axis)
}

/// The split's twentieths, its first guess the tenth of them (cut 9, counted from 0), the 18 cuts
/// tried after, and the weight on a first guess that leaves a side empty.
const SPLIT_CUTS: f32 = 20.0;
const SPLIT_FIRST: f32 = 10.0;
const SPLIT_FIRST_CUT: usize = 9;
const SPLIT_TRIED: usize = 18;
const SPLIT_UNSPLIT: f32 = 100.0;

impl Phy {
    /// Read the file out of its bytes.
    pub fn read_bytes(bytes: &[u8]) -> Phy {
        Phy::read(&Ini::parse_bytes(bytes))
    }

    /// Load a parsed file, in the reader's order: each section's keys into the reader's own
    /// temporaries or straight into the models, then the conversions, then the models' setters.
    pub fn read(ini: &Ini) -> Phy {
        let zero = |s: &Section, key: &str| s.float_or(key, F::ZERO);
        let deg = |v: F| v * F(DEG);
        let mph = |v: F| v * F(MPH);

        let mass_properties = Part::read(ini.section("Mass Properties"));

        let s = ini.section("Main Rotor");
        let max_lateral_cyclic = deg(zero(&s, "MaxLateralCyclicPitchAngle"));
        let max_longitudinal_cyclic = deg(zero(&s, "MaxLongitudinalCyclicPitchAngle"));
        let max_collective = deg(zero(&s, "MaxCollectivePitchAngle"));
        let lift_curve_slope = zero(&s, "LiftCurveSlope");
        let blade_chord = zero(&s, "BladeChord");
        let blades = zero(&s, "NumberOfBlades");
        let radius = zero(&s, "Radius");
        let angular_rate = zero(&s, "AngularRate");
        let inertia = zero(&s, "Inertia");
        let position = s.vector("Position", ORIGIN);
        let max_moment = s.vector("MaxRotorMoment", ORIGIN);
        let main_rotor = MainRotor {
            inertia,
            position,
            min_altitude: zero(&s, "MinAltitude"),
            max_altitude: zero(&s, "MaxAltitude"),
        };

        let s = ini.section("Tail Rotor");
        let max_tail_pitch = deg(zero(&s, "MaxPitchAngle"));
        let tail_radius = zero(&s, "Radius");
        let tail_rotor = TailRotor {
            lift_curve_slope: zero(&s, "LiftCurveSlope"),
            blade_chord: zero(&s, "BladeChord"),
            blades: zero(&s, "NumberOfBlades"),
            radius: tail_radius,
            radius_squared: tail_radius * tail_radius,
            gear_ratio: zero(&s, "GearRatio"),
            inertia: zero(&s, "Inertia"),
            position: s.vector("Position", ORIGIN),
        };
        let rotor = Rotor {
            max_lateral_cyclic,
            max_longitudinal_cyclic,
            max_collective,
            max_tail_pitch,
            lift_curve_slope,
            blade_chord,
            blades,
            radius,
            radius_squared: radius * radius,
            angular_rate,
            max_moment: [F(max_moment.x()), F(max_moment.y()), F(max_moment.z())],
        };

        let s = ini.section("Aerodynamics");
        let chord = zero(&s, "Chord");
        let span = zero(&s, "Span");
        let area = zero(&s, "SurfaceArea");
        let aspect_ratio = zero(&s, "AspectRatio");
        let default_angle_of_attack = zero(&s, "DefaultAngleOfAttack");
        let cl_round_off = zero(&s, "CLRoundOff");
        let cl_drop_off = zero(&s, "CLDropOff");
        let cl_scale = zero(&s, "CLScale");
        let cl_angle_scale = zero(&s, "CLAngleScale");
        let cl_alpha = zero(&s, "CLalpha");
        let cl_q = zero(&s, "CLq");
        let cd0 = zero(&s, "CD0");
        let cd1 = zero(&s, "CD1");
        let cy_beta = zero(&s, "CYbeta");
        let cy_p = zero(&s, "CYp");
        let cy_r = zero(&s, "CYr");
        let cl_beta = zero(&s, "Clbeta");
        let cl_p = zero(&s, "Clp");
        let cl_r = zero(&s, "Clr");
        let cl_da = zero(&s, "Clda");
        let cm0 = zero(&s, "CM0");
        let cm_alpha = zero(&s, "CMalpha");
        let cm_q = zero(&s, "CMq");
        let cm_de = zero(&s, "CMde");
        let cn_beta = zero(&s, "Cnbeta");
        let cn_p = zero(&s, "Cnp");
        let cn_r = zero(&s, "Cnr");
        let cn_dr = zero(&s, "Cndr");
        let cy_beta_pedal = zero(&s, "CYbetaPedal");
        let cn_beta_pedal = zero(&s, "CnbetaPedal");
        let cn_r_pedal = zero(&s, "CnrPedal");
        let low_speed_drag = zero(&s, "LowSpeedDrag");
        let drag_high_speed = mph(zero(&s, "DragHighSpeed"));
        let high_speed_drag = zero(&s, "HighSpeedDrag");
        let low_speed_aero = mph(zero(&s, "LowSpeedAero"));
        let high_speed_aero = mph(zero(&s, "HighSpeedAero"));
        let turbulence_magnitude = mph(zero(&s, "TurbulenceMagnitude"));
        let turbulence_time_scale = zero(&s, "TurbulenceTimeScale");
        let turbulence_time_offset = s.vector("TurbulenceTimeOffset", ORIGIN);
        // The wind is scaled on the vector unit, `x`, `y` and `z` only.
        let wind = s.vector("SteadyWind", ORIGIN);
        let steady_wind = wind.merge(Dest::XYZ, wind.mul(Vf::splat(MPH)));
        let chord = wing_chord(chord, span, aspect_ratio);
        let aero = Aero {
            density: F(AIR_DENSITY),
            chord,
            span,
            area,
            cl_alpha,
            cd0,
            cy_beta,
            cl_beta,
            cl_p,
            cl_da,
            cm0,
            cm_alpha,
            cm_q,
            cm_de,
            cn_beta,
            cn_r,
            cn_dr,
            low_speed_drag,
            low_speed_aero,
            high_speed_aero,
        };
        let aero_keys = AeroKeys {
            aspect_ratio,
            default_angle_of_attack: deg(default_angle_of_attack),
            cl_round_off,
            cl_drop_off,
            cl_scale,
            cl_angle_scale,
            cl_q,
            cd1,
            cy_p,
            cy_r,
            cl_r,
            cn_p,
            cy_beta_pedal,
            cn_beta_pedal,
            cn_r_pedal,
            drag_high_speed,
            high_speed_drag,
            turbulence_magnitude,
            turbulence_time_scale,
            turbulence_time_offset,
            steady_wind,
        };

        let s = ini.section("Flight Controller");
        let lanes = |v: Vf| [F(v.x()), F(v.y()), F(v.z())];
        let max_roll = deg(zero(&s, "MaxRollCommand"));
        let roll_gain = zero(&s, "RollGain");
        let speed_command_rate = mph(zero(&s, "SpeedCommandRate"));
        let min_speed_command = mph(zero(&s, "MinSpeedCommand"));
        let max_speed_command = mph(zero(&s, "MaxSpeedCommand"));
        let speed_gain = zero(&s, "SpeedGain");
        let max_pitch = deg(zero(&s, "MaxPitchCommand"));
        let pitch_gain = zero(&s, "PitchGain");
        let min_feed_forward_speed = mph(zero(&s, "MinPitchRateFeedForwardSpeed"));
        let min_feed_forward_scale = zero(&s, "MinPitchRateFeedForwardScale");
        let max_feed_forward_speed = mph(zero(&s, "MaxPitchRateFeedForwardSpeed"));
        let max_feed_forward_scale = zero(&s, "MaxPitchRateFeedForwardScale");
        let min_rate = lanes(s.vector("MinRateCommand", ORIGIN));
        let max_rate = lanes(s.vector("MaxRateCommand", ORIGIN));
        let controller = Controller {
            lateral_cyclic_gain: zero(&s, "LateralCyclicGain"),
            longitudinal_cyclic_gain: zero(&s, "LongitudinalCyclicGain"),
            pedal_gain: zero(&s, "PedalGain"),
            max_altitude: zero(&s, "MaxAltitude"),
            hold_delay: F(HOLD_DELAY),
            collective_gain: zero(&s, "CollectiveGain"),
            climb_rate: zero(&s, "ClimbRateGain"),
            descent_rate: zero(&s, "DescentRateGain"),
            altitude_hold_gain: zero(&s, "AltitudeHoldGain"),
            integrator_gain: zero(&s, "CollectiveIntegratorGain"),
            min_collective: zero(&s, "MinCollective"),
            max_pitch,
            max_roll,
            pitch_gain,
            roll_gain,
            speed_command_rate,
            min_speed_command,
            max_speed_command,
            speed_gain,
            min_feed_forward_speed,
            min_feed_forward_scale,
            max_feed_forward_speed,
            max_feed_forward_scale,
            min_rate,
            max_rate,
        };

        let wrecks = Wrecks::read(ini);
        let s = ini.section("Wrecks");
        let wreck_rotor = WreckRotor::new(s.float_or("WreckRotorRadius", F(WRECK_ROTOR_RADIUS)), zero(&s, "WreckRotorHeight"));

        let s = ini.section("Animation");
        Phy {
            mass_properties,
            rotor,
            main_rotor,
            tail_rotor,
            aero,
            aero_keys,
            controller,
            wrecks,
            wreck_rotor,
            pelvis_box_position: s.vector("PelvisBoxPosition", ORIGIN),
            pelvis_box_size: s.vector("PelvisBoxSize", ORIGIN),
        }
    }

    /// The crash state the file sets up: `MinWreckSpeed` and `WreckTime`, flying.
    pub fn crash(&self) -> Crash {
        Crash { min_wreck_speed: self.wrecks.min_wreck_speed, wreck_time: self.wrecks.wreck_time, ..Crash::default() }
    }
}

/// The aerodynamics' wing setter's one piece of arithmetic: a file with no `Chord` but a positive
/// `AspectRatio` gets the chord the span and the ratio make.
pub fn wing_chord(chord: F, span: F, aspect_ratio: F) -> F {
    if F::ZERO == chord && !(aspect_ratio <= F::ZERO) { span / aspect_ratio } else { chord }
}

/// One thrown blade: `helicopter\wreckblade.phy`'s `[PhysicsPart1]`, which the MX helicopter's reader
/// reads into each of its four blades the same.
pub fn wreck_blade(bytes: &[u8]) -> Part {
    Part::read_bytes(bytes, "PhysicsPart1")
}

// --- the helicopter's collision object ---------------------------------------------------------------

/// The helicopter's ray probe: the rigid-body part's `%sm.col` for the skin, the Kiowa's here.
pub const KIOWA_RAYS: &str = "helicopter\\col\\kiowam.col";
/// The helicopter's collidable mesh, beside it (the `%sf.col`): what another object's rays meet the
/// hull on — the thrown blades' (C21).
pub const KIOWA_HULL: &str = "helicopter\\col\\kiowaf.col";

/// The helicopter's collidable's categories (the collision object's `+0x1f8`, its `+0x84`) and the
/// object's actor and collides-with masks (`+0x110`, `+0x114`), off C14's save state: not keys of the
/// file. The plane's are the same but for the collidable's one bit ([`crate::flight::COLLIDABLE`]).
pub const COLLIDABLE: u32 = 0x0020_2058;
pub const ACTORS: u32 = 8;
pub const COLLIDES_WITH: u32 = 0xffff_ffff;

/// The crash test's surface mask (`+0x460`): a record whose world rays last struck an object of one of
/// these categories is spared the speed test. The vehicles' bit, as the plane's.
pub const CRASH_MASK: u32 = crate::flight_search::VEHICLE;

/// **The helicopter's collision object** (C14a): the rigid-body part's, the same class the plane's is
/// ([`crate::flight::Flight`]'s `collider` and `record`) — its response's tuning off `[Mass
/// Properties]` and the body's inertia ([`crate::wreck_bike::collider`]), its search's record with the
/// ray probe out of [`KIOWA_RAYS`] (one contact a ray), and the near-field search its nearby list is
/// kept in.
///
/// Its frame is the plane's: [`Collision::search`] before the world steps (the level's collision node
/// searches every object before any dynamics world runs), the response inside the force listener's
/// gravity step every substep, and the nearby list gathered again in the body wrapper's post
/// ([`Surroundings::post`]). [`Grounded`] is it as the helicopter's [`Surroundings`].
#[derive(Clone, Debug)]
pub struct Collision {
    pub collider: crate::contact::Collider,
    pub record: crate::contact_search::Record,
    pub rays: std::sync::Arc<crate::flight_search::Rays>,
    pub nearby: crate::broad_phase::Search,
    /// The part's mass: what the response pushes the body with.
    pub mass: F,
}

impl Collision {
    /// The collision object as the part's loader builds it for a helicopter whose body is `body` and
    /// whose body's inertia tensor (the body's `+0x110`, worked out from the model's box) is `inertia`.
    pub fn new(part: &crate::phy::Part, body: &Body, inertia: Vf, rays: crate::flight_search::Rays) -> Collision {
        use crate::wreck_bike;
        let collider = crate::contact::Collider {
            collidable: Some(COLLIDABLE),
            actors: ACTORS,
            collides_with: COLLIDES_WITH,
            ..wreck_bike::collider(part, part.mass, body.inv_mass, inertia)
        };
        let mut record = wreck_bike::record(part, wreck_bike::node_of(body));
        record.rays = Some(rays.pairs());
        record.capacity = rays.segments.len();
        Collision { collider, record, rays: std::sync::Arc::new(rays), nearby: crate::flight_search::search(), mass: part.mass }
    }

    /// **The search**, before the world steps: the record emptied, then the sweep back, the rays against
    /// the terrain and against the world's meshes and plants ([`crate::contact_search::collide`] over
    /// [`crate::flight_search::Scene::flown`]), the transform where the last post left the body. Gives
    /// the movers the contacts name, for the response.
    pub fn search(&mut self, body: &mut Body, world: &mut dyn crate::world::World, dt: F) -> Vec<crate::contact::Mover> {
        self.search_knocking(body, world, dt, &[]).0
    }

    /// **[`Collision::search`] with the thrown blades in the helicopter's nearby list** (C24): each
    /// of `knocks` (a blade, its collidable, and its distance as the list's gather left it,
    /// [`Blades::gathered`]) a collidable after the world's
    /// ([`crate::flight_search::Scene::knocking_gathered`]), its collidable's owner the blade's collision
    /// record. The helicopter's entry on blade `k` names `k` as its other side, for
    /// [`Grounded::resolve_among`]'s [`Blades::with_others`]; the mirrors each blade's record takes
    /// are given back by blade, in `knocks`' order, each naming the helicopter as side `0`, for
    /// [`Blades::file_struck`].
    pub fn search_knocking(
        &mut self,
        body: &mut Body,
        world: &mut dyn crate::world::World,
        dt: F,
        knocks: &[(usize, Knock<'_>, F)],
    ) -> (Vec<crate::contact::Mover>, Vec<(usize, Vec<crate::contact::Contact>)>) {
        use crate::contact_search::{self, Search};
        use crate::flight_search::{self, Scene};
        let rays = self.rays.clone();
        self.record.node = crate::wreck_bike::node_of(body);
        contact_search::clear(&mut self.record, Some(&mut *body));
        let (field, props, trees) = flight_search::near_world(world, &rays, &self.record.node, &mut self.nearby);
        let bounds = field.as_deref().map(field_box);
        let posed: Vec<Knock<'_>> = knocks.iter().map(|&(_, n, _)| n).collect();
        let distances: Vec<F> = knocks.iter().map(|&(_, _, d)| d).collect();
        let mut scene =
            Scene::flown(props, trees, None, bounds, &rays, &self.record.node, &mut self.nearby).sweep_of(crate::prop_world::sweep_categories());
        if !posed.is_empty() {
            scene = scene.knocking_gathered(&posed, 1, &distances);
        }
        let mut records = vec![std::mem::take(&mut self.record)];
        records.extend(posed.iter().map(|n| contact_search::Record { capacity: n.capacity, ..contact_search::Record::default() }));
        contact_search::collide(&mut records, 0, Some(body), field.as_deref(), &mut scene, dt);
        let movers = Search::movers(&scene);
        drop(scene);
        let mut records = records.into_iter();
        self.record = records.next().unwrap_or_default();
        for e in &mut self.record.contacts {
            if let Some(i) = e.other.and_then(|o| o.checked_sub(1)) {
                e.other = Some(knocks[i].0);
            }
        }
        let struck = records
            .zip(knocks)
            .filter(|(r, _)| !r.contacts.is_empty())
            .map(|(r, &(k, _, _))| (k, r.contacts.into_iter().map(|m| crate::contact::Contact { other: Some(0), ..m }).collect()))
            .collect();
        (movers, struck)
    }

    /// The body wrapper's post: the nearby list gathered again round the ray probe placed where the body
    /// now stands ([`crate::flight_search::refresh_flown`]).
    pub fn refresh(&mut self, body: &Body, world: &mut dyn crate::world::World) {
        let node = crate::wreck_bike::node_of(body);
        let (field, props, trees) = crate::flight_search::near_world(world, &self.rays, &node, &mut self.nearby);
        let bounds = field.as_deref().map(field_box);
        crate::flight_search::refresh_flown(props, trees, None, bounds, &self.rays, &node, &mut self.nearby);
    }

    /// **The rotor's contact search** (C17), in the crash test: the rotor's ray probe ([`ray_tree`]
    /// over `rotor`, the wreck rays) placed at the measure's `matrix` and searched against this
    /// object's nearby list as the last post gathered it, the probe's collidable the rotor's
    /// ([`ROTOR_CATEGORY`], [`ROTOR_SPARED`]). Whether its first contact struck an object whose
    /// category is not in [`STRUCK_MASK`].
    pub fn rotor_struck(&mut self, rotor: &[[Vf; 2]], matrix: &Mat4, world: &mut dyn crate::world::World) -> bool {
        let first = self.rotor_first_hit(rotor, matrix, world);
        first.is_some_and(|h| crate::flight_search::category(h.struck.object) & STRUCK_MASK == 0)
    }

    /// The rotor probe's first hit (C19), as [`Collision::rotor_struck`] searches it: what the
    /// original's ray record at `rotor probe + 0x6c` holds as its first point and `objects[0]`.
    pub fn rotor_first_hit(
        &mut self,
        rotor: &[[Vf; 2]],
        matrix: &Mat4,
        world: &mut dyn crate::world::World,
    ) -> Option<crate::contact_search::Hit> {
        use crate::contact_search::Search;
        use crate::flight_search::{self, Scene};
        let segments: Vec<_> = rotor.iter().map(|&[from, to]| crate::ray_probe::Segment { from, to }).collect();
        let rays = ray_tree(&segments);
        let node = crate::contact::Node { rows: [matrix.0[0], matrix.0[1], matrix.0[2]], origin: matrix.0[3] };
        let (at, _) = flight_search::reach(&rays, &matrix.0);
        let (field, props, trees) = world.wooded_near(at);
        let bounds = field.as_deref().map(field_box);
        let mut scene = Scene::flown(props, trees, None, bounds, &rays, &node, &mut self.nearby).ray_of(ROTOR_CATEGORY, &ROTOR_SPARED);
        scene.rays(true).first().copied()
    }

    /// **What the thrown blades' rays put on the hull** (C21), after this frame's
    /// [`Collision::search`]: the mirrors after the record's own entries, as many as its capacity
    /// leaves room for, each naming its blade; the record touching another object.
    pub fn file_mirrors(&mut self, mirrors: &[crate::contact::Contact]) {
        if mirrors.is_empty() {
            return;
        }
        let r = &mut self.record;
        (r.touching, r.on_object, r.touching_other) = (true, true, true);
        let room = r.capacity.saturating_sub(r.contacts.len());
        r.contacts.extend(mirrors.iter().copied().take(room));
    }

    /// The record's entries as the touchdown reads them.
    pub fn entries(&self) -> Vec<Touch> {
        self.record.contacts.iter().map(|c| Touch { kind: c.kind, t: F(c.on_body.y()) }).collect()
    }

    /// Whether what the record's world rays last struck is in the crash test's [`CRASH_MASK`].
    pub fn ignored(&self) -> bool {
        self.record.struck.is_some_and(|o| crate::flight_search::category(o) & CRASH_MASK != 0)
    }
}

/// A heightfield's box on the ground plane, (x, z) low and high.
fn field_box(f: &(dyn crate::contact_search::Heightfield + '_)) -> ([F; 2], [F; 2]) {
    let (lo, hi) = (f.min(), f.max());
    ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
}

/// **The helicopter in a world** (C14a): its [`Collision`] object, searched already this frame
/// ([`Collision::search`], whose `movers` it carries), as the frame's [`Surroundings`] — the response
/// is [`crate::contact::resolve`] on the record's contacts with impulses, the touching flag and the
/// entries are the record's, the post refreshes the nearby list in `world`, and the rotor's strike
/// asks `world`'s ground under the rotor's wreck rays' ends ([`rotor_in_ground`], `rotor` being
/// [`WreckRotor::rays`]).
///
/// What a crash test asks for is left in [`Grounded::asked`] for the caller to carry out (the wreck,
/// the blades, the put-back).
///
/// The strike also asks the rotor's own contact search ([`Collision::rotor_struck`], C17).
pub struct Grounded<'a> {
    pub collision: &'a mut Collision,
    pub world: &'a mut dyn crate::world::World,
    pub movers: &'a [crate::contact::Mover],
    pub rotor: &'a [[Vf; 2]],
    pub asked: Vec<Crashed>,
}

impl Grounded<'_> {
    /// [`Surroundings::resolve`] with the other sides the record's contacts name (C21): the thrown
    /// blades whose rays struck the hull, by blade ([`Blades::with_others`]).
    pub fn resolve_among(&mut self, body: &mut Body, others: &mut [crate::contact::Other<'_>]) {
        let c = &mut *self.collision;
        let mut weighed = crate::contact::Weighed { body, mass: c.mass };
        crate::contact::resolve(&mut c.collider, &mut weighed, &mut c.record.contacts, others, self.movers, true);
    }
}

impl Surroundings for Grounded<'_> {
    fn post(&mut self, body: &Body) {
        self.collision.refresh(body, self.world);
    }
    fn resolve(&mut self, body: &mut Body) {
        let c = &mut *self.collision;
        let mut weighed = crate::contact::Weighed { body, mass: c.mass };
        crate::contact::resolve(&mut c.collider, &mut weighed, &mut c.record.contacts, &mut [], self.movers, true);
    }
    fn touching(&self) -> bool {
        self.collision.record.touching
    }
    fn ignored(&self) -> bool {
        self.collision.ignored()
    }
    fn entries(&self) -> Vec<Touch> {
        self.collision.entries()
    }
    fn strike(&mut self, matrix: &Mat4) -> Strike {
        let ends: Vec<Vf> = self.rotor.iter().map(|r| r[1]).collect();
        let world = &mut *self.world;
        let in_ground = rotor_in_ground(matrix, &ends, |p| world.ground(p).0);
        Strike { in_ground, struck: self.collision.rotor_struck(self.rotor, matrix, &mut *self.world) }
    }
    fn crashed(&mut self, asked: &Crashed, _: &mut Ride, _: &mut Rng) {
        self.asked.push(*asked);
    }
}

// --- the wreck and the put-back ----------------------------------------------------------------------

/// What every accumulator and a body's start velocity and rates are reset to: the constant
/// `(0, 0, 0, 1)` the place hands the body's two start setters.
const STILL: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
/// The unwreck's damping, both of them: the constant `(1, 1, 1, 1)`.
const UNDAMPED: Vf = Vf::new(1.0, 1.0, 1.0, 1.0);
/// The wreck's scale on the rates (the plane's is 4).
const WRECK_SPIN: f32 = 1.0;
/// A thrown blade's random spin: up to this many radians a second on each axis, and a lift on `y`.
const BLADE_SPIN: f32 = 5.0;
const BLADE_LIFT: f32 = 7.0;

/// Which of the MX helicopter's model nodes are hidden (bit 6 of a node's byte 3): the ones the wreck,
/// the blade throw and the unwreck hide and show. The main rotor's node (`+0xd1c`, the one the rotor's
/// spin is skipped for) is [`Ride::rotor_hidden`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nodes {
    /// The rotor's three other nodes (`+0xd20`, `+0xd24`, `+0xd28`), hidden with it.
    pub rotor: [bool; 3],
    /// `+0xd3c`, which the wreck hides and the unwreck shows.
    pub intact: bool,
    /// The wrecked blades' node (`+0xd40`), which the wreck shows and the unwreck hides.
    pub wrecked: bool,
}

/// Where the blades are thrown from, as the scene graph has it the frame they go: each blade's node's
/// world transform (`+0xd08..+0xd14`), and the helicopter model's (`+0x18c`). Rows 0..2 the axes, row
/// 3 the origin, as the nodes keep them after their lazy rebuild.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throw {
    pub blades: [Mat4; 4],
    pub model: Mat4,
}

/// The helicopter's model, in its first skin: every skin has the same nodes ([`RotorTree::read`]).
pub const KIOWA_MODEL: &str = "helicopter\\anim\\kiowa.psg";

/// The nodes the main rotor turns, the four blades' nodes hang under, by their names in the model.
const SPINDLE: &str = "RotorSpindle";
const BLADE_NODES: [&str; 4] = ["Blade_Node01", "Blade_Node02", "Blade_Node03", "Blade_Node04"];
/// The axis the measure turns the spindle about by the rotor's angle: `y`.
const SPINDLE_AXIS: [f32; 3] = [0.0, 1.0, 0.0];

/// **The helicopter's model tree, as far as the blade throw reads it**: each node's own matrix (its
/// local rows, as the `.psg` holds them) and its parent, for the root, the main rotor's spindle and
/// everything between the spindle and the four blades' nodes. The root's own rows are never read:
/// the body's post writes them ([`Throw::of`]).
#[derive(Clone, Debug, PartialEq)]
pub struct RotorTree {
    /// Each node's local rows and its parent's index (`None` for the root), parents first.
    pub nodes: Vec<(Mat4, Option<usize>)>,
    /// The spindle's index: its rotation is the rotor's angle, its position the file's.
    pub spindle: usize,
    /// `Blade_Node01`..`04`'s indices.
    pub blades: [usize; 4],
}

impl RotorTree {
    /// The tree out of a skin's `.psg` bytes ([`KIOWA_MODEL`]): its 224-byte node records (the name,
    /// the local rows at byte 128, the parent's index at 220). An error for a file that is no `.psg`,
    /// lacks the spindle or a blade node, or hangs a blade anywhere but under the spindle.
    pub fn read(psg: &[u8]) -> Result<RotorTree, String> {
        const NODES: usize = 12;
        const RECORD: usize = 224;
        const NAME: usize = 128;
        const MATRIX: usize = 128;
        const PARENT: usize = 220;
        let word = |at: usize| -> Result<u32, String> {
            psg.get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| format!(".psg ends at {} before byte {at}", psg.len()))
        };
        if psg.get(..4) != Some(b"psg\0".as_slice()) {
            return Err("not a .psg: no psg\\0 at the front".into());
        }
        let count = word(8)? as usize;
        let at = |i: usize| NODES + i * RECORD;
        let mut nodes = Vec::with_capacity(count);
        let mut names = Vec::with_capacity(count);
        for i in 0..count {
            let name = psg.get(at(i)..at(i) + NAME).ok_or("a .psg cut short in its nodes")?;
            names.push(name.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect::<String>());
            let row = |r: usize| -> Result<Vf, String> {
                let b = at(i) + MATRIX + r * 16;
                Ok(Vf::from_bits([word(b)?, word(b + 4)?, word(b + 8)?, word(b + 12)?]))
            };
            let local = Mat4([row(0)?, row(1)?, row(2)?, row(3)?]);
            let parent = match usize::try_from(word(at(i) + PARENT)? as i32) {
                Ok(p) if p < i => Some(p),
                Ok(p) => return Err(format!("node {i}'s parent {p} comes after it")),
                Err(_) => None,
            };
            nodes.push((local, parent));
        }
        if nodes.first().is_none_or(|n| n.1.is_some()) {
            return Err("a .psg whose first node is not its root".into());
        }
        let find = |want: &str| names.iter().position(|n| n == want).ok_or_else(|| format!("the model has no {want} node"));
        let spindle = find(SPINDLE)?;
        let mut blades = [0; 4];
        for (k, want) in BLADE_NODES.iter().enumerate() {
            blades[k] = find(want)?;
            let mut up = nodes[blades[k]].1;
            while up.is_some_and(|p| p != spindle) {
                up = up.and_then(|p| nodes[p].1);
            }
            if up.is_none() {
                return Err(format!("{want} does not hang under {SPINDLE}"));
            }
        }
        Ok(RotorTree { nodes, spindle, blades })
    }
}

/// **`node_set_rotation`'s matrix** (`axis_rotation_build`): the turn about `axis` (normalised first,
/// on the FPU) whose angle's cosine and sine are `cos` and `sin`, as VU0 builds it — `v (1 - cos)`
/// times each of `v`'s lanes, then `cos` on the diagonal and `v sin`'s lanes added and taken off
/// either side of it. Row 3 is `(0, 0, 0, 1)`. The axis's `w` lane is the sign half of its `z` word,
/// as the register pair the original moves it in holds it.
fn axis_turn(axis: [f32; 3], cos: F, sin: F) -> Mat4 {
    let [x, y, z] = axis.map(F);
    let len2 = fpu::madd(fpu::adda((x * x).0, (y * y).0), z.0, z.0);
    let r = F(fpu::rsqrt(1.0, len2));
    let (x, y, z) = (x * r, y * r, z * r);
    let w = if z.to_bits() >> 31 != 0 { f32::from_bits(u32::MAX) } else { 0.0 };
    let v = Vf::new(x.0, y.0, z.0, w);
    let one_less = Vf::msub(v.add(Vf::VF0.bc(Field::X)), v, Vf::splat(cos.0));
    let s = v.mul(Vf::splat(sin.0));
    let [r0, r1, r2] = [Field::X, Field::Y, Field::Z].map(|f| one_less.mul(v.bc(f)));
    use ps2_float::vu::{add, sub};
    let (c, sx, sy, sz) = (cos.0, s.x(), s.y(), s.z());
    let row0 = Vf::new(add(r0.x(), c), add(r0.y(), sz), sub(r0.z(), sy), r0.w());
    let row1 = Vf::new(sub(r1.x(), sz), add(r1.y(), c), add(r1.z(), sx), r1.w());
    let row2 = Vf::new(add(r2.x(), sy), sub(r2.y(), sx), add(r2.z(), c), r2.w());
    Mat4([row0, row1, row2, Vf::new(0.0, 0.0, 0.0, 1.0)])
}

/// **`transform_rebuild`'s step**: a node's world rows, its local rows carried by its parent's
/// world rows — each axis row `p0 x + p1 y + p2 z` (accumulated in that order), the origin
/// `p3 + p0 x + p1 y + p2 z`, every lane.
fn rebuild(local: &Mat4, parent: &Mat4) -> Mat4 {
    let [p0, p1, p2, p3] = parent.0;
    let row = |acc: Vf, l: Vf| Vf::madd(Vf::madd(Vf::madd(acc, p0, l.bc(Field::X)), p1, l.bc(Field::Y)), p2, l.bc(Field::Z));
    let [l0, l1, l2, l3] = local.0;
    let axis = |l: Vf| Vf::madd(Vf::madd(p0.mul(l.bc(Field::X)), p1, l.bc(Field::Y)), p2, l.bc(Field::Z));
    Mat4([axis(l0), axis(l1), axis(l2), row(p3.add(Vf::VF0.bc(Field::X)), l3)])
}

impl Throw {
    /// **Where the blades go from, as the MX helicopter's own nodes have it** at the crash test:
    ///
    /// * the model's root is where the body's post (the first thing the measure does) put it, the
    ///   body's last orientation at its last centre of gravity less the turned `CofG`
    ///   ([`Body::pose`]);
    /// * the spindle is turned by `rotor_angle` about `y` at the file's position, as the measure's
    ///   rotor step left it — the frame before's angle, the crash test coming before this frame's
    ///   step ([`spin_blades`]): `{cos, sin}` off the curve table ([`CurveTable::at`]) into
    ///   [`axis_turn`];
    /// * every other node is the file's, and each blade node's world rows are the lazy rebuild's
    ///   chain from the root down ([`rebuild`]).
    ///
    /// A hidden rotor's angle is not run on, and nor is its spindle turned: the angle still names
    /// the turn the spindle was last given.
    pub fn of(tree: &RotorTree, body: &Body, rotor_angle: F, curve: &CurveTable) -> Throw {
        let (quat, origin) = body.pose();
        let root = quat_to_mat4(quat, origin);
        let (cos, sin) = curve.at(rotor_angle);
        let mut spun = axis_turn(SPINDLE_AXIS, cos, sin);
        let at = tree.nodes[tree.spindle].0.0[3];
        spun.0[3] = Vf::new(at.x(), at.y(), at.z(), 1.0);
        let mut worlds: Vec<Option<Mat4>> = vec![None; tree.nodes.len()];
        worlds[0] = Some(root);
        fn world(i: usize, tree: &RotorTree, spun: &Mat4, worlds: &mut [Option<Mat4>]) -> Mat4 {
            if let Some(m) = worlds[i] {
                return m;
            }
            let (local, parent) = &tree.nodes[i];
            let local = if i == tree.spindle { spun } else { local };
            // Only the root has no parent, and its rows are set above.
            let p = world(parent.unwrap_or(0), tree, spun, worlds);
            let m = rebuild(local, &p);
            worlds[i] = Some(m);
            m
        }
        let blades = tree.blades.map(|b| world(b, tree, &spun, &mut worlds));
        Throw { blades, model: root }
    }
}

/// What the wreck, the unwreck and the put-back read and write besides [`Ride`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wreckage {
    /// `[Wrecks]`' three the body takes: `WreckGravity` (the scale on gravity while wrecked),
    /// `WreckDamping` (both of the body's dampings) and `WreckInertia` (a scale on its inertia).
    pub wreck_gravity: F,
    pub wreck_damping: Vf,
    pub wreck_inertia: Vf,
    /// The body's inertia as the part was built (`+0x410`): what the wreck scales and the unwreck
    /// gives back. The unwreck's setter writes 1 into its `w`.
    pub inertia: Vf,
    /// The body's mass, which both gravities are weighed by.
    pub mass: F,
    /// The inertia the body was last given (the body's `+0x110`; `w` 1). The body steps only on its
    /// inverse ([`Body::inv_inertia`]).
    pub body_inertia: Vf,
    /// Set by the wreck and cleared by the unwreck (`+0x3d8`).
    pub wrecked: bool,
    /// Where the put-back stands the body, and its nose and up: the body wrapper's place
    /// (`+0xe0`, `+0xf0`, `+0x100`). The put-back squares the nose and up in place and sets the
    /// place's `w` to 1.
    pub place: [Vf; 3],
    /// The MX helicopter's spawn (`+0xcd0`, three quads), which its put-back copies over the place
    /// unless the helicopter's owner keeps it.
    pub spawn: [Vf; 3],
    pub nodes: Nodes,
    /// The four thrown blades. [`Wreckage::new`] leaves none: building them off the disc's files is
    /// [`Blades::new`]'s, and the caller's to put here.
    pub blades: Blades,
}

/// `body_set_inertia`: the tensor's `w` set to 1 (in the caller's copy too), kept, and each of its
/// three lanes' reciprocal into the inverse, or 0 for a lane not above 0. The inverse's `w` stays.
fn set_inertia(body: &mut Body, v: &mut Vf) -> Vf {
    v[3] = 1.0;
    for k in 0..3 {
        body.inv_inertia[k] = if F(v[k]) <= F::ZERO { 0.0 } else { (F::ONE / F(v[k])).to_f32() };
    }
    *v
}

/// The physics object's place, on a body: the nose and up squared (the plane's [`crate::flight::spawn_axes`]),
/// the position's `w` set to 1, and the body reset standing still there, turned to them. Hands the
/// squared nose and up back, as the original writes them over its arguments.
fn place(body: &mut Body, position: &mut Vf, facing: Vf, up: Vf) -> (Vf, Vf) {
    let turn = crate::flight::spawn_turn(facing, up);
    position[3] = 1.0;
    let mut start = crate::ragdoll_pose::Start::default();
    start.place(*position, STILL);
    start.turn(turn, STILL);
    start.reset(body);
    crate::flight::spawn_axes(facing, up)
}

/// `orientation_transform_point`: `p` through the transform, row 3 first (`+ vf0.x`), then `x`,
/// `y` and `z`; `w` is `p`'s.
fn node_point(m: &Mat4, p: Vf) -> Vf {
    let m = &m.0;
    let acc = m[3].add(Vf::VF0.bc(Field::X));
    let acc = Vf::madd(acc, m[0], p.bc(Field::X));
    let acc = Vf::madd(acc, m[1], p.bc(Field::Y));
    p.merge(Dest::XYZ, Vf::madd(acc, m[2], p.bc(Field::Z)))
}

/// `transform_rotate`: `v` through the transform's three axis rows; `w` is `v`'s.
fn node_rotate(m: &Mat4, v: Vf) -> Vf {
    let m = &m.0;
    let acc = m[0].mul(v.bc(Field::X));
    let acc = Vf::madd(acc, m[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m[2], v.bc(Field::Z)))
}

/// `transform_unpoint`: `p` less the origin, through the transpose of the axes, which the original
/// builds with the multimedia word shuffles (its third column keeps row 2's upper half: a 64-bit `or`
/// copies only the lower one). `w` is `p`'s.
fn node_unpoint(m: &Mat4, p: Vf) -> Vf {
    let [r0, r1, r2, origin] = m.0;
    let c0 = Vf::new(r0.x(), r1.x(), r2.x(), r2.y());
    let c1 = Vf::new(r0.y(), r1.y(), r2.y(), r2.w());
    let c2 = Vf::new(r0.z(), r1.z(), r2.z(), r2.w());
    let d = p.merge(Dest::XYZ, p.sub(origin));
    let acc = c0.mul(d.bc(Field::X));
    let acc = Vf::madd(acc, c1, d.bc(Field::Y));
    d.merge(Dest::XYZ, Vf::madd(acc, c2, d.bc(Field::Z)))
}

impl Wreckage {
    /// The figures off the `.phy` ([`Phy::wrecks`]) and the body as it was built: its inertia (the
    /// tensor before any wreck) and its mass. Standing, not wrecked, no blades out, and the nodes as
    /// S148's flown helicopter has them (and the unwreck leaves them): only the wrecked blades hidden.
    pub fn new(phy: &Phy, inertia: Vf, mass: F) -> Wreckage {
        Wreckage {
            wreck_gravity: phy.wrecks.wreck_gravity,
            wreck_damping: phy.wrecks.wreck_damping,
            wreck_inertia: phy.wrecks.wreck_inertia,
            inertia,
            mass,
            body_inertia: Vf::new(inertia.x(), inertia.y(), inertia.z(), 1.0),
            nodes: Nodes { wrecked: true, ..Nodes::default() },
            ..Wreckage::default()
        }
    }

    /// **The wreck**, the base slot `+0x1b8`: the reason, not down, the wrecked flag, the rates
    /// scaled by 1 (`xyz`; the oracle finds no lane it changes), both dampings `WreckDamping`, the
    /// inertia `WreckInertia` times the built one, and the body wrapper's gravity `WreckGravity` times
    /// the mass's weight. The wreck's sound is the original's too, and not simulation. The clock is not
    /// touched: the unwreck zeroed it.
    pub fn wreck(&mut self, ride: &mut Ride, reason: u32) {
        let mut inertia = self.wreck_inertia.merge(Dest::XYZ, self.wreck_inertia.mul(self.inertia));
        ride.crash.reason = reason;
        ride.crash.down = false;
        self.wrecked = true;
        let b = &mut ride.body;
        b.rates = b.rates.merge(Dest::XYZ, b.rates.mul(Vf::splat(WRECK_SPIN)));
        b.damping = self.wreck_damping;
        b.ang_damping = self.wreck_damping;
        self.body_inertia = set_inertia(b, &mut inertia);
        ride.gravity = crate::biplane::gravity_force(self.wreck_gravity, self.mass);
    }

    /// **The MX helicopter's wreck** (its slot `+0x1b8`): [`Wreckage::wreck`], then the blades thrown
    /// ([`Wreckage::throw_blades`]) unless it is the edge's [`THROWN`] wreck (a thrown helicopter
    /// throws them when it comes down), then `+0xd3c` hidden and the wrecked blades shown. (A
    /// player's pad rumbles too.)
    pub fn wreck_mx(&mut self, ride: &mut Ride, reason: u32, throw: &Throw, rng: &mut Rng) {
        self.wreck(ride, reason);
        if reason != THROWN {
            self.throw_blades(ride, throw, rng);
        }
        self.nodes.intact = true;
        self.nodes.wrecked = false;
    }

    /// **The blade throw**, the MX helicopter's slot `+0x1b0`. Each of the four blades, in order:
    ///
    /// * stands where its node puts its centre of gravity, turned to the node's `z` and `y` rows
    ///   (the body wrapper's place: squared, reset standing still);
    /// * is given a spin of the helicopter's rates plus `5 * unit()` on each axis (three `rand()`s,
    ///   `x` first), with 7 more on `y`, and flies off at the helicopter's velocity plus that spin
    ///   crossed with the blade's lever from the helicopter's centre of gravity (the blade's point in
    ///   the model's frame, less the body's `CofG`), turned back by the model's axes;
    /// * keeps the helicopter's rates, not that spin.
    ///
    /// Each is started first ([`Blades::start`]) and placed with the throw's place and axes, not its
    /// own ([`Blades::throw`]).
    ///
    /// Then the rotor's four nodes and `+0xd3c` are hidden and the wrecked blades shown.
    pub fn throw_blades(&mut self, ride: &mut Ride, throw: &Throw, rng: &mut Rng) {
        let heli = ride.body;
        for (k, node) in throw.blades.iter().enumerate() {
            // With no blades built the generator is still drawn on, as the original's four are.
            let cofg = self.blades.set.bones.get(k).map_or(Vf::ZERO, |b| b.body.cofg);
            let at = node_point(node, cofg);
            let facing = node.0[2].merge(Dest::W, Vf::ZERO);
            let up = node.0[1].merge(Dest::W, Vf::ZERO);
            let mut draw = [F::ZERO; 3];
            for d in &mut draw {
                *d = F(BLADE_SPIN) * rng.unit();
            }
            // The draws' quad's `w` is whatever the stack held; nothing reads it.
            let spin = Vf::new(draw[0].to_f32(), draw[1].to_f32(), draw[2].to_f32(), 0.0);
            let mut spin = spin.merge(Dest::XYZ, spin.add(heli.rates));
            spin[1] = (F(spin.y()) + BLADE_LIFT).to_f32();
            let lever = node_unpoint(&throw.model, at);
            let lever = lever.merge(Dest::XYZ, lever.sub(heli.cofg));
            let swing = spin.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(spin, lever), lever, spin));
            let swing = node_rotate(&throw.model, swing);
            let velocity = heli.velocity.merge(Dest::XYZ, heli.velocity.add(swing));
            if k < self.blades.set.bones.len() {
                self.blades.throw(k, at, facing, up, velocity, heli.rates);
            }
        }
        ride.rotor_hidden = true;
        self.nodes = Nodes { rotor: [true; 3], intact: true, wrecked: false };
    }

    /// **The unwreck**, the base slot `+0x1bc`: an edge-thrown helicopter's throw scale doubled back
    /// (divided by 0.5), the reason, the clock and the wrecked flag zeroed (not down), both dampings
    /// 1, the built inertia given back, and the body wrapper's gravity the gravity scale's again. The
    /// sound's stop is the original's too. The pad, the altitude fraction and the held height it
    /// zeroes are [`Kiowa::unwreck_controls`].
    pub fn unwreck(&mut self, ride: &mut Ride) {
        if ride.crash.reason == THROWN {
            ride.throw_scale = ride.throw_scale / F(0.5);
        }
        ride.crash.reason = 0;
        ride.crash.clock = F::ZERO;
        self.wrecked = false;
        let b = &mut ride.body;
        b.damping = UNDAMPED;
        b.ang_damping = UNDAMPED;
        self.body_inertia = set_inertia(b, &mut self.inertia);
        ride.gravity = crate::biplane::gravity_force(ride.gravity_scale, self.mass);
    }

    /// **The MX helicopter's unwreck** (its slot `+0x1bc`): [`Wreckage::unwreck`], then the rotor's
    /// nodes and `+0xd3c` shown, the wrecked blades hidden, and the four blades stopped and hidden
    /// ([`Blades::stop`]). (The
    /// original asks for the rotor's node and the first blade first; the MX helicopter's constructor
    /// always builds both.)
    pub fn unwreck_mx(&mut self, ride: &mut Ride) {
        self.unwreck(ride);
        ride.rotor_hidden = false;
        self.nodes = Nodes { rotor: [false; 3], intact: false, wrecked: true };
        for k in 0..self.blades.set.objects.len() {
            self.blades.stop(k, true);
        }
    }

    /// **The MX helicopter's put-back**, its slot `+0x198`: the body wrapper's reset — the body stood
    /// still at the place, turned to its (squared) nose and up — then the unwreck
    /// ([`Wreckage::unwreck_mx`]), then, unless the helicopter's owner keeps its place (`owned`: the
    /// owner's flag bit 0), the spawn copied over the place. Returns the altitude fraction the
    /// put-back leaves: 0, or 0.5 when owned ([`Kiowa::put_back_controls`]).
    ///
    /// Not here: the reset's taking out and putting back of the wrapper's collision probes, the
    /// player's camera snapped to it, and the presentation copy (`kiowa_mx_present_a`).
    pub fn put_back(&mut self, ride: &mut Ride, owned: bool) -> F {
        let [mut at, facing, up] = self.place;
        let (facing, up) = place(&mut ride.body, &mut at, facing, up);
        self.place = [at, facing, up];
        self.unwreck_mx(ride);
        if owned {
            F(0.5)
        } else {
            self.place = self.spawn;
            F::ZERO
        }
    }

    /// **What a crash test asked for, done**, in the original's order (the MX helicopter's slots):
    /// the wreck, the blade throw, the put-back. `throw` is where the blades go from, should either
    /// throw them. Returns the put-back's altitude fraction if it put the helicopter back: the frame
    /// cannot reach the [`Kiowa`], so the caller hands it to [`Kiowa::put_back_controls`] after the
    /// tick (nothing between reads what it zeroes).
    pub fn act(&mut self, asked: &Crashed, ride: &mut Ride, throw: &Throw, owned: bool, rng: &mut Rng) -> Option<F> {
        if let Some(reason) = asked.wreck {
            self.wreck_mx(ride, reason, throw, rng);
        }
        if asked.debris {
            self.throw_blades(ride, throw, rng);
        }
        asked.recover.then(|| self.put_back(ride, owned))
    }
}

// --- the thrown blades ------------------------------------------------------------------------------

use crate::contact::Mover;
use crate::contact_search::Heightfield;
use crate::eco::Trees;
use crate::flight_search::{Knock, Rays};
use crate::track::Mesh;
use crate::prop_world::{self, Collection, Edge, Object};
use crate::props::NodeBox;
use crate::ragdoll::{Bone, Tables};
use crate::world::Props;
use crate::wreck_bike::{self, WreckBike};
use std::sync::Arc;

/// The thrown blade's model (the wrapper's node, `BladeBent`), its rays and its collidable mesh, in
/// `machine.res`. The two `.col`s are under `helicopter\col`, not beside the model.
pub const BLADE_MODEL: &str = "helicopter\\anim\\bladebent.psg";
pub const BLADE_RAYS: &str = "helicopter\\col\\bladebentm.col";
pub const BLADE_SHAPE: &str = "helicopter\\col\\bladebentf.col";

/// Where a blade's wrapper is put back to, and its look and up, as the MX constructor's init leaves
/// them: the origin, and the model's own frame.
const BLADE_PLACE: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
const BLADE_LOOK: Vf = Vf::new(0.0, 0.0, 1.0, 0.0);
const BLADE_UP: Vf = Vf::new(0.0, 1.0, 0.0, 0.0);

/// What a blade's start and stop switch on its wrapper besides the collection's object.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Started {
    /// Started: the wrapper's two active bits (`+0x31` bits 0 and 1), which its child components
    /// follow.
    pub out: bool,
    /// The blade's node shown (bit 6 of its byte 3 clear).
    pub shown: bool,
    /// The node in the scene, under the helicopter's parent node.
    pub placed: bool,
    /// The collision object's two probes and its mesh in the broad phase.
    pub probes: bool,
}

/// **The MX helicopter's four thrown blades** (`+0xe48..+0xe54`): one body wrapper each, the generic
/// physics object a prop's is ([`crate::prop_world`]), built off `helicopter\wreckblade.phy`
/// ([`wreck_blade`]) with its inertia off `bladebent.psg`'s box and the `bladebentm.col` rays. They
/// share the helicopter's world (the wreck bike's, [`wreck_bike::WORLD`]) and its world node; one
/// [`Collection`] holds them, always active, with no settle listener and the props' put-back off
/// (`ResetTime -1`: a blade come to rest sleeps where it lies until the unwreck stops it).
///
/// Each blade's collidable (the wrapper's `+0x128`, in the broad phase with its probes) is
/// `bladebentf.col` where the blade's node stands it ([`Blades::knocks`]), the props' kind
/// ([`prop_world::COLLIDABLE`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Blades {
    pub set: Collection,
    pub started: [Started; 4],
    /// Each blade's ray probe tree, in blade order, for the collide pass.
    pub rays: Arc<Vec<Option<Rays>>>,
    /// The four blades' collidable mesh, `bladebentf.col`, when it was given.
    pub shape: Option<Arc<Mesh>>,
    /// The helicopter's collidable mesh, [`KIOWA_HULL`], when it was given ([`Blades::with_hull`]):
    /// what the blades' rays meet the hull on (C21).
    pub hull: Option<Arc<Mesh>>,
    /// The blades in the helicopter's nearby list as its post last gathered or re-sorted it, each
    /// with its distance then ([`Blades::gather_round`], C24).
    pub near: Vec<(usize, F)>,
    /// Where the helicopter's list was last gathered round, as the last [`Blades::gather_round`] saw it.
    pub gathered_round: Option<[F; 4]>,
}

impl Default for Blades {
    /// No blades: what [`Wreckage::new`] leaves before [`Blades::new`]'s are put in.
    fn default() -> Blades {
        Blades { set: blade_set(Vec::new(), Vec::new(), None), started: [Started::default(); 4], rays: Arc::default(), shape: None, hull: None, near: Vec::new(), gathered_round: None }
    }
}

fn blade_set(bones: Vec<Bone>, objects: Vec<Object>, edge: Option<Edge>) -> Collection {
    Collection {
        world: wreck_bike::WORLD,
        bones,
        objects,
        joints: Vec::new(),
        springs: Vec::new(),
        bodies: Vec::new(),
        forces: Vec::new(),
        preforce: Vec::new(),
        posts: Vec::new(),
        listeners: Vec::new(),
        active: true,
        pin_to_body: false,
        reset_time: F(-1.0),
        countdown: F(-1.0),
        edge,
    }
}

impl Blades {
    /// **The four blades as the MX constructor builds them**: `wreckblade.phy`'s part (`phy`), each
    /// body's inertia off the model's box ([`NodeBox::from_model`] of `psg`, [`crate::props::box_size`],
    /// [`Part::inertia`]), the collision object with the `.col`'s rays (`rays`, [`BLADE_RAYS`]), the
    /// collidable mesh (`shape`, [`BLADE_SHAPE`]), and the world's bounds a body past which is put
    /// back (`edge`, the terrain's). Each placed at the origin in the model's frame, asleep, in no
    /// list, unfiled, its node hidden and out of the scene.
    pub fn new(phy: &[u8], psg: &[u8], rays: Option<&[u8]>, shape: Option<&[u8]>, edge: Option<Edge>) -> Result<Blades, String> {
        let part = wreck_blade(phy);
        let inertia = part.inertia(crate::props::box_size(Some(NodeBox::from_model(psg)?)));
        let rays = rays.map(crate::track::Rays::parse).transpose()?.map(|r| Rays::of(&r));
        let shape = shape.map(Mesh::parse).transpose()?.map(Arc::new);
        let made = WreckBike::new(&part, inertia, Vf::VF0, Vf::VF0);
        let mut body = made.body;
        body.enabled = false;
        let mut object = Object::new(&part, BLADE_PLACE, BLADE_LOOK, BLADE_UP);
        object.collision = Some(prop_world::Collision::new(&part, made.collider, rays.as_ref().map(Rays::pairs)));
        let bone = Bone { body, mass: part.mass, inertia, reference: Vf::VF0 };
        let mut set = blade_set(vec![bone; 4], vec![object; 4], edge);
        for k in 0..4 {
            set.place(k);
        }
        Ok(Blades { set, started: [Started::default(); 4], rays: Arc::new(vec![rays; 4]), shape, hull: None, near: Vec::new(), gathered_round: None })
    }

    /// **`wreck_part_start`** on blade `k` (its wrapper's slot `+0x11c`): started, the node shown and
    /// put in the scene, the probes and the mesh into the broad phase (the part collides), then the
    /// wake ([`Collection::enable`]: into the world's lists, the listener on the node, the collision
    /// object filed).
    pub fn start(&mut self, k: usize) {
        let s = &mut self.started[k];
        s.out = true;
        s.shown = true;
        s.placed = true;
        s.probes |= self.set.objects[k].collision_enabled;
        self.set.enable(k);
    }

    /// **`wreck_part_stop`** on blade `k` (slot `+0x120`): stopped, put to sleep and taken off the
    /// world node ([`Collection::take_off`]), the node hidden and out of the scene when `hide` (the
    /// unwreck always asks it), and the probes and the mesh out of the broad phase.
    pub fn stop(&mut self, k: usize, hide: bool) {
        self.started[k].out = false;
        self.set.take_off(k);
        let s = &mut self.started[k];
        if hide {
            s.shown = false;
            s.placed = false;
        }
        s.probes = false;
    }

    /// One blade thrown: started, then placed at `at` turned to `facing` and `up` (the wrapper's
    /// place, which squares them; its own put-back place is kept), and given the throw's `velocity`
    /// and `rates`.
    pub fn throw(&mut self, k: usize, at: Vf, facing: Vf, up: Vf, velocity: Vf, rates: Vf) {
        self.start(k);
        let o = &mut self.set.objects[k];
        let kept = (o.place, o.look, o.up);
        (o.place, o.look, o.up) = (at, facing, up);
        self.set.place(k);
        let o = &mut self.set.objects[k];
        (o.place, o.look, o.up) = kept;
        let b = &mut self.set.bones[k].body;
        b.velocity = velocity;
        b.rates = rates;
    }

    /// **The world node's collide pass, for the blades** (before any world steps): each filed
    /// blade's collision object searches the terrain `field`, the scene's `props` and the
    /// ecosystem's `trees` ([`Collection::search`]).
    pub fn search(&mut self, dt: F, field: Option<&dyn Heightfield>, props: Option<&Props>, trees: Option<&Trees>) {
        let rays = Arc::clone(&self.rays);
        self.set.search(dt, &rays, field, props, trees);
    }

    /// The blades with the helicopter's collidable mesh (`hull`, [`KIOWA_HULL`]) for their rays to
    /// meet ([`Blades::search_on`]).
    pub fn with_hull(mut self, hull: &[u8]) -> Result<Blades, String> {
        self.hull = Some(Arc::new(Mesh::parse(hull)?));
        Ok(self)
    }

    /// **The helicopter's gather, as the blades are in it** (C24): its post's refresh
    /// ([`Collision::refresh`]) gathers its kept list afresh only when the ray probe's reach has left
    /// the bound round where it was last gathered, and else only measures the list's distances again
    /// (`broad_phase::gather`). The blades are moving objects, which a fresh gather walks after the
    /// static ones, and their collidables are not held to the bound, so a gather while blades are out
    /// ([`Blades::knocks`]) puts every one of them in the list, where their last posts stood them (the crash test that throws them and their own
    /// posts come after the helicopter's); a re-sort keeps the ones it has, each measured again
    /// ([`crate::flight_search::knock_distance`] from the probe standing where `body` now is). A
    /// helicopter wrecked where it stands does not gather again, so its rays never meet its blades.
    /// Run straight after every refresh: the list's centre moves only on a fresh gather.
    pub fn gather_round(&mut self, collision: &Collision, body: &Body) {
        let node = crate::wreck_bike::node_of(body);
        let centre = collision.nearby.centre;
        let fresh = self.gathered_round != Some(centre);
        self.gathered_round = Some(centre);
        let knocks = self.knocks();
        let measured = |k: usize| knocks.iter().find(|(j, _)| *j == k).map(|(_, n)| crate::flight_search::knock_distance(&collision.rays, &node, n));
        self.near = if fresh {
            knocks.iter().map(|&(k, _)| k).filter_map(|k| Some((k, measured(k)?))).collect()
        } else {
            self.near.iter().filter_map(|&(k, _)| Some((k, measured(k)?))).collect()
        };
    }

    /// The blades of the helicopter's nearby list ([`Blades::gather_round`]) still out, each where
    /// its node now stands it with its distance as gathered: what [`Collision::search_knocking`]
    /// offers the helicopter's rays.
    pub fn gathered(&self) -> Vec<(usize, Knock<'_>, F)> {
        let knocks = self.knocks();
        self.near.iter().filter_map(|&(k, d)| knocks.iter().find(|(j, _)| *j == k).map(|&(_, n)| (k, n, d))).collect()
    }

    /// **[`Blades::search`] with the helicopter's hull in the blades' world** (C21): its
    /// [`Blades::hull`] where its collision record's node stands it (as [`Collision::search`] left
    /// it this frame) and moving with `body`, its collidable the helicopter's ([`COLLIDABLE`],
    /// [`COLLIDES_WITH`]). A blade's entry on the hull names the helicopter as its other side `0`;
    /// the mirrors go on the hull's record after its own entries ([`Collision::file_mirrors`]),
    /// each naming its blade. Without the hull mesh, [`Blades::search`].
    pub fn search_on(
        &mut self,
        dt: F,
        field: Option<&dyn Heightfield>,
        props: Option<&Props>,
        trees: Option<&Trees>,
        collision: &mut Collision,
        body: &Body,
    ) {
        let Some(mesh) = self.hull.clone() else { return self.search(dt, field, props, trees) };
        let n = &collision.record.node;
        let at = Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin]);
        let wrecks = [prop_world::Wreck { hull: crate::flight_search::Plane { mesh: &mesh, at, velocity: body.velocity }, side: 0 }];
        let mut strikes = [crate::prop_wreck::WreckStrike::default()];
        let rays = Arc::clone(&self.rays);
        self.set.search_among_hulled(dt, &rays, field, props, trees, &wrecks, (COLLIDABLE, COLLIDES_WITH), &mut strikes);
        let [strike] = strikes;
        let mirrors: Vec<_> = strike
            .mirrors
            .iter()
            .map(|m| crate::contact::Contact { other: m.other.map(|i| strike.struck[i].object), ..*m })
            .collect();
        collision.file_mirrors(&mirrors);
    }

    /// **What the helicopter's rays put on the blades** (C24), after this frame's
    /// [`Blades::search_on`]: `struck`'s mirrors ([`Collision::search_knocking`]) on each blade's
    /// record ahead of its own entries — the collision node clears every record before any searches,
    /// and files the helicopter ahead of the four blades — as many as its capacity holds, its own
    /// entries after them up to it; the record touching another object.
    pub fn file_struck(&mut self, struck: Vec<(usize, Vec<crate::contact::Contact>)>) {
        for (k, mirrors) in struck {
            let Some(c) = self.set.objects[k].collision.as_mut() else { continue };
            let r = &mut c.record;
            (r.touching, r.on_object, r.touching_other) = (true, true, true);
            let own = std::mem::replace(&mut r.contacts, mirrors);
            r.contacts.extend(own);
            r.contacts.truncate(r.capacity);
        }
    }

    /// The blades as the other sides of the hull's mirrors (C21), by blade: each one's collision
    /// object and body, for `f`'s resolve ([`Grounded::resolve_among`]).
    pub fn with_others<R>(&mut self, f: impl FnOnce(&mut [crate::contact::Other<'_>]) -> R) -> R {
        let set = &mut self.set;
        let mut weighed: Vec<crate::contact::Weighed<'_>> =
            set.bones.iter_mut().map(|b| crate::contact::Weighed { body: &mut b.body, mass: b.mass }).collect();
        let mut others: Vec<crate::contact::Other<'_>> = set
            .objects
            .iter_mut()
            .zip(weighed.iter_mut())
            .filter_map(|(o, w)| Some(crate::contact::Other { collider: &mut o.collision.as_mut()?.collider, side: crate::contact::Side::Body(w) }))
            .collect();
        f(&mut others)
    }

    /// The blades as [`Surroundings::among`] hands them to the helicopter's step, the hull being
    /// `collision`'s.
    pub fn among<'a>(&'a mut self, tables: &'a Tables, movers: &'a [Mover], collision: &'a mut Collision) -> Among<'a> {
        Among { blades: &mut self.set, tables, movers, hull: &mut collision.collider, mass: collision.mass }
    }

    /// **The blades' collidables as another's rays meet them**: each blade whose probes and mesh are
    /// in the broad phase ([`Started::probes`]), as `(blade, its mesh where its node stands it)`, in
    /// blade order, posed as a knocked prop's is ([`crate::world::Prop::posed`]) — for a search's
    /// [`crate::flight_search::Scene::knocking`]. None without the [`Blades::shape`].
    pub fn knocks(&self) -> Vec<(usize, Knock<'_>)> {
        let Some(mesh) = self.shape.as_deref() else { return Vec::new() };
        let mut out = Vec::new();
        for (k, (o, s)) in self.set.objects.iter().zip(&self.started).enumerate() {
            let (true, Some(col)) = (s.probes, o.collision.as_ref()) else { continue };
            let (to_world, centre, radius) = crate::world::Prop::posed(mesh, &crate::prop_set::PropSet::matrix(&o.node));
            let velocity = self.set.bones[k].body.velocity;
            out.push((k, Knock { mesh, to_world, centre, radius, velocity, capacity: col.record.capacity }));
        }
        out
    }

    /// **The blades' share of the helicopter's world step**, but for the posts
    /// ([`Collection::step_to_posts`]), stepped apart from it: run beside [`Kiowa::tick`]'s, before
    /// the crash test's answer is acted on. `movers` are what the collide pass's hits name
    /// ([`crate::flight_search::movers`]). Where the blades meet the hull, they step in the
    /// helicopter's own step instead ([`Surroundings::among`], C21).
    pub fn step(&mut self, dt: F, tables: &Tables, movers: &[Mover]) {
        self.set.step_to_posts(dt, tables, movers);
    }

    /// **The frame's end, for the blades**: after [`Wreckage::act`], their post listeners (which
    /// follow the helicopter's in the world's list, so a blade the crash test threw this frame is
    /// posted where it was thrown and steps from the next), then their listeners on the node
    /// ([`Collection::step_posts`]).
    pub fn post(&mut self, dt: F) {
        self.set.step_posts(dt);
    }
}

impl Kiowa {
    /// The unwreck's share on the controls: the pad, the altitude fraction and the held height
    /// zeroed.
    pub fn unwreck_controls(&mut self) {
        self.commands.altitude_fraction = F::ZERO;
        self.pad = Pad::default();
        self.control.hold_altitude = F::ZERO;
    }

    /// The put-back's: [`Kiowa::unwreck_controls`], then the altitude fraction
    /// [`Wreckage::put_back`] hands back.
    pub fn put_back_controls(&mut self, altitude_fraction: F) {
        self.unwreck_controls();
        self.commands.altitude_fraction = altitude_fraction;
    }
}
