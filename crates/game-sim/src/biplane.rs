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
//! The rigid body, the world that steps it, the tires and the measure are not in this module; see the
//! journal entry for what is ported and what is still the original's. The crash test is, below.
//!
//! Every field is named by the `biplane.phy` key it is loaded from, where it has one. The loader is
//! S33's; until it lands the proofs read the values off a save state's live plane.
//!
//! # The crash
//!
//! The second half of the module is the part of the plane's frame that decides it has crashed and
//! puts it back, ported from `Biplane.cpp` and `MXBiplane` (research/oracle.txt, R8 and
//! S36) and proved against a playback of the human's biplane recording
//! (`tests/biplane.rs::the_biplane_crashes_and_respawns`). It runs on a [`Plane`] that carries only
//! the fields those functions read and write; S34's flight is to grow that struct rather than
//! replace it, and to call the functions below from the places the original calls them:
//!
//! * [`Plane::reset_chord`] from the input slot, before the forces. A respawn there is integrated on
//!   the same tick, which is why the capture's chord respawn shows a body already falling.
//! * [`Plane::wreck_check`] from the measure slot, after the surfaces are animated and **before**
//!   `airspeed` and `up_y` are recomputed: the test reads the previous tick's two.
//! * [`Plane::frame`] from the frame slot, which clears the chord's latch and runs the respawn clock.
//!
//! # What a crash is
//!
//! Two reasons, and only two ([`Wreck`]):
//!
//! * **Contact (1).** The body itself — not a tire — is touching something, and either the plane was
//!   faster than `MinWreckSpeed` (the file's 50 mph) on a surface without the `VEHICLE` material flag,
//!   or it is upside down. Ground, a wall and a building are all the same test: there is no reason of
//!   its own for water, and no wall test apart from the body's contact.
//! * **The map edge (13).** Within 150 ft of the terrain's edge the plane is thrown back toward the
//!   middle of the map on a ballistic arc with a random tumble, and wrecked.
//!
//! The wreck is the plane's own and not `Wreckbike.phy`'s: the body keeps flying under `[Wrecks]`'
//! damping, inertia scale and heavier gravity, the throttle and the engine are cut, and the prop is
//! drawn bent ([`Prop`]). After `WreckTime` seconds (reason 13: after it has been down for 2.5 s, then
//! `WreckTime`) the plane is unwrecked, which resets it: back to the spawn transform the track's
//! `machine.txt` placed it at, standing still, with the throttle and every surface at zero.
//!
//! The reset chord (L1 and R1, [`crate::ride::Controls::respawn`]) does the same at once, but only
//! for a plane that is on its wheels or wrecked.

#![deny(clippy::float_arithmetic)]

use crate::camera::CurveTable;
use crate::hash::{State, Walk};
use crate::motion::remap;
use crate::phy::{Ini, MPH};
use crate::rng::Rng;
use ps2_float::fpu::{self, Acc};
use ps2_float::{Dest, F, Vf, libm};

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

// --- the crash ---------------------------------------------------------------------------------------


/// The quadword the game's rodata holds for "nothing": a point at the origin.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
/// The rodata quadword an unwreck restores both dampings to.
const UNDAMPED: Vf = Vf::new(1.0, 1.0, 1.0, 1.0);

/// How close to the terrain's edge the map-edge test fires, in feet.
const EDGE_MARGIN: f32 = 150.0;
/// How much the terrain query multiplies its root-cell count by: sixteen cells a side, two levels.
const CELLS_PER_ROOT: f32 = 256.0;
/// The map-edge throw's horizontal speed is capped at this distance, flown in one arc.
const THROW_REACH: f32 = 2073.599_9;
/// The throw's arc: the time constant under the square root, and the two terms of the lift.
const THROW_G: f32 = 14.906_832;
const THROW_LIFT_RATE: f32 = 16.1;
const THROW_LIFT_REACH: f32 = 240.0;
/// The tumble the throw puts in, per axis, as a share of one random number in `[0, 1)`.
const THROW_SPIN: [f32; 3] = [0.5, 0.4, 0.7];
/// A wreck spins the body up by this much.
const WRECK_SPIN: f32 = 4.0;
/// How long a reason-13 plane must have been down before its `WreckTime` starts.
const EDGE_SETTLE: f32 = 2.5;
/// A reason-13 plane keeps this share of its velocity through the unwreck.
const EDGE_KEEP: f32 = 0.3;
/// Pounds to slugs as the plane's body is given its mass: 1/32.2 rounded the way the parts reader and
/// the collision record round it, through a multiply. `Weight / 32.2` would be one ulp heavier
/// (`div.s` rounds to nearest), and the captured gravity force shows the multiply.
const SLUGS_PER_POUND: f32 = 0.031_055_9;
/// The respawn clock's length, when something arms it ([`Plane::arm_respawn`]).
const RESPAWN_TIME: f32 = 10.0;

/// `[Wrecks]` off `Biplane\biplane.phy`: the five keys the plane's reader takes out of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wrecks {
    /// `WreckTime`: seconds from the wreck to the unwreck.
    pub wreck_time: F,
    /// `MinWreckSpeed`, written in miles per hour and held in feet a second (the reader scales it by
    /// [`MPH`] after everything else is read).
    pub min_wreck_speed: F,
    /// `WreckGravity`: the gravity scale while wrecked.
    pub wreck_gravity: F,
    /// `WreckDamping`, a quadword, into both of the body's dampings.
    pub wreck_damping: Vf,
    /// `WreckInertia`, a quadword: what the body's inertia is scaled by while wrecked.
    pub wreck_inertia: Vf,
}

impl Wrecks {
    /// Read the block out of the file's bytes.
    pub fn read_bytes(bytes: &[u8]) -> Wrecks {
        Wrecks::read(&Ini::parse_bytes(bytes))
    }

    /// Load `[Wrecks]` in the reader's order. A missing float leaves the destination alone, and the
    /// object's are zero before the read, so a missing one reads as zero; a missing quadword takes
    /// the rodata's origin point.
    pub fn read(ini: &Ini) -> Wrecks {
        let s = ini.section("Wrecks");
        Wrecks {
            wreck_time: s.float_or("WreckTime", F::ZERO),
            min_wreck_speed: s.float_or("MinWreckSpeed", F::ZERO) * F(MPH),
            wreck_gravity: s.float_or("WreckGravity", F::ZERO),
            wreck_damping: s.vector("WreckDamping", NOTHING),
            wreck_inertia: s.vector("WreckInertia", NOTHING),
        }
    }
}

/// Why the plane is wrecked: the value the original keeps in the plane's wreck word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Wreck {
    /// The body touched something too fast, or upside down.
    Contact = 1,
    /// The plane came within [`EDGE_MARGIN`] of the terrain's edge and was thrown back.
    MapEdge = 13,
}

/// Which prop model the plane shows: the blurred cone, or the bent one once it is wrecked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    /// `propconeblur`, textured `biplaneprop`.
    Blur,
    /// `propconebent`, textured `biplanepropbroken`.
    Bent,
}

/// A transform as the actor keeps it: a position and two axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub at: Vf,
    pub forward: Vf,
    pub up: Vf,
}

/// The rigid body, as far as the crash code touches it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The mass in slugs (`Weight` × [`SLUGS_PER_POUND`]).
    pub mass: F,
    pub at: Vf,
    pub forward: Vf,
    pub up: Vf,
    pub vel: Vf,
    /// The angular velocity.
    pub spin: Vf,
    /// The diagonal of the inertia tensor.
    pub inertia: Vf,
    /// The two dampings a wreck writes: the body's setters at `+0x1c` and `+0x54`, both given the
    /// same quadword. Which is linear and which angular is not established.
    pub damping: Vf,
    pub spin_damping: Vf,
}

/// What the body is touching this tick, as the crash test reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contact {
    /// The body (not a tire) is in contact.
    pub touching: bool,
    /// The surface it touches carries the `VEHICLE` material flag, which the speed rule spares.
    pub vehicle: bool,
}

/// The biplane, as far as S36 ports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub body: Body,
    /// The gravity force the body is given: `(0, −scale·m·g, 0, 1)`.
    pub gravity: Vf,
    /// The gravity scale an unwreck restores the force from.
    pub gravity_scale: F,
    /// A second scale the map-edge throw halves and its unwreck doubles back. Zero in the shipped
    /// state, and nothing in this module reads it.
    pub edge_scale: F,
    /// The throttle, and the engine's power, which a wreck cuts.
    pub throttle: F,
    pub engine_power: F,
    /// The surfaces a reset zeroes: elevator, elevator command, aileron, rudder.
    pub elevator: F,
    pub elevator_command: F,
    pub aileron: F,
    pub rudder: F,
    /// Any tire on the ground.
    pub grounded: bool,
    /// The speed and the up vector's `y`, both as the last measure left them.
    pub airspeed: F,
    pub up_y: F,
    /// The wreck word, the reason-13 "has been down" latch, and the seconds wrecked.
    pub wreck: Option<Wreck>,
    pub down: bool,
    pub clock: F,
    /// The inertia the body had when the file was read, which a wreck scales and an unwreck restores.
    pub inertia: Vf,
    /// Where `machine.txt` put the plane, and the actor's transform, which a reset copies it into.
    pub spawn: Placement,
    pub placed: Placement,
    /// A velocity the reset clears with the transform.
    pub kept_vel: Vf,
    /// The reset chord has fired this frame.
    pub latch: bool,
    /// The respawn clock, in seconds; idle at zero.
    pub respawn_clock: F,
    /// The base reset's flag on the body, set by every reset.
    pub fresh: bool,
}

impl Plane {
    /// A plane standing at its spawn: `weight` in pounds, `inertia` the body's diagonal, `gravity_scale`
    /// the scale its gravity force is built with.
    pub fn new(spawn: Placement, weight: F, inertia: Vf, gravity_scale: F) -> Plane {
        let mass = weight * F(SLUGS_PER_POUND);
        let body = Body {
            mass,
            at: spawn.at,
            forward: spawn.forward,
            up: spawn.up,
            vel: NOTHING,
            spin: NOTHING,
            inertia,
            damping: UNDAMPED,
            spin_damping: UNDAMPED,
        };
        Plane {
            body,
            gravity: gravity_force(gravity_scale, mass),
            gravity_scale,
            edge_scale: F::ZERO,
            throttle: F::ZERO,
            engine_power: F::ZERO,
            elevator: F::ZERO,
            elevator_command: F::ZERO,
            aileron: F::ZERO,
            rudder: F::ZERO,
            grounded: false,
            airspeed: F::ZERO,
            up_y: F::ONE,
            wreck: None,
            down: false,
            clock: F::ZERO,
            inertia,
            spawn,
            placed: spawn,
            kept_vel: NOTHING,
            latch: false,
            respawn_clock: F::ZERO,
            fresh: false,
        }
    }

    /// **The crash test**, once a tick from the measure slot.
    ///
    /// `extent` is the terrain's size ([`map_extent`]), `None` for a world with no terrain. The
    /// map-edge throw draws three numbers from `rng`, the game's own `rand`.
    pub fn wreck_check(&mut self, dt: F, w: &Wrecks, contact: Contact, extent: Option<(F, F)>, rng: &mut Rng) {
        if self.wreck != Some(Wreck::MapEdge) {
            if let Some(size) = extent.filter(|&s| near_edge(self.body.at, s)) {
                self.throw_back(size, rng);
                self.wreck(Wreck::MapEdge, w);
            }
        }
        match self.wreck {
            None => {
                let fast = contact.touching && !contact.vehicle && !(self.airspeed <= w.min_wreck_speed);
                if fast || (contact.touching && self.up_y < F::ZERO) {
                    self.wreck(Wreck::Contact, w);
                }
            }
            Some(reason) => {
                self.clock = self.clock + dt;
                match reason {
                    Wreck::MapEdge => {
                        if !self.down && F(EDGE_SETTLE) < self.clock && (self.grounded || contact.touching) {
                            self.down = true;
                            self.clock = F::ZERO;
                        }
                        if self.down && w.wreck_time < self.clock {
                            let v = self.body.vel;
                            self.body.vel = v.merge(Dest::XYZ, v.mul(Vf::splat(EDGE_KEEP)));
                            self.unwreck();
                        }
                    }
                    Wreck::Contact => {
                        if w.wreck_time < self.clock {
                            self.unwreck();
                        }
                    }
                }
            }
        }
    }

    /// The map-edge throw: toward the middle of the map, level with the plane, as fast as an arc of
    /// half the map (at most [`THROW_REACH`]) needs, plus the arc's lift, and a random tumble.
    fn throw_back(&mut self, (width, depth): (F, F), rng: &mut Rng) {
        let half = F(0.5);
        let mid_x = half * width;
        let mid_z = half * depth;
        let at = self.body.at;
        let target = Vf::new(mid_x.0, at.y(), mid_z.0, 1.0);
        let d = target.merge(Dest::XYZ, target.sub(at));
        let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
        let len2 = Acc::mula(x, x).madda(y, y).madd(z, z);
        let dir = if F::ZERO == len2 {
            NOTHING
        } else {
            let k = F::ONE.rsqrt(len2);
            Vf::new((x * k).0, (y * k).0, (z * k).0, d.w())
        };
        let root = F(2.0) * F(THROW_G).sqrt();
        let speed = F(THROW_REACH).min(mid_x) / root;
        let lift = F(THROW_LIFT_RATE) * root + F(THROW_LIFT_REACH) / root;
        let vel = dir.merge(Dest::XYZ, dir.mul(Vf::splat(speed.0)));
        self.body.vel = Vf::new(vel.x(), (F(vel.y()) + lift).0, vel.z(), vel.w());
        self.edge_scale = self.edge_scale * half;
        let sx = F(THROW_SPIN[0]) * rng.unit();
        let sy = F(THROW_SPIN[1]) * rng.unit();
        let sz = F(THROW_SPIN[2]) * rng.unit();
        self.body.spin = Vf::new(sx.0, sy.0, sz.0, 1.0);
    }

    /// **The wreck**: the reason kept, the body spun up and given `[Wrecks]`' damping, inertia and
    /// gravity, the throttle and the engine cut. (The player's pad rumble and the engine sound are
    /// the original's too, and are not simulation.)
    pub fn wreck(&mut self, reason: Wreck, w: &Wrecks) {
        let inertia = w.wreck_inertia.merge(Dest::XYZ, w.wreck_inertia.mul(self.inertia));
        self.wreck = Some(reason);
        self.down = false;
        let s = self.body.spin;
        self.body.spin = s.merge(Dest::XYZ, s.mul(Vf::splat(WRECK_SPIN)));
        self.body.damping = w.wreck_damping;
        self.body.spin_damping = w.wreck_damping;
        self.body.inertia = inertia;
        self.gravity = gravity_force(w.wreck_gravity, self.body.mass);
        self.throttle = F::ZERO;
        self.engine_power = F::ZERO;
    }

    /// **The unwreck**: everything [`Plane::wreck`] changed put back, then [`Plane::reset`]. The reset
    /// chord calls it on a plane that is not wrecked too, and it resets that one the same way.
    pub fn unwreck(&mut self) {
        if self.wreck == Some(Wreck::MapEdge) {
            self.edge_scale = self.edge_scale / F(0.5);
        }
        self.wreck = None;
        self.clock = F::ZERO;
        self.body.damping = UNDAMPED;
        self.body.spin_damping = UNDAMPED;
        self.body.inertia = self.inertia;
        self.gravity = gravity_force(self.gravity_scale, self.body.mass);
        self.reset();
    }

    /// **The reset**: the plane back at its spawn transform, standing still, with the throttle and the
    /// four surfaces at zero and the respawn clock idle.
    pub fn reset(&mut self) {
        self.placed = self.spawn;
        self.kept_vel = NOTHING;
        self.throttle = F::ZERO;
        self.elevator = F::ZERO;
        self.elevator_command = F::ZERO;
        self.aileron = F::ZERO;
        self.rudder = F::ZERO;
        self.body.at = self.placed.at;
        self.body.forward = self.placed.forward;
        self.body.up = self.placed.up;
        self.body.vel = NOTHING;
        self.body.spin = NOTHING;
        self.fresh = true;
        self.respawn_clock = F::ZERO;
    }

    /// **The reset chord**, from the input slot: `respawn` is [`crate::ride::Controls::respawn`], the
    /// same two commands the bike's chord is. Once a frame, and only on the wheels or wrecked.
    pub fn reset_chord(&mut self, respawn: bool) {
        if respawn && !self.latch && (self.grounded || self.wreck.is_some()) {
            self.latch = true;
            self.unwreck();
        }
    }

    /// Arm the respawn clock: the hook the original's machine-racer interface calls on the plane.
    /// `immediate` is that interface's mode 1, which resets at once instead.
    pub fn arm_respawn(&mut self, immediate: bool) {
        if immediate {
            self.respawn();
        } else {
            self.respawn_clock = F(RESPAWN_TIME);
        }
    }

    /// **The frame slot's share**: the chord's latch cleared, and the respawn clock run down.
    pub fn frame(&mut self, dt: F) {
        self.latch = false;
        if F::ZERO < self.respawn_clock {
            self.respawn_clock = self.respawn_clock - dt;
            if self.respawn_clock <= F::ZERO {
                self.respawn();
            }
        }
    }

    /// The respawn clock's end: the spawn transform put back and the plane reset, then the controls
    /// zeroed again.
    fn respawn(&mut self) {
        self.placed = self.spawn;
        self.kept_vel = NOTHING;
        self.reset();
        self.throttle = F::ZERO;
        self.elevator = F::ZERO;
        self.elevator_command = F::ZERO;
        self.aileron = F::ZERO;
        self.rudder = F::ZERO;
    }

    /// Which prop the plane shows.
    pub fn prop(&self) -> Prop {
        if self.wreck.is_some() { Prop::Bent } else { Prop::Blur }
    }

    /// The wreck word as the original holds it: 0, 1 or 13.
    pub fn wreck_word(&self) -> u32 {
        self.wreck.map_or(0, |r| r as u32)
    }
}

/// The gravity force for a scale and a mass: `(0, (−scale·m)·g, 0, 1)`, in the original's order.
fn gravity_force(scale: F, mass: F) -> Vf {
    let y = F(G) * (-scale * mass);
    Vf::new(0.0, y.0, 0.0, 1.0)
}

/// The terrain's size as the map-edge test computes it: root cells times 256 times the sample spacing,
/// across and along.
pub fn map_extent(cells_x: F, scale_x: F, cells_z: F, scale_z: F) -> (F, F) {
    (cells_x * F(CELLS_PER_ROOT) * scale_x, cells_z * F(CELLS_PER_ROOT) * scale_z)
}

/// The map-edge test: the point is within [`EDGE_MARGIN`] of the terrain's edge, or off it.
pub fn near_edge(at: Vf, (width, depth): (F, F)) -> bool {
    let m = F(EDGE_MARGIN);
    let (x, z) = (F(at.x()), F(at.z()));
    let inside = m <= x && x <= width - m && m <= z && z <= depth - m;
    !inside
}

impl State for Plane {
    fn walk(&self, w: &mut Walk<'_>) {
        w.group("body", |w| {
            w.f("mass", self.body.mass);
            w.vf("at", self.body.at);
            w.vf("forward", self.body.forward);
            w.vf("up", self.body.up);
            w.vf("vel", self.body.vel);
            w.vf("spin", self.body.spin);
            w.vf("inertia", self.body.inertia);
            w.vf("damping", self.body.damping);
            w.vf("spin_damping", self.body.spin_damping);
        });
        w.vf("gravity", self.gravity);
        w.f("gravity_scale", self.gravity_scale);
        w.f("edge_scale", self.edge_scale);
        w.f("throttle", self.throttle);
        w.f("engine_power", self.engine_power);
        w.f("elevator", self.elevator);
        w.f("elevator_command", self.elevator_command);
        w.f("aileron", self.aileron);
        w.f("rudder", self.rudder);
        w.bool("grounded", self.grounded);
        w.f("airspeed", self.airspeed);
        w.f("up_y", self.up_y);
        w.u32("wreck", self.wreck_word());
        w.bool("down", self.down);
        w.f("clock", self.clock);
        w.vf("inertia", self.inertia);
        w.vf("spawn_at", self.spawn.at);
        w.vf("spawn_forward", self.spawn.forward);
        w.vf("spawn_up", self.spawn.up);
        w.vf("placed_at", self.placed.at);
        w.vf("kept_vel", self.kept_vel);
        w.bool("latch", self.latch);
        w.f("respawn_clock", self.respawn_clock);
        w.bool("fresh", self.fresh);
    }
}
