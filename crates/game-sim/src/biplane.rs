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
//! rates — is measured by the plane's measure listener after every integration and read here as given.
//! The rigid body, the world that steps it, the tires, the measure and the crash test are not in this
//! module; see the journal entry for what is ported and what is still the original's.
//!
//! Every field is named by the `biplane.phy` key it is loaded from, where it has one. The loader is
//! S33's; until it lands the proofs read the values off a save state's live plane.

#![deny(clippy::float_arithmetic)]

use crate::camera::CurveTable;
use crate::math::basis;
use crate::motion::remap;
use crate::rng::Rng;
use crate::world::World;
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

// --- the wheels -------------------------------------------------------------------------------------
//
// Three tires (front left, front right, the tail wheel), each its own set of listeners on the dynamics
// world: a preforce one that forgets the last frame's contact, and two per substep, one that asks the
// world where the ground is and one that turns the answer into forces on the plane's body. They read
// the plane's drawn frame (its measure listener's, last frame's) and never each other.

/// The rigid body the wheels push on: the few fields they read, and the entries they push through.
///
/// The body itself is S38's. The order of the calls is the original's, and it matters: the strut reads
/// the velocity back after [`Body::add_velocity`] has changed it.
pub trait Body {
    /// Whether the body takes part at all (the body's first word).
    fn enabled(&self) -> bool;
    fn position(&self) -> Vf;
    fn velocity(&self) -> Vf;
    /// The world-frame force gathered so far this substep.
    fn force(&self) -> Vf;
    /// The orientation quaternion.
    fn orientation(&self) -> Vf;
    /// Move the body without telling anything: the strut's own write to the position.
    fn set_position(&mut self, at: Vf);
    /// An instant change of velocity.
    fn add_velocity(&mut self, v: Vf);
    /// A world-frame force through the centre of mass.
    fn add_force(&mut self, f: Vf);
    /// A world-frame force at a world point, its moment masked lane by lane.
    fn add_force_at(&mut self, f: Vf, at: Vf, mask: Vf);
    /// A body-frame moment.
    fn add_moment_local(&mut self, m: Vf);
}

/// What the wheels read off the plane each substep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mount {
    /// The plane's drawn frame, three rows and the position.
    pub frame: [Vf; 4],
    /// The angular velocity the measure keeps, crossed with the lever to the contact.
    pub rates: Vf,
    /// The airspeed.
    pub speed: F,
    /// The brake (the front wheels') and the tail wheel's steer.
    pub brake: F,
    pub steer: F,
}

/// The `(1, 1, 1, 1)` moment mask every wheel force goes through with.
const ALL_LANES: Vf = Vf::new(1.0, 1.0, 1.0, 1.0);
/// The low end of the strut's fade with the slope it stands on; a global in the original, only ever
/// read.
const FADE_FLOOR: f32 = 0.5;
/// How steep the ground under a strut can be before it starts to fade (the `y` of the normal in the
/// plane's frame).
const FADE_FROM: f32 = 0.5;
const FADE_TO: f32 = f32::from_bits(0x3f23_d70a);
/// How much of an over-compression the strut takes out of the position (of an excess that is always
/// zero by then; see [`Tire::strut`]).
const PUSH_OUT: f32 = f32::from_bits(0x3f66_6666);
/// Airspeed to the friction's scale.
const ROLL_SCALE: f32 = f32::from_bits(0x3dcc_cccd);

/// `vaddax` / `vmaddax` / `vmadday` / `vmaddz`, `.xyz`: a point through the frame's rows and its
/// position, `w` kept from the point.
fn place(m: &[Vf; 4], v: Vf) -> Vf {
    let acc = m[3].add(Vf::VF0.bc(Field::X));
    let acc = Vf::madd(acc, m[0], v.bc(Field::X));
    let acc = Vf::madd(acc, m[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m[2], v.bc(Field::Z)))
}

/// `vmulax` / `vmadday` / `vmaddz`, `.xyz`: a vector through three rows, `w` kept.
fn rows(m: &[Vf; 3], v: Vf) -> Vf {
    let acc = m[0].mul(v.bc(Field::X));
    let acc = Vf::madd(acc, m[1], v.bc(Field::Y));
    v.merge(Dest::XYZ, Vf::madd(acc, m[2], v.bc(Field::Z)))
}

/// The same through the rows transposed: a world vector into the frame. The original transposes in the
/// integer registers (`pextlw`, `pexcw`, `pextuw`, `pcpyld`, `pcpyud`); only the `xyz` lanes survive.
fn unrows(m: &[Vf; 3], v: Vf) -> Vf {
    let t: [Vf; 3] = core::array::from_fn(|k| Vf::new(m[0][k], m[1][k], m[2][k], 0.0));
    rows(&t, v)
}

/// `vmul.xyz`, `vadday.x`, `vmaddz.x` (by one), `vrsqrt`, `vmulq.xyz`: unit length, `w` kept.
fn unit(v: Vf) -> Vf {
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, vu::dot3(v.0, v.0))))
}

/// A world vector into the body's frame by its quaternion, `w` kept; the original's outer-product
/// sequence, operand for operand.
fn unrotate(q: Vf, n: Vf) -> Vf {
    let acc = Vf::madd(Vf::opmula(n, q), n, q.bc(Field::W));
    let t = Vf::opmsub(acc, q, n);
    let m = q.mul(n);
    let acc = Vf::madd(Vf::opmula(t, q), q, m.bc(Field::X));
    let acc = Vf::madd(acc, q, m.bc(Field::Y));
    let acc = Vf::madd(acc, q, m.bc(Field::Z));
    let acc = Vf::madd(acc, t, q.bc(Field::W));
    n.merge(Dest::XYZ, Vf::opmsub(acc, q, t))
}

/// The FPU's three-term dot product, `mula.s` / `madda.s` / `madd.s`.
fn dot(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// `v.xyz · s`, `w` kept (`vmulx.xyz` against a broadcast single).
fn scaled(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::splat(s.to_f32())))
}

/// `a.xyz + b.xyz`, `w` from `a`.
fn plus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `a.xyz − b.xyz`, `w` from `a`.
fn minus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// One of the three tires: its `biplane.phy` section and what it keeps between calls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tire {
    /// `StrutCompression`: how far the strut gives.
    pub max_compression: F,
    /// `MaxDecompressionRate`, per second.
    pub max_decompression_rate: F,
    /// `SpringCoefficient`.
    pub spring: F,
    /// The damping while the strut closes, and while it opens.
    pub bounce: F,
    pub rebound: F,
    /// `BodyTireCenter`, in the plane's frame.
    pub center: Vf,
    /// `TireRadius`, and one over it.
    pub radius: F,
    pub inv_radius: F,
    /// `MaxBrakingForce`, `SideForce`, `BrakeFriction`, `SteeringMoment`.
    pub max_braking: F,
    pub side_force: F,
    pub brake_friction: F,
    pub steering_moment: F,
    /// Whether the plane's brake reaches this wheel (the front two), and its steer (the tail wheel).
    pub braked: bool,
    pub steered: bool,

    /// From the hub to the bottom of the tire, toward the last ground, in the plane's frame.
    pub offset: Vf,
    /// The ground was at or above the bottom of the tire when the probe looked.
    pub below: bool,
    /// A contact held over from elsewhere; only ever cleared (see the journal).
    pub held: bool,
    /// The strut is compressed this substep.
    pub touching: bool,
    pub compression: F,
    pub last_compression: F,
    /// The strut's push along the normal, never negative.
    pub spring_force: F,
    /// The bottom of the tire in the plane's frame, and in the world.
    pub bottom: Vf,
    pub low: Vf,
    /// The surface the ground query found under it.
    pub surface: u8,
    /// The ground point and normal the query gave back.
    pub ground: Vf,
    pub normal: Vf,
    /// Where the hub-to-bottom line crosses the ground.
    pub contact: Vf,
    /// The hub in the world.
    pub hub: Vf,
    /// The body's velocity at the contact.
    pub contact_velocity: Vf,
    /// The wheel's angle and spin, for the drawing.
    pub angle: F,
    pub spin: F,
    /// The normal the next probe leans the tire toward.
    pub last_normal: Vf,
    /// The contact's velocity in the tire's frame, the friction in it, and the frame (side, up,
    /// forward).
    pub slip: Vf,
    pub friction: Vf,
    pub frame: [Vf; 3],
}

impl Tire {
    /// The preforce listener, once a frame: no contact yet.
    pub fn clear(&mut self) {
        self.below = false;
        self.held = false;
    }

    /// The first force listener, every substep: where the ground is under the bottom of the tire.
    ///
    /// The bottom is the hub less one radius toward the last ground normal, taken into the plane's frame
    /// with its sideways part dropped, so the tire is a disc and not a ball.
    pub fn probe(&mut self, world: &mut impl World, m: &Mount) {
        self.below = false;
        if self.held {
            return;
        }
        self.hub = place(&m.frame, self.center);
        let rows3 = [m.frame[0], m.frame[1], m.frame[2]];
        let mut lean = unrows(&rows3, self.last_normal);
        lean[0] = 0.0;
        self.offset = scaled(unit(lean), -self.radius);
        self.bottom = plus(self.center, self.offset);
        self.low = place(&m.frame, self.bottom);
        let (ground, normal) = world.ground(self.low);
        self.surface = world.surface(ground);
        self.ground = self.low.merge(Dest::XYZ, ground);
        self.normal = normal;
        self.last_normal = normal;
        if !(self.ground.y() < self.low.y()) {
            self.below = true;
        }
    }

    /// The second force listener, every substep: the strut and then the tire's friction.
    pub fn force(&mut self, body: &mut impl Body, m: &Mount, dt: F) {
        self.strut(body, m, dt);
        self.roll(body, m, dt);
    }

    /// The strut: how far the ground has pushed the tire up its line from the hub, faded on a slope, a
    /// spring and damper on that, and the result along the normal at the contact.
    ///
    /// Past full compression the original means to push the body out and kill its velocity and force
    /// into the ground, but it clamps the compression first and then takes the excess from the clamped
    /// value, so the position moves by `normal · 0`; the velocity and force parts do happen.
    pub fn strut(&mut self, body: &mut impl Body, m: &Mount, dt: F) {
        self.compression = F::ZERO;
        self.touching = false;
        if self.below || self.held {
            self.hub = place(&m.frame, self.center);
            self.bottom = plus(self.center, self.offset);
            self.low = place(&m.frame, self.bottom);
            let n = self.normal;
            let up = minus(self.hub, self.low);
            let along = dot(up, n);
            if !(along <= 0.0) {
                let t = dot(minus(self.ground, self.low), n) / along;
                if !(t <= 0.0) {
                    self.contact = plus(self.low, scaled(up, t));
                    let b = minus(self.contact, self.low);
                    self.compression = dot(b, b).sqrt();
                }
            }
            if !(self.compression <= 0.0) {
                self.touching = true;
                let mut flat = unrotate(body.orientation(), n);
                flat[2] = 0.0;
                let fade = remap(F(unit(flat).y()), F(FADE_FROM), F(FADE_TO), F(FADE_FLOOR), F::ONE);
                self.compression = self.compression * fade;
                if !(self.compression <= self.max_compression) {
                    self.compression = self.max_compression;
                    let excess = self.compression - self.max_compression;
                    let pos = body.position();
                    body.set_position(plus(pos, scaled(n, F(PUSH_OUT) * excess)));
                    body.add_velocity(scaled(n, -dot(body.velocity(), n).min(F::ZERO)));
                    body.add_force(scaled(n, -dot(body.force(), n).min(F::ZERO)));
                }
                let lever = minus(self.contact, body.position());
                let swing = Vf::opmsub(Vf::opmula(m.rates, lever), lever, m.rates);
                let vel = body.velocity();
                self.contact_velocity = plus(vel, swing);
                let closing = dot(self.contact_velocity, n);
                let damping = if closing <= 0.0 { self.rebound } else { self.bounce };
                self.spring_force = F::ZERO.max(self.spring * self.compression - damping * closing);
                body.add_force_at(scaled(n, self.spring_force), self.contact, ALL_LANES);
            }
        }
        if !self.touching {
            self.spring_force = F::ZERO;
            self.contact_velocity = Vf::VF0;
        }
        if self.compression < self.last_compression {
            let open = (self.last_compression - self.compression).min(self.max_decompression_rate * dt);
            self.compression = self.last_compression - open;
        }
        self.last_compression = self.compression.min(self.max_compression);
    }

    /// The tire's friction: side force against the contact's sideways slip, braking against its
    /// rolling, the tail wheel's steering moment, all scaled by the airspeed up to one; and the
    /// wheel's spin from the rolling.
    ///
    /// Off the ground the friction is zeroed but still applied, through last substep's frame at last
    /// substep's contact.
    pub fn roll(&mut self, body: &mut impl Body, m: &Mount, dt: F) {
        if !self.touching {
            self.slip = Vf::VF0;
            self.friction = Vf::VF0;
        } else {
            let b = basis(Vf::VF0, m.frame[2], self.normal, true, false);
            self.frame = [b.0[0], b.0[1], b.0[2]];
            self.slip = unrows(&self.frame, self.contact_velocity);
            let brake = if self.braked { m.brake } else { F::ZERO };
            let scale = (F(ROLL_SCALE) * m.speed).max(F(-1.0)).min(F::ONE);
            let braking = scale * brake.max(self.max_braking);
            self.friction[0] = (self.side_force * F(self.slip.x())).to_f32();
            self.friction[1] = 0.0;
            self.friction[2] = (self.brake_friction * braking).to_f32();
            if self.steered {
                let turn = self.steering_moment * (m.steer * scale);
                body.add_moment_local(Vf::new(0.0, turn.to_f32(), 0.0, 1.0));
            }
        }
        body.add_force_at(rows(&self.frame, self.friction), self.contact, ALL_LANES);
        self.spin = F(self.slip.z()) * self.inv_radius;
        self.angle = wrap(self.angle + self.spin * dt);
    }
}

/// The plane's collision force, every substep before anything else: gravity onto the body, then the
/// collision object's contact resolution (handed in; not ported, see the journal), and the
/// collision's pending word cleared. Nothing happens to a disabled body. Returns whether it ran.
pub fn collision_force<B: Body>(body: &mut B, gravity: Vf, resolve: impl FnOnce(&mut B)) -> bool {
    if !body.enabled() {
        return false;
    }
    body.add_force(gravity);
    resolve(body);
    true
}

// --- the crash test ---------------------------------------------------------------------------------

/// How close to the terrain's edge the plane may fly before it is thrown back.
pub const EDGE: f32 = 150.0;
/// The crash reasons the test sets: a contact, and the edge of the map.
pub const CRASHED: u32 = 1;
pub const THROWN: u32 = 13;
/// The throw: a fall of this many feet sets its time, the speed toward the centre is capped here, and
/// the upward part is `16.1·k + 240/k` for `k = 2·sqrt(fall)`.
const THROW_FALL: f32 = f32::from_bits(0x416e_8262);
const THROW_CAP: f32 = f32::from_bits(0x4501_9999);
const THROW_UP: f32 = f32::from_bits(0x4180_cccd);
const THROW_LIFT: f32 = 240.0;
/// The random spins a thrown plane is given, per axis, times a draw in `[0, 1)`.
const THROW_SPIN: [f32; 3] = [0.5, f32::from_bits(0x3ecc_cccd), f32::from_bits(0x3f33_3333)];
/// How long a thrown plane flies before it may count as landed, and what is left of its velocity when it
/// is put back.
const THROWN_FOR: f32 = 2.5;
const RECOVER_KEEP: f32 = f32::from_bits(0x3e99_999a);

/// The flyable area: the terrain's extent in `x` and `z`, each 256 times a count times a size (the
/// terrain's four floats, in that pairing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub width: F,
    pub depth: F,
}

impl Area {
    pub fn new(x_count: F, z_count: F, x_size: F, z_size: F) -> Area {
        Area { width: F(256.0) * x_count * x_size, depth: F(256.0) * z_count * z_size }
    }

    /// Whether a point is within `margin` of an edge, or past it. `NaN` counts as outside.
    pub fn outside(&self, at: Vf, margin: F) -> bool {
        let inside = |v: f32, extent: F| !(v < margin) && v <= extent - margin;
        !(inside(at.x(), self.width) && inside(at.z(), self.depth))
    }
}

/// What the crash test reads off the plane and its collision object.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// The collision object has a contact this frame.
    pub touching: bool,
    /// The contact's surface is one the plane's crash mask ignores for the speed test.
    pub ignored: bool,
    /// The measure's on-the-ground flag.
    pub grounded: bool,
    /// `|velocity|`, and the `y` of the plane's up row.
    pub speed: F,
    pub up_y: F,
}

/// The plane's crash state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Crash {
    /// 0 flying, [`CRASHED`] or [`THROWN`].
    pub reason: u32,
    /// A thrown plane has come down.
    pub landed: bool,
    /// `MinWreckSpeed`, and `WreckTime`.
    pub min_wreck_speed: F,
    pub wreck_time: F,
    /// Time since the crash.
    pub clock: F,
}

/// What a crash-test call asked the plane to do: wreck itself (S36's), and then perhaps recover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Crashed {
    pub wreck: Option<u32>,
    pub recover: bool,
}

impl Crash {
    /// The wreck's own bookkeeping, the part this module needs: the reason, and not landed. The rest of
    /// the wreck (the body's inertia and damping, the engine) is the caller's.
    fn wreck(&mut self, reason: u32, out: &mut Crashed) {
        self.reason = reason;
        self.landed = false;
        out.wreck = Some(reason);
    }

    /// The crash test, once a frame after the measure.
    ///
    /// Near the edge of the area a plane is thrown back: a velocity toward the centre (at the body's own
    /// height) and up, the gravity scale halved, three random spins, and the [`THROWN`] wreck. Then, for
    /// a plane still flying, a contact is a [`CRASHED`] wreck when it is faster than `MinWreckSpeed` on
    /// a surface that counts, or upside down on any. A crashed plane's clock runs; a thrown one first has
    /// to have been in the air 2.5 s and then be on something, and when the clock passes `WreckTime` it
    /// keeps 0.3 of its velocity and recovers.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        c: &Contact,
        area: &Area,
        position: Vf,
        velocity: &mut Vf,
        rates: &mut Vf,
        gravity_scale: &mut F,
        rng: &mut Rng,
        dt: F,
    ) -> Crashed {
        let mut out = Crashed::default();
        if self.reason != THROWN && area.outside(position, F(EDGE)) {
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
            *gravity_scale = *gravity_scale * 0.5;
            for (lane, spin) in THROW_SPIN.into_iter().enumerate() {
                rates[lane] = (F(spin) * rng.unit()).to_f32();
            }
            rates[3] = 1.0;
            self.wreck(THROWN, &mut out);
        }
        if self.reason == 0 {
            if c.touching {
                if !c.ignored && !(c.speed <= self.min_wreck_speed) {
                    self.wreck(CRASHED, &mut out);
                    return out;
                }
                if c.up_y < 0.0 {
                    self.wreck(CRASHED, &mut out);
                }
            }
            return out;
        }
        self.clock = self.clock + dt;
        if self.reason == THROWN {
            if !self.landed && !(self.clock <= THROWN_FOR) && (c.grounded || c.touching) {
                self.landed = true;
                self.clock = F::ZERO;
            }
            if self.landed && !(self.clock <= self.wreck_time) {
                *velocity = scaled(*velocity, F(RECOVER_KEEP));
                out.recover = true;
            }
        } else if self.reason == CRASHED && !(self.clock <= self.wreck_time) {
            out.recover = true;
        }
        out
    }
}
