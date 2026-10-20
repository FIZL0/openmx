//! The biplane's own flight: everything the plane works out for itself before the rigid body it rides
//! turns it into motion.
//!
//! The plane is not stepped the way the bike is. The bike's ground step is a substep loop of its own;
//! the plane is one body in the engine's generic dynamics world, and that world calls back into it:
//! once a frame before the substeps (the *preforce* listener: the controls), then once every substep
//! (the *force* listener: the autopilot, the engine and the aerodynamics, then the three tires), with
//! the body integrated by the world in between. So the per-tick update R8 found is four pieces that
//! never call each other, and this module ports them as four functions over plain state:
//!
//! 1. [`controls`] — the pad's five fields through the rate limits into the throttle, the brake, the
//!    three control surfaces and the tail wheel's steer, with the elevator faded out as the plane rolls
//!    past the file's `ElevatorRampOffRollAngle`;
//! 2. [`autopilot`] — the pitch-rate loop: with the aileron stick inside its dead zone the elevator
//!    command is a pitch rate (the stick's, plus the turn rate a banked plane needs to hold its nose up)
//!    chased by a proportional-integral controller on the body's own pitch rate; outside it, the
//!    elevator is passed through scaled by `MaxElevator`;
//! 3. [`Engine::step`] — power from the throttle, a propeller efficiency that grows with speed and an
//!    altitude fade, as a thrust along the body's nose; plus the prop's angle and the engine-note pitch
//!    the audio and the animation read;
//! 4. [`Aero::forces`] — the six coefficients (lift with its rounded stall, drag, side force, and the
//!    roll, pitch and yaw moments), each from the angle of attack, the sideslip, the three body rates
//!    and the three surfaces, into a body-frame force and moment.
//!
//! What the original hands each of them — the plane's attitude, its airspeed and its angles, the body's
//! rates — is measured by the plane's measure listener after the substeps. Those are the rest of the
//! module:
//!
//! 5. [`Body`] — the rigid body: its force and moment accumulators (world-frame force, body-frame
//!    force turned by the orientation, body-frame moment), the plane's gravity, the second-order
//!    integrator the world calls every substep and the substep's end, and the pose it leaves;
//! 6. [`World::step`] — the dynamics world's frame: `(int)(dt * 115) + 1` substeps, the listeners and
//!    the bodies called over a [`Dynamics`] the caller supplies (in the game nothing is on the
//!    constraint list, but the passes are there);
//! 7. [`Measure`] — the measure listener: the grounded flag from the three wheels, the airspeed, alpha
//!    and beta off the arc cosine table, the body-frame velocity, the world rates, the Euler angles and
//!    the orientation matrix, with [`Measure::attitude`] and [`Measure::air`] handing them to 1–4.
//!
//! The tires, the collision response and the crash test are not in this module; see the journal
//! entries for what is ported and what is still the original's.
//!
//! Every field is named by the `biplane.phy` key it is loaded from, where it has one. The loader is
//! S33's; until it lands the proofs read the values off a save state's live plane.

#![deny(clippy::float_arithmetic)]

use crate::air::AcosTable;
use crate::camera::CurveTable;
use crate::chassis::{CLEAR, normalize, turn};
use crate::math::{Mat4, quat_to_euler, quat_to_mat4};
use crate::motion::remap;
use ps2_float::fpu::{self, Acc};
use ps2_float::{Dest, F, Field, Vf, libm, vu};

const PI: f32 = 3.1415927;
const FRAC_PI_2: f32 = 1.5707964;
const TWO_PI: f32 = 6.2831855;
const INV_TWO_PI: f32 = 0.15915494;
/// The curve table's index scale, `5999 / 2pi`: the engine keeps it in a global beside the table.
const CURVE_INV_STEP: f32 = 954.77045;
/// Feet per second squared.
const G: f32 = 32.2;
/// The nose's height (the forward axis' `y`) past which the elevator is not faded with roll: about
/// 53 degrees of pitch.
const STEEP_NOSE: f32 = 0.8;
/// The nose's height past which the autopilot stops asking for a coordinated turn's pitch rate: the
/// sine of 45 degrees, as the original spells it (`0.7070`, not `0.70710677`).
const TURN_NOSE: f32 = f32::from_bits(0x3f34_fdf4);
/// Horsepower to foot-pounds per second.
const HP: f32 = 550.0;
/// Revolutions per minute to radians per second, `2pi/60` as the original rounds it.
const RPM_TO_RAD: f32 = f32::from_bits(0x3dd6_7756);
/// The speed the side force and the yaw moment's sideslip terms use as their floor when the aero model
/// is told not to use the true airspeed for them: a global the engine sets to 131.9994 ft/s (90 mph).
const MIN_DYNAMIC_SPEED: f32 = f32::from_bits(0x4303_ffd9);

// --- the pilot ---------------------------------------------------------------------------------------

/// The pad, as the plane's input handler leaves it: five fields, each in `-1..1` (forward and back are
/// the buttons' analogue pressure, `0..1`). The controls zero all five while the plane is not being
/// flown.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    /// Throttle up (Cross as shipped).
    pub forward: F,
    /// Throttle down, the wheel brake, and with `forward` on the ground the reverse (Square).
    pub back: F,
    /// The left stick's `y`, cubed and negated: up is positive, the nose goes down.
    pub elevator: F,
    /// The left stick's `x`, cubed and negated: left is positive.
    pub aileron: F,
    /// L1 is -1, R1 is +1.
    pub rudder: F,
}

/// The plane's control state: what the controls and the autopilot write and the engine, the
/// aerodynamics, the tires and the animation read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surfaces {
    /// `ReverseThrottle..1`.
    pub throttle: F,
    /// The front wheels' brake, `0..1` (1 while the plane is not flown).
    pub brake: F,
    /// The elevator as a fraction of `MaxElevator`, rate-limited, faded with roll.
    pub elevator: F,
    /// The elevator deflection the aerodynamics use (radians): the autopilot's output.
    pub elevator_command: F,
    /// Radians, rate-limited, `+-MaxAileron`.
    pub aileron: F,
    /// Radians, rate-limited, `+-MaxRudder`.
    pub rudder: F,
    /// The tail wheel's steer, `-1..1` at the rudder's rate.
    pub steer: F,
    /// The pitch-rate loop's integrator, `+-MaxElevator`.
    pub pitch_integral: F,
}

/// The `[Joystick]` section and the surface limits from `[Aerodynamics]`, all in radians.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Joystick {
    /// `ThrottleIncrement`, per second.
    pub throttle_increment: F,
    /// `ReverseThrottle`, the throttle's floor.
    pub reverse_throttle: F,
    /// `AileronControlRateLimit`, radians per second.
    pub aileron_rate: F,
    /// `ElevatorControlRateLimit`, fraction per second.
    pub elevator_rate: F,
    /// `RudderControlRateLimit`, radians per second; also the tail wheel's.
    pub rudder_rate: F,
    /// `MaxElevator`.
    pub max_elevator: F,
    /// `MaxAileron`.
    pub max_aileron: F,
    /// `MaxRudder`.
    pub max_rudder: F,
    /// `ElevatorRampOffRollAngle`: the roll at which the elevator starts to fade.
    pub ramp_off_roll: F,
    /// `ElevatorOffRollAngle`: the roll at which it has faded to `min_elevator`.
    pub off_roll: F,
    /// `MinElevator`: the fraction of the stick left at `off_roll`.
    pub min_elevator: F,
}

/// The `[Autopilot]` section.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Autopilot {
    /// `AileronDeadZone`: with the aileron stick inside it the pitch loop runs.
    pub aileron_dead_zone: F,
    /// `MaxPitchRate`: the pitch rate full elevator asks for, radians per second.
    pub max_pitch_rate: F,
    /// `MaxRollAngle`: the bank the turn compensation is computed for at most.
    pub max_roll: F,
    /// `PitchRateFeedForwardscale`.
    pub feed_forward: F,
    /// `PitchRateGain`.
    pub gain: F,
    /// `PitchRateIntegratorGain`.
    pub integrator_gain: F,
}

/// What the controls and the autopilot read off the rest of the plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Attitude {
    /// The plane is being flown. While it is not, the controls zero the pad and the surfaces.
    pub flown: bool,
    /// Any wheel is on the ground.
    pub grounded: bool,
    /// The forward axis' `y`: the sine of the nose's pitch.
    pub nose_y: F,
    /// The roll angle (the measure's third Euler angle), `-pi..pi`.
    pub roll: F,
    /// The airspeed along the nose.
    pub speed: F,
    /// The body's pitch rate (its body-frame angular velocity's `x`).
    pub pitch_rate: F,
}

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

/// `step` toward `target`, at most `limit` either way.
fn toward(value: F, target: F, limit: F) -> F {
    value + (target - value).max(-limit).min(limit)
}

/// The preforce listener's control law, once a frame: the throttle and brake from the two buttons, the
/// three surfaces and the tail wheel rate-limited toward the pad.
///
/// The elevator is faded out between `ramp_off_roll` and `off_roll` of roll (and the same again
/// mirrored about inverted), but only while the nose is within about 53 degrees of level; the aileron
/// and rudder are not faded. With both buttons held on the ground and the throttle not already
/// forward, the throttle jumps to `reverse_throttle` and the brake comes off; otherwise a negative
/// throttle is first put back to zero, then forward opens it and back closes it at
/// `throttle_increment` per second, and back is the brake.
pub fn controls(pad: &mut Pad, s: &mut Surfaces, j: &Joystick, at: &Attitude, dt: F) {
    if !at.flown {
        *pad = Pad::default();
        *s = Surfaces {
            brake: F::ONE,
            elevator_command: s.elevator_command,
            pitch_integral: s.pitch_integral,
            ..Surfaces::default()
        };
        return;
    }
    let aileron_step = j.aileron_rate * dt;
    let elevator_step = j.elevator_rate * dt;
    let rudder_step = j.rudder_rate * dt;
    let roll = at.roll.abs();
    let fade = if at.nose_y.abs() < STEEP_NOSE {
        if roll <= FRAC_PI_2 {
            remap(roll, j.ramp_off_roll, j.off_roll, F::ONE, j.min_elevator)
        } else {
            remap(roll, F(PI) - j.off_roll, F(PI) - j.ramp_off_roll, j.min_elevator, F::ONE)
        }
    } else {
        F::ONE
    };
    s.elevator = toward(s.elevator, pad.elevator * fade, elevator_step);
    s.aileron = toward(s.aileron, pad.aileron * j.max_aileron, aileron_step);
    s.rudder = toward(s.rudder, pad.rudder * j.max_rudder, rudder_step);
    s.steer = toward(s.steer, pad.rudder, rudder_step);

    if pad.forward != 0.0 && pad.back != 0.0 && at.grounded {
        if s.throttle <= 0.0 {
            s.throttle = j.reverse_throttle;
        }
        s.brake = F::ZERO;
    } else {
        if s.throttle < 0.0 {
            s.throttle = F::ZERO;
        }
        if pad.forward != 0.0 {
            s.throttle = s.throttle + j.throttle_increment * dt;
        } else if pad.back != 0.0 {
            s.throttle = s.throttle - j.throttle_increment * dt;
        }
        s.brake = pad.back;
    }
    s.throttle = s.throttle.max(j.reverse_throttle).min(F::ONE);
}

/// The force listener's first call, every substep: the elevator command the aerodynamics fly.
///
/// With the aileron stick outside its dead zone it is just the elevator times `MaxElevator`. Inside it,
/// the elevator asks for a pitch rate instead — `elevator * MaxPitchRate`, plus `feed_forward` times
/// the pitch rate a coordinated turn at the current bank needs (`-(g sin(roll) / V) tan(roll)`, the
/// roll first held to `MaxRollAngle` from level or from inverted, and nothing at all with the nose
/// steeper than 45 degrees) — and a PI controller on the body's pitch rate chases it, its integral and
/// its output each held to `+-MaxElevator`.
pub fn autopilot(pad: &Pad, s: &mut Surfaces, j: &Joystick, ap: &Autopilot, at: &Attitude, table: &CurveTable, dt: F) {
    if !(pad.aileron.abs() < ap.aileron_dead_zone) {
        s.elevator_command = s.elevator * j.max_elevator;
        return;
    }
    let limit = ap.max_roll;
    let roll = if at.roll.abs() <= F(PI) - limit {
        at.roll.max(-limit).min(limit)
    } else if at.roll <= 0.0 {
        at.roll.min(F(-PI) + limit)
    } else {
        at.roll.max(F(PI) - limit)
    };
    let turn = if at.nose_y.abs() < TURN_NOSE {
        let (_, sin) = curve(wrap(wrap(roll)), table);
        let yaw_rate = F(G) * sin / at.speed;
        let (cos, sin) = curve(wrap(wrap(roll)), table);
        -yaw_rate * (sin / cos)
    } else {
        F::ZERO
    };
    let error = Acc::mula(s.elevator, ap.max_pitch_rate).madd(ap.feed_forward, turn) - at.pitch_rate;
    let max = j.max_elevator;
    s.pitch_integral = (s.pitch_integral + dt * (ap.integrator_gain * error)).max(-max).min(max);
    s.elevator_command = (s.pitch_integral + ap.gain * error).max(-max).min(max);
}

// --- the engine --------------------------------------------------------------------------------------

/// The engine object: the `[Engine]` keys and what it keeps between substeps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Engine {
    /// `MinPropEfficiency`: the propeller's efficiency standing still.
    pub min_efficiency: F,
    /// `MinPropEfficiencySpeed` (in ft/s): the speed at which the efficiency reaches 1.
    pub full_efficiency_speed: F,
    /// The prop's spin in revolutions per minute, written by the audio side.
    pub prop_rpm: F,
    /// The prop's angle, `-pi..pi`.
    pub prop_angle: F,
    /// `MaxPower`, horsepower.
    pub max_power: F,
    /// The altitude the power starts to fade at (0 as the constructor leaves it).
    pub fade_altitude: F,
    /// `MaxAltitude`: where the power is gone.
    pub max_altitude: F,
    /// `MaxPower * throttle`.
    pub power: F,
    /// The throttle as revolutions per minute, `idle_rpm..max_rpm`.
    pub rpm: F,
    /// The engine's idle and top revolutions (500 and 2500 as the constructor sets them).
    pub idle_rpm: F,
    pub max_rpm: F,
    /// Pounds along the nose.
    pub thrust: F,
    /// The engine note, `0..max_rpm`: the rpm's place in its range, wobbled at the ends by the smooth
    /// noise and lifted as the nose drops.
    pub note: F,
    /// Seconds of running, the noise's clock.
    pub clock: F,
}

impl Engine {
    /// One substep: the thrust as a body-frame force `(0, 0, thrust, 1)`, which the original hands the
    /// body to rotate into the world. `velocity` and `altitude` are the body's world velocity and its
    /// position's `y`; `noise` is the engine's shared smooth noise.
    pub fn step(&mut self, throttle: F, velocity: Vf, altitude: F, nose_y: F, dt: F, noise: &mut impl FnMut(F) -> F) -> Vf {
        self.power = self.max_power * throttle;
        self.rpm = remap(throttle, F::ZERO, F::ONE, self.idle_rpm, self.max_rpm);
        let v = [0, 1, 2].map(|k| F(velocity[k]));
        let speed = Acc::mula(v[0], v[0]).madda(v[1], v[1]).madd(v[2], v[2]).sqrt();
        let thin = remap(altitude, self.fade_altitude, self.max_altitude, F::ONE, F::ZERO);
        let efficiency = remap(speed, F::ZERO, self.full_efficiency_speed, self.min_efficiency, F::ONE);
        self.thrust = F(HP) * self.power * (thin * efficiency) / speed.max(F::ONE);
        let force = Vf::new(0.0, 0.0, self.thrust.to_f32(), 1.0);

        self.prop_angle = wrap(self.prop_angle + F(RPM_TO_RAD) * self.prop_rpm * dt);
        self.note = remap(self.rpm, self.idle_rpm, self.max_rpm, F(0.1), F(0.9));
        if !(self.note <= 0.75) {
            self.note = self.note + F(0.05) * noise(F(0.5) * self.clock);
        } else if self.note < 0.101 {
            self.note = self.note + F(0.2) * noise(F(0.5) * self.clock);
        }
        self.clock = self.clock + dt;
        self.note = self.note + remap(nose_y, F(-1.0), F::ONE, F(0.1), F(-0.1));
        self.note = remap(self.note, F::ZERO, F::ONE, F::ZERO, self.max_rpm);
        force
    }
}

// --- the aerodynamics --------------------------------------------------------------------------------

/// The aerodynamic model: the `[Aerodynamics]` keys and the six coefficients it last worked out.
///
/// The moment coefficients follow the file's names: `cl_*` is the roll moment (`Cl`), `cm_*` the
/// pitch moment, `cn_*` the yaw moment; `lift_*` is the lift coefficient (`CL`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aero {
    /// Use the true airspeed for the sideslip terms' dynamic pressure (the plane sets this); without it
    /// they never see less than [`MIN_DYNAMIC_SPEED`].
    pub true_speed: bool,
    /// Air density, slugs per cubic foot.
    pub density: F,
    /// `Chord`, `Span`, `SurfaceArea`.
    pub chord: F,
    pub span: F,
    pub area: F,
    /// `DefaultAngleOfAttack`, radians: added to the measured angle of attack.
    pub default_alpha: F,
    /// `CmBreakAngleOfAttack`, radians: where the stall's extra pitch moment starts.
    pub cm_break_alpha: F,
    /// The lift curve's linear slope (0 as the shipped file leaves it: the lift is all in the rounded
    /// term below).
    pub lift_alpha: F,
    /// `CLRoundOff`, `CLDropOff`, `CLScale`, `CLAngleScale`: the lift is
    /// `alpha * CLScale * CLAngleScale / (1 + |RoundOff * AngleScale * alpha|^DropOff)`, which rises
    /// linearly, rounds over and falls away past the stall.
    pub lift_round_off: F,
    pub lift_drop_off: F,
    pub lift_scale: F,
    pub lift_angle_scale: F,
    /// `CLq`, `CLde`.
    pub lift_q: F,
    pub lift_de: F,
    /// `CD0`, `CD1` (induced, times `CL^2`), `CDde` (times the elevator's magnitude).
    pub drag0: F,
    pub drag1: F,
    pub drag_de: F,
    /// `CYbeta`, `CYda`, `CYdr`, `CYp`, `CYr`.
    pub side_beta: F,
    pub side_da: F,
    pub side_dr: F,
    pub side_p: F,
    pub side_r: F,
    /// `Clbeta`, `Clp`, `Clr`, `Clda`, `Cldr`.
    pub cl_beta: F,
    pub cl_p: F,
    pub cl_r: F,
    pub cl_da: F,
    pub cl_dr: F,
    /// `CM0`, `CMalpha`, `CMq`, `CMde`, `CMstall`.
    pub cm0: F,
    pub cm_alpha: F,
    pub cm_q: F,
    pub cm_de: F,
    pub cm_stall: F,
    /// `Cnbeta`, `Cnp`, `Cnr`, `Cndr`, `Cnda`.
    pub cn_beta: F,
    pub cn_p: F,
    pub cn_r: F,
    pub cn_dr: F,
    pub cn_da: F,
    /// The coefficients of the last call: lift, drag, side force, roll, pitch (with the stall term).
    pub lift: F,
    pub drag: F,
    pub side: F,
    pub roll: F,
    pub pitch: F,
}

/// What the aerodynamics read: the measure's angles and airspeed, the surfaces, the body's rates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Air {
    pub alpha: F,
    pub beta: F,
    /// The airspeed along the nose (negative flying backwards).
    pub speed: F,
    /// [`Surfaces::elevator_command`], [`Surfaces::aileron`], [`Surfaces::rudder`].
    pub elevator: F,
    pub aileron: F,
    pub rudder: F,
    /// The body-frame angular velocity: `x` pitch, `y` yaw, `z` roll.
    pub rates: Vf,
}

impl Aero {
    /// One substep: the body-frame force and moment, each with `w` of 1 as the original passes them.
    ///
    /// The force is `S q (CY, CL, -CD)` (`+CD` flying backwards) with `q = rho V^2 / 2`; the moment is
    /// `S q (c CM, b Cn, b Cl)`. The rate terms are scaled by `c/2V` and `b/2V`, and dropped below 1
    /// ft/s. The yaw moment is the one oddity: its sideslip, yaw-rate and rudder terms are taken at the
    /// dynamic pressure of `max(V, MIN_DYNAMIC_SPEED)` unless [`Aero::true_speed`] is set, its roll-rate
    /// and aileron terms at the true one.
    pub fn forces(&mut self, air: &Air) -> (Vf, Vf) {
        let alpha = air.alpha + self.default_alpha;
        let v = air.speed;
        let beta = air.beta;
        let yaw_speed = if self.true_speed { v } else { F(MIN_DYNAMIC_SPEED).max(v) };
        let (c2v, b2v) = if v <= 1.0 {
            (F::ZERO, F::ZERO)
        } else {
            let half = F(0.5) / v;
            (self.chord * half, self.span * half)
        };
        let w = [0, 1, 2].map(|k| F(air.rates[k]));
        let (elevator, aileron, rudder) = (air.elevator, air.aileron, air.rudder);

        let round = F(libm::powf((alpha * (self.lift_round_off * self.lift_angle_scale)).abs().to_f32(),
                                 self.lift_drop_off.to_f32()));
        let curve = alpha * (self.lift_scale * self.lift_angle_scale) / (F::ONE + round);
        self.lift = Acc::adda(c2v * (self.lift_q * w[0]), self.lift_alpha * alpha + curve).madd(self.lift_de, elevator);
        self.drag = Acc::adda(self.drag0, self.lift * (self.drag1 * self.lift)).madd(self.drag_de, elevator.abs());
        let side = Acc::adda(self.side_beta * beta, self.side_da * aileron).madd(self.side_dr, rudder);
        self.side = Acc::adda(b2v * (self.side_p * w[2]), side).madd(b2v, self.side_r * w[1]);
        let roll = Acc::adda(self.cl_beta * beta, b2v * (self.cl_p * w[2])).madd(b2v, self.cl_r * w[1]);
        self.roll = Acc::adda(self.cl_da * aileron, roll).madd(self.cl_dr, rudder);
        self.pitch = Acc::adda(self.cm0 + self.cm_alpha * alpha, c2v * (self.cm_q * w[0])).madd(self.cm_de, elevator);
        if !(alpha.abs() <= self.cm_break_alpha) && alpha.abs() < F(2.0) * self.cm_break_alpha {
            self.pitch = self.pitch + alpha * (self.cm_stall * (alpha.abs() - self.cm_break_alpha));
        }

        let yaw_slow = Acc::adda(self.cn_beta * beta, b2v * (self.cn_r * w[1])).madd(self.cn_dr, rudder);
        let yaw_fast = Acc::mula(b2v, self.cn_p * w[2]).madd(self.cn_da, aileron);
        let half_rho_v = F(0.5) * self.density * v;
        let q = v * half_rho_v;
        let sq = self.area * q;
        let sq_slow = self.area * (yaw_speed * half_rho_v);
        let back = if v < 0.0 { F::ONE } else { F(-1.0) };
        let yaw = Acc::mula(yaw_slow, sq_slow).madd(yaw_fast, sq);
        let force = Vf::new((self.side * sq).to_f32(), (self.lift * sq).to_f32(), (sq * (self.drag * back)).to_f32(), 1.0);
        let moment = Vf::new(
            (self.chord * (self.pitch * sq)).to_f32(),
            (self.span * yaw).to_f32(),
            (self.span * (self.roll * sq)).to_f32(),
            1.0,
        );
        (force, moment)
    }
}

// --- the rigid body ----------------------------------------------------------------------------------

/// The quaternion derivative's scale as the integrator loads it: `(1/2, 1/2, 1/2, -1/2)`, the half of
/// `q' = q ⊗ (w, 0) / 2` with the scalar half's minus sign folded in.
const HALF: Vf = Vf([0.5, 0.5, 0.5, -0.5]);

/// The rigid body the plane flies on: the engine's generic dynamics body, not the bike's chassis.
///
/// Everything is in the world frame except the moment, the inverse inertia and the rates, which are
/// in the body's own (`x` pitch, `y` yaw, `z` roll). The force and the moment are accumulated over one
/// substep by the listeners and emptied by [`Body::end_substep`]; [`Body::integrate`] turns them into
/// motion in between. `w` lanes are kept as the original leaves them, because it stores whole
/// quadwords.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// The world steps the body (and the plane pushes gravity on it) only while this is set.
    pub enabled: bool,
    /// One over the mass. Zero pins the body in place: the linear step is skipped.
    pub inv_mass: F,
    /// What is kept of the velocity each substep, per axis (1 as shipped; the wreck lowers it).
    pub damping: Vf,
    /// Where the centre of mass was at the start of the last substep.
    pub last_position: Vf,
    /// The centre of mass.
    pub position: Vf,
    pub velocity: Vf,
    /// The force of the last substep, and this one's.
    pub last_force: Vf,
    pub force: Vf,
    /// The linear acceleration of the last substep (`xyz`; see [`Body::integrate`] for `w`).
    pub accel: Vf,
    /// A second force accumulator nothing in the plane's own listeners writes (the collision
    /// response's), emptied each substep, and the largest magnitude per axis it reached since the
    /// plane's preforce listener last cleared it.
    pub shock_peak: Vf,
    pub shock: Vf,
    /// The step the linear half last took: `(h, h^2/2, inv_mass)`. The angular half reads it, so a
    /// pinned body turns with the step it was last moved by.
    pub step: Vf,
    /// The inverse inertia, a diagonal in the body frame. Exactly `(0, 0, 0, 1)` pins the orientation.
    pub inv_inertia: Vf,
    /// What is kept of the rates each substep, per axis.
    pub ang_damping: Vf,
    /// Which way round the body was at the start of the last substep, and is now.
    pub last_quat: Vf,
    pub quat: Vf,
    /// The centre of mass in the body's frame, from the model's origin.
    pub cofg: Vf,
    /// The moment of this substep (body frame), and the last one's.
    pub moment: Vf,
    pub last_moment: Vf,
    /// The body-frame angular velocity.
    pub rates: Vf,
    /// The angular acceleration of the last substep (`xyz`).
    pub ang_accel: Vf,
    /// The moment's twin of [`Body::shock`] and its peak.
    pub twist_peak: Vf,
    pub twist: Vf,
}

/// `a.xyz + b.xyz`, `w` from `a`: how every accumulator is added to.
fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `v` turned into the body's frame by `q`: [`crate::chassis::turn`] with every pair of operands
/// swapped, which is the conjugate's rotation without the conjugate. `w` is `v`'s.
fn unturn(q: Vf, v: Vf) -> Vf {
    let acc = Vf::opmula(v, q);
    let acc = Vf::madd(acc, v, q.bc(Field::W));
    let t = Vf::opmsub(acc, q, v);
    let p = q.mul(v);
    let acc = Vf::opmula(t, q);
    let acc = Vf::madd(acc, q, p.bc(Field::X));
    let acc = Vf::madd(acc, q, p.bc(Field::Y));
    let acc = Vf::madd(acc, q, p.bc(Field::Z));
    let acc = Vf::madd(acc, t, q.bc(Field::W));
    v.merge(Dest::XYZ, Vf::opmsub(acc, q, t))
}

/// `q ⊗ (w, 0)` scaled by [`HALF`]: the vector half as an `vopmula`/`vopmsub` cross product with the
/// scalar term folded in, the scalar half as `-1/2` times each of the three products, accumulated.
fn spin(q: Vf, w: Vf) -> Vf {
    let p = q.mul(w);
    let acc = Vf::opmula(q, w);
    let acc = Vf::madd(acc, w, q.bc(Field::W));
    let v = Vf::opmsub(acc, w, q).mul(HALF);
    let s = vu::madd(vu::madd(vu::mul(HALF.w(), p.x()), HALF.w(), p.y()), HALF.w(), p.z());
    Vf::new(v.x(), v.y(), v.z(), s)
}

impl Body {
    /// A world-frame force onto this substep's accumulator (`xyz`). The original then tells the
    /// body's own listeners, of which the plane's body has none.
    pub fn add_force(&mut self, f: Vf) {
        self.force = add_xyz(self.force, f);
    }

    /// A body-frame force: turned into the world by the orientation, then [`Body::add_force`].
    pub fn add_force_local(&mut self, f: Vf) {
        self.add_force(turn(self.quat, f));
    }

    /// A body-frame moment, added as it is (the rates and the inertia are in the body frame too).
    pub fn add_moment_local(&mut self, m: Vf) {
        self.moment = add_xyz(self.moment, m);
    }

    /// The plane's gravity, once a substep before its other forces: a world-frame force the plane
    /// keeps (`(0, -32.2 m, 0)` as it builds it), pushed only while the body is enabled. The original
    /// runs the collision response straight after, which is not this module's.
    pub fn push_gravity(&mut self, gravity: Vf) {
        if self.enabled {
            self.add_force(gravity);
        }
    }

    /// The plane's preforce listener's part: the shock peak is cleared, once a frame.
    pub fn clear_shock_peak(&mut self) {
        self.shock_peak = CLEAR;
    }

    /// The linear half of a substep: a second-order Taylor step, `x += v h + a h^2/2`, then
    /// `v = a h + damping * v`, with `a = force * inv_mass`. The position is remembered first, even
    /// when the body is pinned.
    fn advance(&mut self, h: F) {
        self.last_position = self.position;
        if self.inv_mass == 0.0 {
            return;
        }
        let half_h2 = F(0.5) * h * h;
        self.step = Vf::new(h.0, half_h2.0, self.inv_mass.0, self.step.w());
        let s = self.step;
        let a = self.force.mul(s.bc(Field::Z));
        let d = a.mul(s.bc(Field::Y)).add(self.velocity.mul(s.bc(Field::X)));
        self.position = add_xyz(self.position, d);
        self.velocity = self.velocity.merge(Dest::XYZ, a.mul(s.bc(Field::X)).add(self.damping.mul(self.velocity)));
        self.accel = self.accel.merge(Dest::XYZ, a);
    }

    /// One substep of motion, the world's integrate call: [`Body::advance`], then the orientation.
    ///
    /// The orientation takes the same second-order step on the quaternion: `q' = q ⊗ (w, 0) / 2`, and
    /// `q'' = q ⊗ (alpha, 0) / 2 - q |q'|^2` (the second derivative of a unit quaternion with the
    /// rates held), `q += q'' h^2/2 + q' h`, then back onto the unit sphere by `vrsqrt`. The rates
    /// take `alpha h + ang_damping * rates` with `alpha = inv_inertia * moment`.
    ///
    /// The two accelerations are stored as whole quadwords whose `w` lanes are whatever the vector
    /// registers held: the linear one's a register nothing in the step writes, the angular one's the
    /// position's `w` once the linear half has run (and left as it was here otherwise).
    pub fn integrate(&mut self, h: F) {
        self.advance(h);
        self.last_quat = self.quat;
        if self.inv_inertia.to_bits() == CLEAR.to_bits() {
            return;
        }
        let (q, s) = (self.quat, self.step);
        let alpha = self.inv_inertia.mul(self.moment);
        let d = spin(q, self.rates);
        let dd = spin(q, alpha);
        let sq = d.mul(d);
        let n2 = vu::madd(vu::madd(vu::add(sq.w(), sq.x()), 1.0, sq.y()), 1.0, sq.z());
        let curl = dd.sub(q.mul(Vf::splat(n2)));
        let q = q.add(curl.mul(s.bc(Field::Y))).add(d.mul(s.bc(Field::X)));
        self.quat = normalize(q);
        self.rates = self.rates.merge(Dest::XYZ, alpha.mul(s.bc(Field::X)).add(self.ang_damping.mul(self.rates)));
        let w = if self.inv_mass == 0.0 { self.ang_accel.w() } else { self.position.w() };
        self.ang_accel = Vf::new(alpha.x(), alpha.y(), alpha.z(), w);
    }

    /// The world's second call per body per substep: each shock lane's peak kept by magnitude, the
    /// force and the moment remembered and emptied, and the two shock accumulators emptied.
    // The negated comparisons are the original's `c.ole.s` branched on false, which an unordered
    // pair takes too.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn end_substep(&mut self) {
        for k in 0..3 {
            if !(F(self.shock[k]).abs() <= F(self.shock_peak[k]).abs()) {
                self.shock_peak[k] = self.shock[k];
            }
        }
        self.last_force = self.force;
        self.force = CLEAR;
        self.shock = CLEAR;
        for k in 0..3 {
            if !(F(self.twist[k]).abs() <= F(self.twist_peak[k]).abs()) {
                self.twist_peak[k] = self.twist[k];
            }
        }
        self.last_moment = self.moment;
        self.moment = CLEAR;
        self.twist = CLEAR;
    }

    /// Where the model is put after the substeps: the orientation and the centre of mass as they were
    /// at the start of the *last* substep, the origin being that centre less the turned
    /// [`Body::cofg`]. Returns `(orientation, origin)`; the origin's `w` is the last position's.
    pub fn pose(&self) -> (Vf, Vf) {
        let offset = turn(self.last_quat, self.cofg);
        let p = self.last_position;
        (self.last_quat, p.merge(Dest::XYZ, p.sub(offset)))
    }
}

// --- the world ---------------------------------------------------------------------------------------

/// The engine's dynamics world, the two numbers its step reads off itself. The lists it walks are the
/// caller's ([`Dynamics`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct World {
    /// How many passes the constraint list gets per substep (1 in the game).
    pub iterations: i32,
    /// Substeps per second at least (115 in the game).
    pub rate: F,
}

/// The world's five lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum List {
    Bodies,
    Force,
    Constraints,
    Preforce,
    Post,
}

/// What the world step calls. Each call is one element of one list; the counts are read again after
/// every call, as the original's loops do, so a listener may grow its own list.
pub trait Dynamics {
    fn count(&self, list: List) -> i32;
    /// Once a frame, with the frame's `dt`.
    fn preforce(&mut self, i: i32, dt: F);
    /// Once a substep, with the substep's.
    fn force(&mut self, i: i32, h: F);
    /// A body is integrated only while this is set ([`Body::enabled`]).
    fn enabled(&self, body: i32) -> bool;
    /// [`Body::integrate`].
    fn integrate(&mut self, body: i32, h: F);
    /// [`Body::end_substep`].
    fn end_substep(&mut self, body: i32);
    /// `first` on the first of [`World::iterations`] passes.
    fn constrain(&mut self, i: i32, first: bool);
    /// Once a frame after the substeps; the plane's measure is one.
    fn post(&mut self, i: i32, dt: F);
}

impl World {
    /// How a frame is cut: `n = (int)(dt * rate) + 1` substeps of `dt / n`. At 30 frames a second and
    /// 115 that is four.
    pub fn substeps(&self, dt: F) -> (i32, F) {
        let n = (dt * self.rate).to_i32().wrapping_add(1);
        (n, dt / F(fpu::from_i32(n)))
    }

    /// One frame: the preforce listeners, then each substep's force listeners, every enabled body's
    /// integrate and end, and the constraint passes; then the post listeners. A world with no bodies
    /// does nothing at all, its listeners included.
    pub fn step(&self, dt: F, d: &mut impl Dynamics) {
        if d.count(List::Bodies) == 0 {
            return;
        }
        let mut i = 0;
        while i < d.count(List::Preforce) {
            d.preforce(i, dt);
            i += 1;
        }
        let (n, h) = self.substeps(dt);
        for _ in 0..n {
            let mut i = 0;
            while i < d.count(List::Force) {
                d.force(i, h);
                i += 1;
            }
            let mut i = 0;
            while i < d.count(List::Bodies) {
                if d.enabled(i) {
                    d.integrate(i, h);
                    d.end_substep(i);
                }
                i += 1;
            }
            let mut count = d.count(List::Constraints);
            if count > 0 {
                for pass in 0..self.iterations {
                    let mut i = 0;
                    while i < count {
                        d.constrain(i, pass == 0);
                        count = d.count(List::Constraints);
                        i += 1;
                    }
                }
            }
        }
        let mut i = 0;
        while i < d.count(List::Post) {
            d.post(i, dt);
            i += 1;
        }
    }
}

// --- the measure -------------------------------------------------------------------------------------

/// What the plane's measure listener works out after the substeps, for the next frame's listeners,
/// the animation and the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// Any of the three wheels is down, now and at the last measure.
    pub grounded: bool,
    pub was_grounded: bool,
    /// The velocity's length.
    pub airspeed: F,
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
    /// The orientation as three angles (the roll is `z`).
    pub euler: Vf,
    /// The orientation and the model's origin as a matrix.
    pub matrix: Mat4,
}

impl Default for Measure {
    fn default() -> Measure {
        let zero = Vf::default();
        Measure {
            grounded: false,
            was_grounded: false,
            airspeed: F::ZERO,
            alpha: F::ZERO,
            beta: F::ZERO,
            world_rates: zero,
            velocity: zero,
            forward: zero,
            up: zero,
            euler: zero,
            matrix: Mat4([zero; 4]),
        }
    }
}

/// `[-1, 1]` as two compares and two moves: anything unordered comes back as `1.0`.
fn clamp_unit(c: F) -> F {
    if c <= F::ONE {
        if c < F(-1.0) { F(-1.0) } else { c }
    } else {
        F::ONE
    }
}

/// The angle between the nose and the velocity in one plane: `acos(along / |(along, across)|)` off
/// the arc cosine table, the length through the approximate reciprocal square root and the cosine
/// clamped twice, negative when `across` is positive.
fn flow_angle(along: F, across: F, table: &AcosTable) -> F {
    let s = -across;
    let sum = Acc::mula(s, s).madd(along, along);
    let c = along * F::ONE.rsqrt(sum);
    let a = table.at(clamp_unit(clamp_unit(c)));
    if s < 0.0 { -a } else { a }
}

impl Measure {
    /// The measure's first half: the grounded flag from the three wheels, and the aero model's
    /// [`Aero::true_speed`], set while the plane is grounded or its collision record says it is
    /// touching something. Returns `(touchdown, true_speed)`; a touchdown (not grounded last time,
    /// grounded now) is when the original plays the wheels' touchdown sound.
    ///
    /// The original then animates the surfaces and runs the crash test, which may move the body,
    /// before [`Measure::flight`] reads it.
    pub fn contacts(&mut self, wheels_down: [bool; 3], touching: bool) -> (bool, bool) {
        self.was_grounded = self.grounded;
        self.grounded = wheels_down.iter().any(|&w| w);
        (!self.was_grounded && self.grounded, self.grounded || touching)
    }

    /// The second half, everything off the body: the airspeed, the body-frame velocity and the world
    /// rates, alpha (from the velocity's `y` and `z`) and beta (its `x` and `z`), the Euler angles,
    /// and the matrix with the forward and up rows copied out. The matrix's origin is the current
    /// centre of mass less the turned [`Body::cofg`].
    pub fn flight(&mut self, body: &Body, table: &AcosTable) {
        let v = body.velocity;
        let (x, y, z) = (F(v.x()), F(v.y()), F(v.z()));
        self.airspeed = Acc::mula(x, x).madda(y, y).madd(z, z).sqrt();
        self.velocity = unturn(body.quat, v);
        self.world_rates = turn(body.quat, body.rates);
        let along = F(self.velocity.z());
        self.alpha = flow_angle(along, F(self.velocity.y()), table);
        self.beta = flow_angle(along, F(self.velocity.x()), table);
        self.euler = quat_to_euler(body.quat, table);
        let p = body.position;
        let origin = p.merge(Dest::XYZ, p.sub(turn(body.quat, body.cofg)));
        self.matrix = quat_to_mat4(body.quat, origin);
        self.forward = self.forward.merge(Dest::XYZ, self.matrix.0[2]);
        self.up = self.up.merge(Dest::XYZ, self.matrix.0[1]);
    }

    /// What [`controls`] and [`autopilot`] read, off this measure and the body.
    pub fn attitude(&self, flown: bool, body: &Body) -> Attitude {
        Attitude {
            flown,
            grounded: self.grounded,
            nose_y: F(self.forward.y()),
            roll: F(self.euler.z()),
            speed: F(self.velocity.z()),
            pitch_rate: F(body.rates.x()),
        }
    }

    /// What [`Aero::forces`] reads, off this measure, the surfaces and the body.
    pub fn air(&self, s: &Surfaces, body: &Body) -> Air {
        Air {
            alpha: self.alpha,
            beta: self.beta,
            speed: F(self.velocity.z()),
            elevator: s.elevator_command,
            aileron: s.aileron,
            rudder: s.rudder,
            rates: body.rates,
        }
    }
}
