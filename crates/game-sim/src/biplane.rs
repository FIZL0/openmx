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
//! Below those, the wheels: each of the three [`Tire`]s has a preforce listener ([`Tire::clear`])
//! and two force listeners ([`Tire::probe`], which asks the world's ground query where the ground is,
//! then [`Tire::force`], the strut and the friction), pushing on the body through the [`WheelBody`]
//! trait. Then the plane's [`collision_force`] (gravity, and the collision object's contact
//! resolution, handed in) and the [`Crash`] test that sets the wreck reason. The contact resolution is
//! not in this module; see the journal entries for what is ported and what is still the original's.
//! S36's crash and comeback are, below.
//!
//! Every field is named by the `biplane.phy` key it is loaded from, where it has one. The loader is
//! S33's; until it lands the proofs read the values off a save state's live plane.
//!
//! # The crash
//!
//! The section after the aerodynamics is the part of the plane's frame that decides it has crashed and
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

use crate::air::AcosTable;
use crate::camera::CurveTable;
use crate::chassis::{CLEAR, normalize, turn};
use crate::hash::{State, Walk};
use crate::math::{Mat4, basis, quat_to_euler, quat_to_mat4};
use crate::motion::remap;
use crate::phy::{Ini, MPH};
use crate::rng::Rng;
use crate::world::World as Ground;
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

/// The rigid body, as far as the crash code touches it (S38's [`Body`] is the whole one; S41 is to join them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrashBody {
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
    pub body: CrashBody,
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
        let body = CrashBody {
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
pub(crate) fn gravity_force(scale: F, mass: F) -> Vf {
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
pub(crate) fn unturn(q: Vf, v: Vf) -> Vf {
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

    /// A world-frame force at a world point: the moment of the lever from the centre of mass, turned
    /// into the body's frame and masked lane by lane, through [`Body::add_moment_local`], then the
    /// force itself through [`Body::add_force`] — the generic body's force-at-a-point entry.
    pub fn add_force_at(&mut self, f: Vf, at: Vf, mask: Vf) {
        let lever = at.sub(self.position);
        let arm = Vf::opmsub(Vf::opmula(lever, f), f, lever);
        self.add_moment_local(unturn(self.quat, arm).mul(mask));
        self.add_force(f);
    }

    /// An instant change of velocity (`xyz`), which the body's shock accumulator takes too.
    pub fn add_velocity(&mut self, v: Vf) {
        self.velocity = add_xyz(self.velocity, v);
        self.shock = add_xyz(self.shock, v);
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

/// The plane's own body is what its wheels push on.
impl WheelBody for Body {
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn position(&self) -> Vf {
        self.position
    }
    fn velocity(&self) -> Vf {
        self.velocity
    }
    fn force(&self) -> Vf {
        self.force
    }
    fn orientation(&self) -> Vf {
        self.quat
    }
    fn set_position(&mut self, at: Vf) {
        self.position = at;
    }
    fn add_velocity(&mut self, v: Vf) {
        Body::add_velocity(self, v);
    }
    fn add_force(&mut self, f: Vf) {
        Body::add_force(self, f);
    }
    fn add_force_at(&mut self, f: Vf, at: Vf, mask: Vf) {
        Body::add_force_at(self, f, at, mask);
    }
    fn add_moment_local(&mut self, m: Vf) {
        Body::add_moment_local(self, m);
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

// --- the wheels -------------------------------------------------------------------------------------
//
// Three tires (front left, front right, the tail wheel), each its own set of listeners on the dynamics
// world: a preforce one that forgets the last frame's contact, and two per substep, one that asks the
// world where the ground is and one that turns the answer into forces on the plane's body. They read
// the plane's drawn frame (its measure listener's, last frame's) and never each other.

/// The rigid body the wheels push on: the few fields they read, and the entries they push through.
///
/// The body itself is S38's. The order of the calls is the original's, and it matters: the strut reads
/// the velocity back after [`WheelBody::add_velocity`] has changed it.
pub trait WheelBody {
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    pub fn probe(&mut self, world: &mut impl Ground, m: &Mount) {
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
    pub fn force(&mut self, body: &mut impl WheelBody, m: &Mount, dt: F) {
        self.strut(body, m, dt);
        self.roll(body, m, dt);
    }

    /// The strut: how far the ground has pushed the tire up its line from the hub, faded on a slope, a
    /// spring and damper on that, and the result along the normal at the contact.
    ///
    /// Past full compression the original means to push the body out and kill its velocity and force
    /// into the ground, but it clamps the compression first and then takes the excess from the clamped
    /// value, so the position moves by `normal · 0`; the velocity and force parts do happen.
    pub fn strut(&mut self, body: &mut impl WheelBody, m: &Mount, dt: F) {
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
    pub fn roll(&mut self, body: &mut impl WheelBody, m: &Mount, dt: F) {
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
pub fn collision_force<B: WheelBody>(body: &mut B, gravity: Vf, resolve: impl FnOnce(&mut B)) -> bool {
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
pub struct CrashContact {
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
        c: &CrashContact,
        area: &Area,
        position: Vf,
        velocity: &mut Vf,
        rates: &mut Vf,
        gravity_scale: &mut F,
        rng: &mut Rng,
        dt: F,
    ) -> Crashed {
        self.step_in(c, Some(area), position, velocity, rates, gravity_scale, rng, dt)
    }

    /// [`Crash::step`] over a world that may have no terrain (`None`), which has no edge to be thrown
    /// back from: the rest of the test is the same.
    #[allow(clippy::too_many_arguments)]
    pub fn step_in(
        &mut self,
        c: &CrashContact,
        area: Option<&Area>,
        position: Vf,
        velocity: &mut Vf,
        rates: &mut Vf,
        gravity_scale: &mut F,
        rng: &mut Rng,
        dt: F,
    ) -> Crashed {
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
