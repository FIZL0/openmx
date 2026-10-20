//! The whole bike, assembled: one state, and one frame of it.
//!
//! Every module around this one ports a piece of the bike and takes the rest of it as given. `move.rs`
//! is the ground step with eleven callees hoisted out into a [`crate::r#move::Call`] list; `wheel.rs`
//! is the suspension and the tire forces with nine of its own; `chassis.rs` is the integrator with
//! eleven; `air.rs` is the preload, the flip and the airborne control; `phy.rs` reads the tuning off
//! the disc. Each of those is proved on its own against the original, and none of them can move a bike
//! by itself. **This module is the wiring**: one state that the four views of the chassis and the two
//! views of each wheel are projections of, and one frame that drives them in the order the original's
//! own frame drives them, answering each hoisted call where the original makes it.
//!
//! # The frame
//!
//! The measured order of the ridden actor's update is in `research/oracle.txt` (A5): the ground step
//! first, then the air state, then the animation and the ride state machine. So [`Ride::frame`] is
//!
//! 1. the input layer's rate limit, which turns what the pad says into what the bike is holding,
//! 2. the chassis' measure — [`chassis::measure`] with [`crate::math::quat_to_euler`]'s three angles,
//!    which is where `air.rs`' pitch and roll come from,
//! 3. the ground step, written out here in [`mv::step`]'s own order with the callees dispatched,
//! 4. the air state ([`air::contact`]), which decides whether the bike is airborne and for how long.
//!
//! Step 3 is the one that needs justifying, because it is `move.rs`' body written twice. The
//! alternative — call [`mv::step`] and replay the `Call` list it hands back — is wrong, and not subtly:
//! a contact found by `Call::Contacts` is read four lines later by `Call::TireForces` and again by the
//! next substep's `stand_upright`, so a list replayed after the fact runs a frame behind itself. What
//! keeps the copy honest instead is a test: `tests/ride.rs` drives this spine and [`mv::step`] from the
//! same state over a world that answers nothing, and asserts that both the call list and every word of
//! the five-object graph come out identical. A line that drifts out of `move.rs` fails there.
//!
//! # What is ported and what is this module's
//!
//! Ported, and running here for real: the ground step and the four rider controllers (`move.rs`), the
//! suspension ray and the tire forces (`wheel.rs`), the integrator (`chassis.rs`), the preload, the
//! backflip and the airborne control (`air.rs`), the sweep (`collide.rs`), the terrain query
//! (`track.rs`), the quaternion-to-Euler conversion and the transform's own rotate, unrotate, point
//! and unpoint (`math.rs`, `chassis.rs`), every parameter the `.phy` reader reads (`phy.rs`).
//!
//! This module's own, each marked where it is written and listed in the journal: the wheel's spin and
//! the forces the tire puts through its contact ([`spin_integrate`], [`surface_drag`]), the steering
//! hinge ([`hinge_force`]), the input layer's rate limiter ([`Held::step`]), the smooth noise the
//! balance controller's rider-wobble reads ([`noise`]), the
//! quaternion from an axis and an angle the integrator's correction needs, the collision bundle the
//! sweep is handed and how a sweep hit becomes a contact, and the terrain blend inside a cell
//! (`world.rs`). Nothing here invents a *constant*: every number comes off the disc through
//! [`phy::Phy`] or out of a ported function.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::{self, AcosTable};
use crate::camera::{self, CurveTable};
use crate::chassis::{self, Mat3};
use crate::collide::{Bundle, Probe, Record};
use crate::hash::{State, Walk};
use crate::math::{self, Mat4};
use crate::phy::{self, Phy};
use crate::r#move::{self as mv, Frame, Outside as _, Side};
use crate::tire;
use crate::trick;
use crate::wheel;
use crate::world::World;
use ps2_float::{Acc, Dest, F, Field, Vf, fpu, libm, vu};

/// World up, with `w` of one, as every module here holds it.
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);
/// The quadword the game's rodata holds for "nothing": a point at the origin.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// How many probes the collision bundle carries, and what each one is for — see [`Ride::bundle`].
const PROBES: usize = 4;

/// The rate the surface's two torques fade in at, per foot a second of the contact's own slide:
/// `wheel_surface_drag`'s own literal, which makes the brake and the rolling torque whole above
/// 3⅓ ft/s and nothing at a standstill. See [`surface_drag`].
const BRAKE_FADE_RATE: f32 = 0.3;

/// The integrator's speed limits, which the original takes from a vehicle setup this port does not
/// have. Wide enough that no ride reaches them: what they are for here is to keep
/// [`chassis::integrate`]'s clamp from reading a zero limit as a standstill.
const MAX_SPEED: f32 = 1000.0;
const MAX_ANG_SPEED: f32 = 100.0;

/// **The bike's own speed limit**, squared: 135 ft/s. The rigid body's constructor writes it, as this
/// literal, and nothing the bike's setup or its `.phy` reads changes it — the live chassis of a
/// freeride 125 holds it (P21). The clamp is on the chassis' velocity, which is 1.25× what the actor's
/// `bike_vel` columns show, so 135 here is 108 in a capture's `bike_vel`.
///
/// It is not what tops a 125 out on the flat: the original's runs out of torque in top gear at 120.
/// It is what a downhill or a launch off a ramp is held to.
const BIKE_MAX_SPEED_SQ: f32 = 18225.0;

/// **The rigid body's angular windows**: the constructor clamps each axis of the body-frame angular
/// velocity to `[-25, 25]` rad/s, and its angular speed limit is the sum of the three windows'
/// squares, 1875 — worked out there, before the `.phy` loader writes the yaw window over the middle
/// one, so a bike's limit stays 1875 whatever its `YawRateLimit` is. The live 125's chassis holds
/// `(25, 3, 25)` and 1875 (P21, P27).
const ANG_WINDOW: f32 = 25.0;

/// The chassis' angular windows and limit as the original's bike gets them: the constructor's, with
/// the file's `YawRateLimit` on the yaw axis.
///
/// A file without the key leaves the constructor's 25: the original's loader writes an uninitialised
/// stack word there instead, which no shipped bike reaches, and [`crate::phy`] reads the missing key
/// as zero — which the window would read as "no yaw at all".
fn bike_ang_limits(p: &Phy) -> (F, [F; 3], [F; 3]) {
    let w = [ANG_WINDOW; 3];
    let sq = F(w[2] * w[2] + w[1] * w[1] + w[0] * w[0]);
    let yaw = if p.mass.yaw_rate_limit == F::ZERO { F(ANG_WINDOW) } else { p.mass.yaw_rate_limit };
    let max = [F(w[0]), yaw, F(w[2])];
    (sq, max, [-max[0], -max[1], -max[2]])
}

// --- what the rider asks for -----------------------------------------------------------------------

/// What the pad says this frame, before the input layer's rate limit.
///
/// One struct for the whole bike: the ground step, the air state and the wreck test between them read
/// every field of it. The two sticks are `[-1, 1]`, the four analogue commands `[0, 1]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// The throttle.
    pub gas: F,
    /// The brake lever the rider is holding, which is both wheels unless the two are split.
    pub brake: F,
    pub front_brake: F,
    pub rear_brake: F,
    /// The clutch lever.
    pub clutch: F,
    /// Reverse, which is a command and not a gear: held, it pushes the bike backwards.
    pub reverse: F,
    /// The left stick: `x` leans and steers, `y` moves the rider fore and aft.
    pub stick: (F, F),
    /// The d-pad, which is what asks for a trick.
    pub dpad_y: bool,
    pub dpad_dir: i32,
    /// A stunt is being held.
    pub stunt: bool,
    /// The three trick buttons, in the order the trick decoder reads them: the group they make is
    /// one half of the gesture that names a stunt ([`trick::group`]).
    pub trick: [bool; trick::TRICK_BUTTONS],
    /// The d-pad, in the order the trick decoder polls it — left, right, up, down — which is the
    /// other half of that gesture and not the eight-way code [`Controls::dpad_dir`] carries.
    pub dpad: [bool; trick::DPAD],
    /// The rider is balancing on one wheel.
    pub balance: bool,
    /// Put the bike back on the start line.
    pub respawn: bool,
}

impl Controls {
    /// What one pad is asking for, through the ported input layer.
    ///
    /// **The binding is the port's own.** The original's front end builds a table of binding to
    /// command and that table is not ported (see [`Held`]), so which button is the throttle is a
    /// choice this project makes and not a fact about the disc; what is the disc's is everything
    /// downstream — the strengths and press timers come from [`crate::input::Pad`]'s ported poll, and
    /// the rate limit from the `.phy`. It lives here rather than in the shell so that a replay fixture
    /// and the shell derive the rider's hands from a pad the same way.
    pub fn from_pad(pad: &crate::input::Pad) -> Controls {
        use crate::input::{Axis, AxisRange, Control};
        let dpad = [Control::Up, Control::Right, Control::Down, Control::Left];
        Controls {
            gas: pad.strength(Control::Cross),
            brake: pad.strength(Control::Square),
            front_brake: pad.strength(Control::R2),
            rear_brake: F::ZERO,
            clutch: pad.strength(Control::L2),
            reverse: if pad.held(Control::R3) { F::ONE } else { F::ZERO },
            // Through the player's own range, ±0.3, which is what the ride's stick consumers carry
            // in the original (I7, proved on a playback of the human's recording) — not the main
            // menu's ±0.2 that [`crate::input::Pad::axis`] hands out.
            stick: (pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), pad.axis_through(Axis::LeftY, &AxisRange::PLAYER)),
            dpad_y: pad.held(Control::Up) || pad.held(Control::Down),
            // The eight-way code the trick table reads: odd for a single direction, and the game's
            // own order round the d-pad. A diagonal is not derived, because which of two held
            // directions wins is the front end's business.
            dpad_dir: dpad.iter().position(|c| pad.held(*c)).map_or(0, |i| i as i32 * 2 + 1),
            stunt: pad.held(Control::Circle),
            // The shipped pad's three trick buttons, in the order the decoder tests them. Which
            // host button each is remains the shell's choice; the *order* is the disc's, because it
            // is what numbers the six groups and so which of the 24 slots a gesture names.
            trick: [
                pad.held(Control::Circle),
                pad.held(Control::Triangle),
                pad.held(Control::R1),
            ],
            dpad: [
                pad.held(Control::Left),
                pad.held(Control::Right),
                pad.held(Control::Up),
                pad.held(Control::Down),
            ],
            balance: pad.held(Control::L3),
            // The reset chord, and unlike the rest of this binding it is the disc's (P20). The
            // original's hand reset is an event handler on two commands of its binding table, and the
            // shipped table binds them to L1 and R1: it fires on the press of either while the other
            // is held, so whichever goes down second is the tick the rider is put back. R1 is also the
            // third trick button above, in the original's table as well as here, so a trick held on R1
            // and an L1 tapped under it is a reset there too. L1 and R1 held from before, with no
            // press this frame, is not a chord.
            respawn: (pad.pressed(Control::L1) && pad.held(Control::R1))
                || (pad.pressed(Control::R1) && pad.held(Control::L1)),
        }
    }
}

/// The controls after the input layer's rate limit: what the bike is actually holding.
///
/// The original's input layer walks a table of bindings and moves each command towards what the pad
/// says at a rate the `.phy` gives (`[Joystick]`'s eight `…RateLimit` keys), so a stab at the throttle
/// is not a step. **The levers' walk is this module's** — the binding table is the front end's and is
/// not ported — but the rates are the disc's, and for the levers a rate of zero is read as "no limit"
/// rather than as "never move", because that is what a `.phy` with the key missing has to mean.
///
/// **The stick's is the original's** (P30), and it is three things, not one:
///
/// - [`Held::stick`] is the stick the bike reads raw: the player-range deflection with the d-pad
///   forcing an axis to ±1 ([`trick::stick`]), then each axis multiplied by its own magnitude, sign
///   kept. The squaring is the rate limiter's, done in place before anything else in the frame reads
///   it, so the air steps' gesture, the centre of gravity's shift and the balance all see `x·|x|` —
///   a stick at 0.85 is 0.72 to them, short of the backflip's 0.8.
/// - [`Held::lean`] walks towards that squared stick at `X`/`YAxisRateLimit` a second, or at
///   `X`/`YDPadRateLimit` on an axis the d-pad is holding ([`Held::digital`]), as
///   `now + min(max(want − now, −r·dt), r·dt)`. A rate of zero here never moves, as in the original.
///   This is the lean the steering, the wheelie and the stoppie read.
/// - [`Held::flick`] is the speed of the stick's last full flick, which the backflip reads as how
///   hard it was asked for — see [`Flick`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Held {
    pub gas: F,
    pub brake: F,
    pub front_brake: F,
    pub rear_brake: F,
    pub clutch: F,
    /// The stick, squared with its sign kept: `+0x628`/`+0x62c` after the rate limiter.
    pub stick: (F, F),
    /// Whether each axis of [`Held::stick`] came from the d-pad, which picks the lean's rate.
    pub digital: [bool; 2],
    /// The rate-limited lean, sideways and fore and aft.
    pub lean: (F, F),
    /// The fore-aft axis's flick speed, in stick travel a second: what `air.rs` reads as
    /// `stick_hold`.
    pub flick: F,
    /// The samples [`Held::flick`] is measured over.
    pub ring: Flick,
}

impl Held {
    /// One frame of the rate limit.
    pub fn step(&mut self, c: &Controls, j: &phy::Joystick, dt: F) {
        self.gas = toward(self.gas, c.gas, j.gas_rate_limit, dt);
        self.brake = toward(self.brake, c.brake, j.brake_rate_limit, dt);
        // One lever or two. S17 proved that `bike+0x650` is the front wheel's brake and `+0x654` the
        // rear's, and left open who writes them; a shell with one brake button writes both, so the
        // general brake is folded into each and a wheel's own lever can only ask for more.
        self.front_brake =
            toward(self.front_brake, c.front_brake.max(c.brake), j.brake_rate_limit, dt);
        self.rear_brake = toward(self.rear_brake, c.rear_brake.max(c.brake), j.brake_rate_limit, dt);
        // The clutch has a rate each way: in fast, out slow, which is how a launch is held.
        let clutch_rate =
            if self.clutch < c.clutch { j.clutch_in_rate_limit } else { j.clutch_out_rate_limit };
        self.clutch = toward(self.clutch, c.clutch, clutch_rate, dt);

        // The stick, as the player record's sampler and the bike's pad read leave it: the d-pad
        // over the deflection, axis by axis. The flick is measured on this, before the squaring.
        let raw = trick::stick(c.stick.0, c.stick.1, c.dpad);
        self.flick = self.ring.sample(raw.y, dt);
        self.digital = raw.digital;
        self.stick = (squared(raw.x), squared(raw.y));
        let rx = if raw.digital[0] { j.x_dpad_rate_limit } else { j.x_axis_rate_limit } * dt;
        let ry = if raw.digital[1] { j.y_dpad_rate_limit } else { j.y_axis_rate_limit } * dt;
        self.lean = (limit(self.lean.0, self.stick.0, rx), limit(self.lean.1, self.stick.1, ry));
    }
}

/// An axis times its own magnitude: the original's `x · x · (x < 0 ? −1 : 1)`.
pub fn squared(x: F) -> F {
    x * x * if x < F::ZERO { F(-1.0) } else { F::ONE }
}

/// The rate limiter's step, in its own order: the gap clamped below, then above, then added.
fn limit(now: F, want: F, r: F) -> F {
    let gap = want - now;
    let neg = F(fpu::neg(r.0));
    let gap = if neg < gap { gap } else { neg };
    now + if gap < r { gap } else { r }
}

/// How many samples the flick is measured over: the original's ring.
pub const FLICK_SAMPLES: usize = 32;

/// How many samples the original's ring takes a second: five a 30 Hz tick, counted in the human's two
/// recordings' save states (see [`Flick`]).
pub const FLICK_RATE: f32 = 150.0;

/// The magnitude past which a sample counts as the stick thrown to one side.
const FLICK_TRAVEL: f32 = 0.8;

/// The flick: how fast the stick last went from one side to the other.
///
/// The original's player record keeps a ring of the last 32 stick samples, each with the time it was
/// taken, and every tick scans it oldest first: a sample past ±0.8 is remembered (the latest one on
/// that side wins), and the next one past 0.8 on the *other* side gives a speed — the change in
/// deflection over the time between the two — if the remembered time is above zero. Each crossing
/// forgets the remembered sample, and the fastest crossing in the ring is the answer, zero if none.
/// The ring and the scan are the original's, comparison for comparison.
///
/// **The sampling is the port's own.** The original samples from a pad listener the engine calls
/// after the pad update, after the simulation, and from inside every DMA wait longer than 4 ms, each
/// sample stamped with the hardware timer and the axes read afresh — about five a 30 Hz tick, a number
/// that depends on how long the GS made the CPU wait. A full flick inside one tick so measures
/// anything from 54 to 469 in the original's own capture, and nothing deterministic can reproduce
/// that. This takes one reading a frame, stamped with the simulation's own clock (which starts at the
/// first frame's `dt`, not zero, because the scan ignores a remembered time of zero), and pushes it as
/// many times as the original would have sampled in that time ([`FLICK_RATE`], at least once), so
/// the ring spans the original's ~0.2 s. Duplicates change nothing in the scan but the span. On the
/// human's recording (`tests/air.rs`, `the_flick_is_the_originals_clock_aside`) that puts every flick
/// on the original's tick, with its sign, and gone again within a tick of the original's; a flick
/// across two ticks measures what the original's does (30.0 against 29.85), and one inside a tick
/// measures a whole tick's worth, the slowest the original could have measured it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Flick {
    /// The simulation's clock, the time of the last sample.
    pub now: F,
    /// The slot the next sample goes in, which is also the oldest.
    pub head: usize,
    pub at: [F; FLICK_SAMPLES],
    pub value: [F; FLICK_SAMPLES],
}

impl Flick {
    /// One frame's reading, `dt` after the last, and the ring scanned.
    pub fn sample(&mut self, value: F, dt: F) -> F {
        self.now = self.now + dt;
        let times = ((dt.0 * FLICK_RATE).round() as usize).clamp(1, FLICK_SAMPLES);
        for _ in 0..times {
            self.at[self.head] = self.now;
            self.value[self.head] = value;
            self.head = (self.head + 1) % FLICK_SAMPLES;
        }
        self.speed()
    }

    /// The fastest crossing in the ring, oldest sample first.
    pub fn speed(&self) -> F {
        let mut best = F::ZERO;
        let (mut then, mut from) = (F::ZERO, F::ZERO);
        for k in 0..FLICK_SAMPLES {
            let s = (self.head + k) % FLICK_SAMPLES;
            let v = self.value[s];
            if !(F(FLICK_TRAVEL) < v.abs()) {
                continue;
            }
            if then != F::ZERO && (v < F::ZERO) != (from < F::ZERO) {
                if F::ZERO < then {
                    let speed = (v - from) / (self.at[s] - then);
                    if best.abs() < speed.abs() {
                        best = speed;
                    }
                }
                then = F::ZERO;
                continue;
            }
            then = self.at[s];
            from = v;
        }
        best
    }
}

/// One of the flick ring's samples, for the walk: its time and its value.
struct Slot(F, F);

impl State for Slot {
    fn walk(&self, w: &mut Walk<'_>) {
        w.f("at", self.0);
        w.f("value", self.1);
    }
}

/// One command towards another at a rate. A rate of zero is no limit at all.
fn toward(now: F, want: F, rate: F, dt: F) -> F {
    if rate <= F::ZERO {
        return want;
    }
    let step = rate * dt;
    let gap = want - now;
    if gap > step {
        now + step
    } else if gap < -step {
        now - step
    } else {
        want
    }
}

// --- one wheel, in both its views ------------------------------------------------------------------

/// One wheel. `mv` is the ground step's view of it and `w` the wheel module's; between them they are
/// the one object the original has, so the overlapping fields are copied across at each boundary
/// rather than stored twice.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wheel {
    pub mv: mv::Wheel,
    pub w: wheel::Wheel,
    /// Which friction surface the contact landed on: the index of a [`phy::Surface`] in this wheel's
    /// tire. The original keeps it in the contact record at `+0x110`.
    pub surface: u8,
    /// The tire's own inverse inertia, from `TireInertia`, which the spin integration needs.
    pub inv_inertia: F,
    /// The axis a steered wheel turns about, in the chassis' frame: the fork's, from the shocks'
    /// node to the handlebars' ([`tire::steering_axis`]). The original keeps it at `+0x8c0`. Straight
    /// up until whoever has the model sets it.
    pub steer_axis: Vf,
    /// The tire's three force-model flags, which [`tire::force`] reads back the next substep.
    pub slip: tire::Slip,
}

impl Wheel {
    /// A wheel built from one half of a `.phy`.
    pub fn of(t: &phy::Tire, mass: F, mount: Vf, steered: bool) -> Wheel {
        let radius = t.tire_radius;
        let travel = t.strut_compression;
        Wheel {
            mv: mv::Wheel {
                max_steer_angle: t.max_steer_angle,
                radius,
                inv_radius: if radius == F::ZERO { F::ZERO } else { F::ONE / radius },
                steered,
                ..mv::Wheel::default()
            },
            w: wheel::Wheel {
                has_ground: true,
                mount,
                // The suspension line: straight down the chassis' own `y`, **one tire radius** long —
                // `wheel_set_tire_derived` builds the same line, from the strut's top at `+0x48` to
                // `-(wheel+0x214)`, and `wheel_suspension` scales the aim by `-(wheel+0x214)` every
                // step of its relaxation. The strut is how far the suspension may *compress*, which
                // is a different number and stays on `max_compression`.
                probe: Vf::new(0.0, -radius.0, 0.0, 0.0),
                probe_length: radius,
                ground_normal: UP,
                max_compression: travel,
                max_damper_speed: t.max_compression_rate,
                extension_rate: t.max_decompression_rate,
                spring_rate: phy::spring_rate(t.spring, mass),
                // The damping ratios the file gives are of *critical* damping, against the spring
                // rate the loader set last, which is `Spring`'s ([`phy::damping`]).
                damping_out: phy::damping(t.rebound_damping_ratio, mass, phy::spring_rate(t.spring, mass)),
                damping_in: phy::damping(t.bounce_damping_ratio, mass, phy::spring_rate(t.spring, mass)),
                steered,
                steer_damping: t.steering_damping,
                steer_inv_inertia: if t.steering_inertia == F::ZERO {
                    F::ZERO
                } else {
                    F::ONE / t.steering_inertia
                },
                max_steer_angle: t.max_steer_angle,
                min_steer_angle: -t.max_steer_angle,
                ..wheel::Wheel::default()
            },
            surface: 0,
            inv_inertia: if t.tire_inertia == F::ZERO { F::ZERO } else { F::ONE / t.tire_inertia },
            steer_axis: UP,
            slip: tire::Slip::default(),
        }
    }

    /// The ground step's view into the wheel module's, before the wheel module runs.
    ///
    /// The commanded steering angle and the hinge's own angle are **one word** in the original —
    /// `wheel+0x8ac`. The ground step's pose stores the command straight into it (the front wheel's
    /// lock times the rider's lean times the speed fade, and nothing else), the steered-but-not-
    /// balancing case of the wheel's own integration leaves it alone, and the balancing case
    /// integrates it under the hinge's torque. Two structs hold that word here, so the two are
    /// copied across each way round the ported call: [`mv::Wheel::steer_command`] in,
    /// [`wheel::Wheel::steer_angle`] out ([`Wheel::from_wheel`]). Copying it in unconditionally is
    /// the aliasing itself and not a decision: outside the pose the two always agree, because
    /// `from_wheel` has just written the hinge's answer back over the command.
    ///
    /// That write-back is also the answer to P11's worry about doing this per substep rather than
    /// once a frame — that a balancing bike's hinge would be overwritten by a command the pose never
    /// made. [`mv::pose`] returns before the steering write while balancing, so the command is never
    /// made then; `from_wheel` leaves the hinge's own integrated angle in it, and the copy back in is
    /// a no-op for as long as the bike is balanced.
    fn to_wheel(&mut self) {
        self.w.contact = self.mv.contact;
        self.w.balancing = self.mv.balancing;
        self.w.steered = self.mv.steered;
        self.w.spring_rate = self.mv.spring;
        self.w.max_steer_angle = self.mv.max_steer_angle;
        self.w.min_steer_angle = -self.mv.max_steer_angle;
        self.w.steer_angle = self.mv.steer_command;
    }

    /// And back, after it has. The four words the ground step reads — the contact flag, the
    /// contact normal both roll angles are measured off, and the two flags
    /// `Wheel::ray_hit`/`Wheel::swept_contact` sit in — `+0xd0` and `+0xd4`, the first two of
    /// [`mv::Fresh`]'s seven. The other five and the three quadwords are the suspension's own
    /// workings, which `move.rs` empties and never reads.
    fn from_wheel(&mut self) {
        self.mv.contact = self.w.contact;
        self.mv.contact_normal = self.w.contact_normal;
        self.mv.fresh.scalars[0] = self.w.ray_hit as u32;
        self.mv.fresh.scalars[1] = self.w.swept_contact as u32;
        // The fifth word, and the one the ground step does *not* read: the shared steering angle,
        // back out of the hinge. See [`Wheel::to_wheel`] — it is here so the two copies of
        // `wheel+0x8ac` cannot disagree, which is what the state hash would otherwise show twice.
        self.mv.steer_command = self.w.steer_angle;
    }
}

// --- the bike ---------------------------------------------------------------------------------------

/// The whole bike: every word of state one ride carries.
///
/// The chassis is held once, as [`chassis::Chassis`] — the object the original has — and the ground
/// step's, the wheel module's and the air module's views of it are built from it and written back
/// where each of those runs. Nothing is stored twice, so there is one answer to "what is the bike
/// doing" and the state hash is unambiguous.
///
/// Two states are compared through [`crate::hash`] rather than with `==`: four of the structs it
/// holds are other modules' and do not derive `PartialEq`, and the hash walk compares every word of
/// them anyway — including the lanes an `==` on a `Vf` would be happy about and a divergence would
/// not be.
#[derive(Clone, Debug, Default)]
pub struct Ride {
    /// The ground step's own fields on the bike: the clutch's ramp, the balance accumulators, the
    /// pitch integral, the peak g.
    pub mv: mv::Ride,
    pub chassis: chassis::Chassis,
    pub engine: mv::Engine,
    pub air: air::Air,
    pub front: Wheel,
    pub rear: Wheel,
    /// The chassis transform's origin, as the integrator leaves it: the body's frame is `quat` about
    /// this. Held because the transform is the engine's object and the port does not have one.
    pub origin: Vf,
    /// The chassis' transform's three rotation rows, as the last frame left them. The original's bike
    /// node *is* the chassis' transform — one object, which [`mv::no_hander`] rolls about its forward
    /// axis and the chassis' integration reads its orientation back from — so between the no-hander
    /// and the next integration these rows are ahead of [`chassis::Chassis::quat`] by the no-hander's
    /// roll, and they are what is drawn (P32).
    pub node: [Vf; 3],
    /// The controls after the rate limit.
    pub held: Held,
    /// Where the bike goes back to, and which way it faces there.
    pub spawn: Vf,
    pub spawn_facing: Vf,
    /// What the last sweep found, kept for the frame after it: the nearest prop's distance, which the
    /// ground step reads as `node_distance`.
    pub near: F,
    /// Whether the body itself is against something solid — a prop, a wall, the scene. `air.rs`
    /// reads it to know a landing is not the only way to be stopped.
    pub touching: bool,
    /// A trick is running. The original's airborne control and its wreck test both read one word on
    /// the bike for this (the trick clocks' "in progress"), and the clocks run *after* the ground
    /// step, so what a frame's air code sees is the previous frame's verdict. [`crate::rider`] is
    /// what writes it; a bare [`Ride`] with no trick machine leaves it false, which is the state of
    /// a rider that never asks for a stunt.
    pub stunt: bool,
}

impl Ride {
    /// A bike built off the disc, stood at a point and pointed a way.
    ///
    /// `mounts` are where the two wheels hang off the chassis, in the chassis' own frame. The original
    /// reads them off the bike's model — the `fwheel` and `rwheel` nodes — which is the renderer's
    /// side of the disc, so they are handed in rather than read here.
    pub fn new(p: &Phy, mounts: (Vf, Vf), at: Vf, facing: Vf) -> Ride {
        let mass = p.mass.mass;
        let inertia = p.mass.inertia;
        let inv = |v: f32| if v == 0.0 { F::ZERO } else { F::ONE / F(v) };
        let ang = bike_ang_limits(p);
        let mut ride = Ride {
            mv: mv::Ride::default(),
            chassis: chassis::Chassis {
                awake: 1,
                cofg: p.mv.cofg,
                inv_mass: if mass == F::ZERO { F::ZERO } else { F::ONE / mass },
                inv_inertia: Mat3::new(
                    Vf::new(inv(inertia.x()).0, 0.0, 0.0, 0.0),
                    Vf::new(0.0, inv(inertia.y()).0, 0.0, 0.0),
                    Vf::new(0.0, 0.0, inv(inertia.z()).0, 0.0),
                ),
                quat: chassis::CLEAR,
                free: [1; 3],
                ang_free: [1; 3],
                // The integrator's own limits, the original's: the speed (P21), and the angular
                // windows and limit (P27).
                max_speed_sq: F(BIKE_MAX_SPEED_SQ),
                max_ang_speed_sq: ang.0,
                ang_vel_max: ang.1,
                ang_vel_min: ang.2,
                ..chassis::Chassis::default()
            },
            // The engine as its own constructor leaves it, with the file's keys on top. Two of the
            // values the constructor writes are not `.phy` keys, so the loader cannot have written
            // them and a zeroed engine makes no torque at all: the clutch's launch multiplier (which
            // [`mv::clutch`] owns from the first frame the lever is out) and the scale on the torque
            // leaving the gearbox, both one. `research/oracle.txt`'s S15 block is where the
            // constructor's six ones are read off.
            engine: mv::Engine {
                clutch_torque_scale: F::ONE,
                torque_scale: F::ONE,
                ..p.engine
            },
            air: air::Air::default(),
            front: Wheel::of(&p.front, mass, mounts.0, true),
            rear: Wheel::of(&p.rear, mass, mounts.1, false),
            origin: NOTHING,
            node: [Vf::ZERO; 3],
            held: Held::default(),
            spawn: at,
            spawn_facing: facing,
            near: F::ONE,
            touching: false,
            stunt: false,
        };
        ride.place(at, facing);
        ride
    }

    /// Put the bike down at a point, pointed a way, stopped: the respawn, less the ride state
    /// machine's own clocks (S5's, and P3's to wire).
    pub fn place(&mut self, at: Vf, facing: Vf) {
        let m = math::basis(at, facing, UP, true, false);
        self.chassis.quat = math::mat4_to_quat(&m);
        self.origin = Vf::new(at.x(), at.y(), at.z(), 1.0);
        self.chassis.cofg_world = math::transform_point(&m, self.chassis.cofg);
        for v in [
            &mut self.chassis.vel,
            &mut self.chassis.ang_vel,
            &mut self.chassis.local_vel,
            &mut self.chassis.world_ang_vel,
            &mut self.chassis.force,
            &mut self.chassis.torque,
            &mut self.chassis.accel,
            &mut self.chassis.ang_accel,
            &mut self.chassis.impulse,
            &mut self.chassis.correction,
        ] {
            *v = Vf::ZERO;
        }
        self.chassis.speed = F::ZERO;
        self.chassis.pos_hist = Default::default();
        self.chassis.vel_hist = Default::default();
        self.chassis.ang_hist = Default::default();
        self.chassis.quat_hist = Default::default();
        self.node = [m.0[0], m.0[1], m.0[2]];
        self.air = air::Air::default();
        self.mv = mv::Ride::default();
        self.front.w.last_compression = F::ZERO;
        self.rear.w.last_compression = F::ZERO;
        self.front.mv.spin = F::ZERO;
        self.rear.mv.spin = F::ZERO;
    }

    /// Where the bike is: the chassis transform's origin, which is what a camera and a model ride on.
    pub fn position(&self) -> Vf {
        self.origin
    }

    /// The chassis' transform as a matrix, built from the quaternion it carries.
    pub fn transform(&self) -> Mat4 {
        held_transform(&self.node, self.chassis.quat, self.origin)
    }

    /// The bike's forward and up axes, as the original's `orientation_axes` reads them off the
    /// transform: rows 2 and 1.
    pub fn axes(&self) -> (Vf, Vf) {
        let (forward, up) = math::quat_axes(self.chassis.quat);
        (
            Vf::new(forward[0].0, forward[1].0, forward[2].0, 0.0),
            Vf::new(up[0].0, up[1].0, up[2].0, 0.0),
        )
    }
}

/// The tables the ride needs, which are built once and shared: the arc cosine curve the rider
/// controllers and the Euler conversion read, and the game's `{cos, sin}` table.
pub struct Tables {
    pub acos: AcosTable,
    pub curve: CurveTable,
}

impl Tables {
    pub fn build() -> Tables {
        Tables { acos: AcosTable::build(), curve: CurveTable::build() }
    }

    fn mv(&self) -> mv::Tables<'_> {
        mv::Tables { acos: &self.acos, curve: &self.curve }
    }
}

// --- the engine's side of the four controllers -----------------------------------------------------

/// What [`mv::step`]'s controllers ask the engine for, answered here.
///
/// Three of the four are arithmetic the port can do — the transpose, a matrix row, an orthonormal basis —
/// and the fourth, the smooth noise, is a table the engine builds at start-up that this port does not
/// have.
///
/// **The two frames are one transform** (P32). The original hangs the chassis off the bike's own
/// node — the bike's setup hands the chassis that node as its transform — so the no-hander's
/// [`mv::Outside::set_axes`] rolls the very transform the chassis integrates from, and the next
/// [`integrate`] reads the roll back into the body's quaternion. Both frames therefore answer from
/// `chassis`, which is taken at the top of the frame, refreshed after every integration, and written
/// by the no-hander between them; [`Ride::node`] carries it over to the next frame.
struct Out {
    chassis: Mat4,
}

impl Out {
    fn mat3(&self, _frame: Frame) -> Mat3 {
        Mat3::new(self.chassis.0[0], self.chassis.0[1], self.chassis.0[2])
    }
}

impl mv::Outside for Out {
    fn unrotate(&mut self, frame: Frame, v: Vf) -> Vf {
        chassis::unrotate(&self.mat3(frame), v)
    }

    fn forward(&mut self, frame: Frame) -> Vf {
        // `orientation_axes`' forward is the matrix' `z` row with a `w` of zero — and a frame with
        // nothing in it answers `(0, 0, 1, 0)`, which is what an all-zero node row comes to here.
        let row = self.mat3(frame).rows[2];
        let row = if row.to_bits()[0] == 0 && row.to_bits()[1] == 0 && row.to_bits()[2] == 0 {
            Vf::new(0.0, 0.0, 1.0, 0.0)
        } else {
            row
        };
        row.merge(Dest::W, Vf::ZERO)
    }

    fn noise(&mut self, t: F) -> F {
        noise(t)
    }

    fn set_axes(&mut self, forward: Vf, up: Vf) {
        self.chassis = set_axes(self.chassis.0[3], forward, up);
    }
}

/// The chassis' transform out of what the ride carries: the rows the last frame left in
/// [`Ride::node`] at `origin`, or — for a ride that has never been placed — the body's own quaternion.
fn held_transform(node: &[Vf; 3], quat: Vf, origin: Vf) -> Mat4 {
    if node[1].to_bits() == Vf::ZERO.to_bits() {
        return math::quat_to_mat4(quat, origin);
    }
    Mat4([node[0], node[1], node[2], origin])
}

/// The transform helper that sets a frame's axes, keeping where the frame is: the original hands its
/// forward and up straight to the orthonormaliser [`math::basis`] with both of that function's flags
/// set — orthogonalise, and keep the forward — and the frame's own position as the origin (P32, read
/// off the helper's disassembly; the call sites are in `research/oracle.txt`). So the forward axis the
/// no-hander hinges on survives exactly and the up is the axis that moves; row0 is `up × forward`,
/// which is the handedness P11 had to find by hand when this was the port's own Gram-Schmidt.
fn set_axes(at: Vf, forward: Vf, up: Vf) -> Mat4 {
    math::basis(at, forward, up, true, true)
}

/// The engine's shared one-dimensional smooth noise, which while the bike is
/// balancing is the rider's own unsteadiness.
///
/// **A stand-in.** The original reads tables built at start-up, which are not on the disc in a form
/// this port can read, so what is here is value noise with the same shape: `[-1, 1]`, continuous, and
/// a feature per unit of `t`. S17's proof of [`mv::balance`] hooks the real one on both sides, so the
/// controller around it is the original's and only the wobble's waveform is this.
pub fn noise(t: F) -> F {
    let whole = t.to_i32();
    let frac = t - F(fpu::from_i32(whole));
    let (a, b) = (noise_at(whole), noise_at(whole.wrapping_add(1)));
    // The smoothstep the name promises, so that the derivative is continuous at the knots too.
    let s = frac * frac * (F(3.0) - F(2.0) * frac);
    a + (b - a) * s
}

/// One knot of [`noise`]: an integer hashed into `[-1, 1]`.
fn noise_at(i: i32) -> F {
    let mut h = (i as u32).wrapping_mul(0x9e37_79b9) ^ 0x85eb_ca6b;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_f491);
    h ^= h >> 13;
    F(fpu::from_u32(h >> 9)) * F(NOISE_KNOT) - F::ONE
}

/// `1 / 2^22`, which takes the 23 bits [`noise_at`] keeps onto `0..2`.
const NOISE_KNOT: f32 = 2.384_185_8e-7;

/// The turn the contacts asked for, as a quaternion, through the SDK's axis-and-angle helper.
///
/// **A stand-in**, in the one respect that matters: the half angle's sine and cosine come from
/// [`libm`] and not from the SDK's own, so the last bit or two can differ. The shape — the axis
/// scaled by the sine, the cosine in `w` — is what a quaternion from an axis and an angle is, and the
/// axis arrives already normalised and already in the world, as `research/oracle.txt` records.
fn quat_from_axis(axis: Vf, angle: F) -> Vf {
    let half = angle * F(0.5);
    let s = F(libm::sinf(half.0));
    Vf::new(
        (F(axis.x()) * s).0,
        (F(axis.y()) * s).0,
        (F(axis.z()) * s).0,
        libm::cosf(half.0),
    )
}

// --- the integrator's given answers ----------------------------------------------------------------

/// How many times [`integrate`] may ask [`chassis::integrate`] what it wants to know.
///
/// Four is enough — the questions form a chain of three links (the quaternion's derivative, then the
/// correction turn, then the two measures off the transform the correction leaves), and each pass
/// settles one. The loop stops early when a pass changes nothing, and the cap is there so that a
/// state it cannot settle costs a bounded amount rather than hanging.
const GIVEN_PASSES: usize = 4;

/// The chassis integrated, with the seven answers it takes as given worked out for it.
///
/// [`chassis::integrate`] is the one ported function whose inputs cannot be filled in before it runs:
/// four of the seven are the results of calls it makes half way down its own body, over state it has
/// just written. So it is run on a *copy* first and asked what it wanted to know — the [`chassis::Call`]
/// list carries every argument — the answers are worked out, and it is run again. Nothing of its
/// arithmetic is duplicated here; the loop only answers.
///
/// Two of the answers are deliberately taken against the transform as it stood when the frame began,
/// because that is when the original asks: it rotates the centre of gravity several instructions
/// *before* it sets the orientation (both call sites are in `research/oracle.txt`'s P2 block), so
/// the origin the transform ends up at is built from the *previous* rotation. The last two calls are after the set and so are answered
/// against the new one.
///
/// `entry` is the transform the step is entered with — the body's own orientation as the last
/// integration set it, unless the no-hander has rolled it since, in which case the turn it made is
/// the difference [`chassis::integrate`] carries onto the quaternion (P32).
fn integrate(ch: &mut chassis::Chassis, dt: F, entry: &Mat4, origin: &mut Vf) -> Vec<chassis::Call> {
    let entry = *entry;
    let mut given = chassis::Given::default();
    for _ in 0..GIVEN_PASSES {
        let mut probe = ch.clone();
        let calls = chassis::integrate(&mut probe, dt, &given);
        let next = answers(&calls, &entry);
        if next == given {
            break;
        }
        given = next;
    }
    let calls = chassis::integrate(ch, dt, &given);
    for call in &calls {
        if let chassis::Call::SetTransform(_, o) = call {
            *origin = *o;
        }
    }
    calls
}

/// The seven answers, read off one pass' questions.
fn answers(calls: &[chassis::Call], entry: &Mat4) -> chassis::Given {
    let mut g = chassis::Given::default();
    // The orientation the transform ends the step at, which the last two calls go through. It is on
    // the `SetTransform` the pass made, so it is read out first.
    let mut after = Mat3::new(entry.0[0], entry.0[1], entry.0[2]);
    for call in calls {
        if let chassis::Call::SetTransform(q, _) = *call {
            let m = math::quat_to_mat4(q, Vf::ZERO);
            after = Mat3::new(m.0[0], m.0[1], m.0[2]);
        }
    }
    let before = Mat3::new(entry.0[0], entry.0[1], entry.0[2]);
    for call in calls {
        match *call {
            chassis::Call::PointCofg(v) => g.cofg_world = math::transform_point(entry, v),
            chassis::Call::MatrixQuat => g.matrix_quat = math::mat4_to_quat(entry),
            chassis::Call::QuatDeriv(q, w) => g.quat_deriv = math::quat_deriv(q, w),
            chassis::Call::CorrectionQuat(axis, angle) => {
                g.correction_quat = quat_from_axis(axis, angle)
            }
            chassis::Call::RotateCofg(v) => g.rotated_cofg = chassis::rotate(&before, v),
            chassis::Call::Unrotate(v) => g.local_vel = chassis::unrotate(&after, v),
            chassis::Call::Rotate(v) => g.world_ang_vel = chassis::rotate(&after, v),
            chassis::Call::FrameUpdate(_)
            | chassis::Call::QuatToEuler(_)
            | chassis::Call::SetTransform(_, _)
            | chassis::Call::TouchTransform => {}
        }
    }
    g
}

// --- the arithmetic the modules around this one each keep their own copy of ------------------------

/// Normalise `xyz` through VU0's *approximate* reciprocal square root, which is what every module in
/// this crate that normalises a vector uses. `w` comes through untouched.
fn normalize(v: Vf) -> Vf {
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, vu::dot3(v.0, v.0))))
}

/// `a × b`, as the original's `vopmula`/`vopmsub` pair. `w` is `a`'s.
fn cross(a: Vf, b: Vf) -> Vf {
    Vf::opmsub(Vf::opmula(a, b), b, a)
}

/// `v.xyz * s`, with `v`'s `w`.
fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::new(s.0, s.0, s.0, s.0).bc(Field::X)))
}

fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// The FPU's three-instruction dot product of two vectors' `xyz`, in `x`, `y`, `z` order.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// `v.xyz`'s length, through the same accumulator and the FPU's square root.
fn length(v: Vf) -> F {
    dot3(v, v).sqrt()
}

/// A *direction* through a matrix: its three rows, with the translation left out.
fn direction(m: &Mat4, v: Vf) -> Vf {
    chassis::rotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), v)
}

/// A world *point* back into the frame `m` describes, which is what `transform_unpoint` does: the
/// translation taken off and then the rotation's transpose.
fn unpoint(m: &Mat4, v: Vf) -> Vf {
    chassis::unrotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), sub_xyz(v, m.0[3]))
}

/// `[-1, 1]` as the port's own two compares, the shape every clamp in `move.rs` and `air.rs` has:
/// anything unordered comes back as `1.0`.
fn clamp_unit(c: F) -> F {
    if c <= F::ONE {
        if c < F(-1.0) { F(-1.0) } else { c }
    } else {
        F::ONE
    }
}

// --- what the tire does, which is this module's ----------------------------------------------------

/// The contact under one wheel, in the frame the two stand-ins below work in: the surface's normal,
/// the way the wheel is pointing along it and the way it is pointing across it, the load the
/// suspension is putting through it, and how fast the patch is sliding each way.
struct Patch {
    forward: Vf,
    right: Vf,
    /// The spring force [`wheel::tire_forces`] worked out, which is the load on the tire.
    load: F,
    /// The patch's own sliding speed: along the wheel, and across it.
    roll: F,
    side: F,
}

/// The contact under a wheel, measured. The wheel's forward comes off the chassis' own `z` axis,
/// flattened into the surface and then steered by the angle the hinge is holding — through
/// [`crate::camera::rotate`], which is the original's own `rotate_about_axis` (S7).
fn patch(wh: &Wheel, m: &Mat4, curve: &CurveTable) -> Patch {
    let n = wh.w.contact_normal;
    let along = direction(m, Vf::new(0.0, 0.0, 1.0, 0.0));
    let flat = normalize(sub_xyz(along, scale(n, dot3(along, n))));
    let forward = if wh.mv.steered { camera::rotate(wh.w.steer_angle, n, flat, curve) } else { flat };
    // Across the wheel. Which way round this points does not matter: every force below is
    // proportional to the slide measured along the same vector, so the sign cancels.
    let right = cross(n, forward);
    let v = wh.w.contact_velocity;
    Patch {
        forward,
        right,
        load: wh.w.spring_force,
        roll: dot3(v, forward),
        side: dot3(v, right),
    }
}

/// The wheel helper that measures its contact velocity: how fast the contact patch is moving —
/// the body's own velocity plus what the body's spin adds at the arm from its centre of mass.
///
/// **Close to the original, not proved against it.** The shape of a rigid body's velocity at a point
/// is not in doubt; the order of the three operations and whether the original takes the arm from the
/// centre of mass or from the transform's origin are, and neither is read off the disassembly here.
fn contact_velocity(vel: Vf, world_ang_vel: Vf, at: Vf, cofg_world: Vf) -> Vf {
    add_xyz(vel, cross(world_ang_vel, sub_xyz(at, cofg_world)))
}

/// What a tire's grip limit needs off the chassis, and nothing more: how much of a push it feels.
/// Copied out once a frame because neither number changes inside one.
#[derive(Clone, Copy)]
struct Body {
    inv_mass: F,
    inv_inertia: Mat3,
}

/// The mass the chassis presents to a push at `arm` along the unit direction `dir`, both in the
/// body's own frame: the inverse of `dir.M-1.dir + n.I-1.n` with `n = arm x dir`, which is the
/// effective mass of a constraint along `dir` at that point. `dir` being a unit vector makes the
/// first term the inverse mass itself.
///
/// The tensor goes through [`chassis::rotate`], which is the three instructions the chassis' own
/// integrator already puts a torque through its inverse inertia with. The tensor is diagonal in this
/// game's data; reading it as a matrix costs nothing and does not assume that.
fn effective_mass(body: &Body, arm: Vf, dir: Vf) -> F {
    let n = cross(arm, dir);
    let inv = body.inv_mass + dot3(n, chassis::rotate(&body.inv_inertia, n));
    if inv > F::ZERO { F::ONE / inv } else { F::ZERO }
}

/// A force into the chassis on a given arm: the force itself in the world, and the moment it makes
/// about the centre of mass in the body's own frame.
///
/// The arm is handed in rather than worked out from a point, because the two callers below do not
/// take theirs where [`wheel::tire_forces`] takes the suspension's. The suspension's force acts at
/// the load point and its arm is that point less the centre of gravity; the tire's drive and side
/// force acts on the arm the `.phy` gives and nothing else — see [`spin_integrate`].
fn push(c: &mut wheel::Chassis, m: &Mat4, force: Vf, arm: Vf) {
    let local_force = chassis::unrotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), force);
    c.force = add_xyz(c.force, force);
    c.torque = add_xyz(c.torque, cross(arm, local_force));
}

/// The wheel's surface drag: what the surface under it takes out of it — the rolling resistance
/// against the patch's slide, the surface's own constant rolling torque, and the brake.
///
/// **The two torques are the original's; the resistance force is still a stand-in.** The original's
/// own version is `wheel_surface_drag` in `research/oracle.txt`, and what it does with its three
/// per-surface numbers was read off it for P13:
///
/// - it builds one **signed fade** first, and both torques below are that fade times a number out
///   of the table: on the ground `0.3 * the patch's own forward slide`, in the air the wheel's
///   spin, each clamped to ±1. So a wheel rolling faster than 3⅓ feet a second — or spinning faster
///   than one radian a second in the air — gets the table's number whole, and under that it gets a
///   share of it that falls to nothing as the wheel stops. Nothing here can push a stopped wheel
///   backwards, which is the whole reason the fade is there;
/// - `MaxBrakingTorque` times the brake lever is that fade's number, whenever the lever is off its
///   stop;
/// - and it goes onto the **driven-wheel torque accumulator** — the same one the engine adds to
///   through `wheel_add_drive_torque` — and not onto the spin. That is what P13 came for: a torque in
///   that accumulator becomes a force at the contact patch in [`spin_integrate`], capped by the grip
///   the load allows, and *that* is what slows the bike down. Taken off the spin instead it did
///   nothing at all, because a wheel on the ground has its spin overwritten by the road in the same
///   substep: the brake could not stop the bike, and the only thing decelerating it was whatever the
///   engine's own braking happened to put through the clutch.
///
/// **`WheelFriction`'s own torque is still not applied.** The original takes `-fade * WheelFriction`
/// off the same accumulator, unconditionally, and that is one line. It is left out here on purpose:
/// this crate reads that key as the tire's *grip* limit (P4's stand-in in [`spin_integrate`]), and
/// the test bike's value was written for that reading, so applying it as a torque as well spends a
/// number twice. On the test bike it costs seven feet a second of top speed and moves the minute's
/// jump, which is a recalibration of P10's fixture and not this task's. The journal entry says so.
///
/// ## The resistance is a force and not a couple
///
/// The original's own version adds its drag to the chassis' force accumulator and **never touches
/// the torque accumulator** — the only writes it makes outside the wheel are that one force and the
/// two torques above (P8's journal entry has the address and the read). So the drag does not pitch
/// the bike at all, whatever the slide is, and this does not either. Its *shape* is still this
/// module's: the original's is `WheelDrag * v²` and is not scaled by the load, where this is
/// proportional to the load and to the slide up to one unit a second. Changing that is a
/// recalibration of every speed in the tests and not P13's; the journal entry says so.
fn surface_drag(wh: &mut Wheel, pt: &Patch, surf: &phy::Surface, brake: F, c: &mut wheel::Chassis) {
    // The fade, and with it the sign: everything the surface takes out of the wheel opposes
    // whichever way the wheel is going.
    let fade = if wh.w.contact {
        clamp_unit(F(BRAKE_FADE_RATE) * pt.roll)
    } else {
        clamp_unit(wh.mv.spin)
    };
    if brake > F::ZERO {
        let torque = (brake * surf.max_braking_torque) * fade;
        wh.mv.drive_torque = wh.mv.drive_torque - torque;
    }

    if !wh.w.contact {
        return;
    }
    let drag = surf.wheel_drag * pt.load * clamp_unit(pt.roll);
    c.force = add_xyz(c.force, scale(pt.forward, -drag));
}

/// The wheel's spin integration: the wheel's own spin, and what the tire puts through its
/// contact patch.
///
/// **A stand-in**, and the largest one in this module. Its numbers are all the disc's — the
/// surface's `SideForce`, `MaxDriveForce`, `MaxSideForce` and `WheelFriction`, the tire's
/// `TireInertia` and `TireRadius` — and the engine's torque reaches it through
/// [`mv::Wheel::drive_torque`], which the ported [`mv::engine_update`] filled in. What is this
/// module's is the model:
///
/// - A wheel on the ground rolls: its spin is the patch's own speed over the radius. What it can put
///   down along itself is the drive torque turned into a force at the patch, capped by the surface's
///   drive limit and then by the grip the load allows; whatever the grip would not take spins the
///   wheel up instead, which is wheelspin.
/// - Across itself it is a tire and not a damper: the slide times the surface's own cornering
///   stiffness, saturated at the surface's side limit — less whatever the donut command asked for,
///   which is how the back end is let go.
/// - A wheel in the air just spins up under the engine's torque.
///
/// ## The four grip limits are per unit load
///
/// `SideForce`, `MaxSideForce`, `MaxDriveForce` and `WheelFriction` are coefficients and not pounds:
/// on the game's own surfaces they are single digits, where a wheel of the bike carries a couple of
/// hundred pounds. Taken as pounds they are a grip circle of a few pounds against a bike of several
/// hundred — a bike with no sideways grip at all, which is the shape of the defect this function came
/// for (P4's journal entry has the numbers; they are the disc's and so are not written here). Each is
/// multiplied by the load the suspension is putting through the patch.
///
/// ## Why the side force cannot run away
///
/// A cornering stiffness is a spring against a *velocity*, so integrating it explicitly has a
/// stability limit: the port's two-step integrator goes unstable once the force can reverse the
/// slide inside one substep, and what that looks like from the pad is a bike that will not stand
/// still and spins out on its own.
///
/// So the force is held to the one that stops the slide dead and no further: friction may null a
/// slide, never reverse it. The bound is `m * |slide| / dt` with `m` the mass the chassis actually
/// presents to a push there ([`effective_mass`]) — a statement about friction rather than a damping
/// constant chosen by eye, and it is the same argument [`surface_drag`] already makes for the brake.
/// On the game's own numbers it never binds; it is what makes every surface in the data safe rather
/// than the ones that happen to have been driven over.
///
/// The arm the bound is taken on stays the contact patch's, even though the force goes in on the
/// arm below: the slide being nulled is the *patch's*, and the patch is where a push moves it most.
/// Measuring the bound anywhere else makes it looser — a force on the `.phy`'s own arm shifts the
/// patch about a quarter as much per pound, so the impulse that would null the slide from there is
/// four times larger. Loose is not safe here, so the tighter of the two is kept.
///
/// It also sets the two fields [`mv::engine_update`] reads back: the forward contact force, and
/// whether the spin is the torque's or the ground's.
///
/// ## Where the force acts on the chassis is a `.phy` key, not the contact patch
///
/// This is the one thing in the function that is **not** a stand-in: the original's own version puts
/// the force into the chassis' force accumulator and the couple into its torque accumulator on an
/// arm it loads straight out of the wheel, and that arm is `BodyTireForcePosition` — the per-tire
/// vector the loader writes there and that nothing else in the wheel reads. It is used exactly as
/// it stands, as an arm about the centre of gravity, with no contact point and no centre of gravity
/// subtracted from it. P8's journal entry has the two addresses and the loads.
///
/// That matters more than it looks. The patch sits about two feet under the centre of gravity, so a
/// drive force taken there turns into a couple of two foot-pounds per pound and loops the bike over
/// backwards the instant the throttle opens — which is what P8 came for. On the arm the file gives,
/// the same force makes a couple the file's author chose, and full throttle from a standstill
/// drives the bike forward instead of standing it up.
fn spin_integrate(wh: &mut Wheel, pt: &Patch, surf: &phy::Surface, body: &Body, dt: F,
                  c: &mut wheel::Chassis, m: &Mat4, at: Vf, brake: F, parametric: bool,
                  curve: &CurveTable) {
    let drive_torque = wh.mv.drive_torque;
    // The engine accumulates onto this every substep and nothing else empties it.
    wh.mv.drive_torque = F::ZERO;

    if !wh.w.contact {
        wh.mv.spin = wh.mv.spin + drive_torque * wh.inv_inertia * dt;
        wh.mv.spin_from_torque = true;
        wh.mv.contact_force_fwd = F::ZERO;
        return;
    }

    let grip = surf.wheel_friction * pt.load;
    let drive_cap = surf.max_drive_force * pt.load;
    let drive = (drive_torque * wh.mv.inv_radius).clamp(-drive_cap, drive_cap);
    let along = drive.clamp(-grip, grip);

    if !parametric {
        let side_cap = (surf.max_side_force * (F::ONE - wh.mv.donut_scale)).max(F::ZERO) * pt.load;
        let arm = sub_xyz(unpoint(m, wh.w.load_point), c.cofg);
        let dir = chassis::unrotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), pt.right);
        let hold = effective_mass(body, arm, dir) * pt.side.abs() / dt;
        let cap = side_cap.min(grip).min(hold);
        let across = (-pt.side * surf.side_force * pt.load).clamp(-cap, cap);
        push(c, m, add_xyz(scale(pt.forward, along), scale(pt.right, across)), at);
    } else {
        tire_side(wh, pt, surf, c, m, at, drive_torque, along, brake, curve);
    }

    // Rolling, plus whatever the grip would not take: the spin the road asks for, and the slip the
    // tire's own inertia lets it run away with.
    let slip = ((drive - along) * wh.mv.radius) * wh.inv_inertia * dt;
    wh.mv.spin = pt.roll * wh.mv.inv_radius + slip;
    wh.mv.spin_from_torque = false;
    wh.mv.contact_force_fwd = along;
}

/// The tire's push for a tire with `UseParametricCurve` — every tire on the disc — as the original
/// makes it (P33): the [`tire::contact_frame`] squared to the ground's normal and steered about the
/// fork, the contact's velocity taken into it, and [`tire::force`]'s side force, turned back out
/// through the frame with the forward force beside it and put in on the `.phy`'s arm.
///
/// The forward force is still [`spin_integrate`]'s own (`along`), not the law's: the law's forward
/// lane is the drive torque over the radius with no `WheelFriction` cap, and the spin model the
/// stand-in pairs it with reads that key as the grip. [`tire::force`] still runs whole, because its
/// spinning and locked flags pick the side curve, and it is handed the drive torque the original
/// would be.
///
/// There is no hold bound: the original has none, and the curve rounds off.
fn tire_side(wh: &mut Wheel, pt: &Patch, surf: &phy::Surface, c: &mut wheel::Chassis, m: &Mat4,
             at: Vf, drive_torque: F, along: F, brake: F, curve: &CurveTable) {
    // Across, the original's own law, in the original's own frame (P33): the contact frame squared
    // to the ground's normal, the velocity taken into it, and the per-surface law's side force.
    let rows = Mat3::new(m.0[0], m.0[1], m.0[2]);
    let (frame, local) = tire::contact_frame(m.0[2], wh.mv.steered, wh.w.contact, wh.w.steer_angle,
                                             wh.steer_axis, wh.w.contact_normal,
                                             wh.w.contact_velocity, curve,
                                             &mut |v| chassis::rotate(&rows, v));
    let contact = tire::Contact {
        load: pt.load,
        drive_torque,
        inv_radius: wh.mv.inv_radius,
        donut: wh.mv.donut_scale,
        brake,
        parametric: true,
        velocity: local,
    };
    let (across, _) = tire::force(surf, &contact, &mut wh.slip);
    let local_force = Vf::new(across.0, 0.0, along.0, 0.0);
    push(c, m, chassis::rotate(&Mat3::new(frame.0[0], frame.0[1], frame.0[2]), local_force), at);
}

/// The steering hinge's force: a steered wheel the bike is balancing on puts its suspension
/// force through the steering hinge rather than straight into the chassis.
///
/// **A stand-in.** The chassis still gets the force — it is what holds the bike up, and the original
/// applies it through the hinge's own frame — and the hinge gets the moment about the steering axis,
/// which here is the chassis' own `y`. The original's steering frame is at `+0x8d0` and is not
/// ported, so the axis is this module's reading.
fn hinge_force(wh: &mut Wheel, c: &mut wheel::Chassis, m: &Mat4, force: Vf, at: Vf) {
    let local_force = chassis::unrotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), force);
    let arm = sub_xyz(unpoint(m, at), c.cofg);
    let moment = cross(arm, local_force);
    c.force = add_xyz(c.force, force);
    c.torque = add_xyz(c.torque, moment);
    wh.w.steer_torque = wh.w.steer_torque + F(moment.y());
}

// --- the collision bundle ---------------------------------------------------------------------------

/// The probes one frame sweeps against everything solid.
///
/// **This composition is the port's.** The original builds its bundle out of the vehicle's own
/// attachment points through the collision search object, which is not ported; what it does with the
/// answers — the contact distributor, the record's three leaf rules — is. Four probes, chosen to
/// cover what the ride reads back out of the record:
///
/// - each wheel's suspension line, so that a contact on a prop reaches
///   [`wheel::tire_forces`] as a [`wheel::Wheel::swept_contact`] and the rider can ride up it;
/// - the centre of mass along its own velocity for the frame, which is what stops the bike dead
///   against a wall rather than letting it tunnel through at speed;
/// - the centre of mass straight down the chassis' own axis, as far as the suspension plus a wheel
///   reaches, which catches a prop the bike has come to rest on top of.
fn bundle(ch: &chassis::Chassis, m: &Mat4, front: &Wheel, rear: &Wheel, dt: F) -> Bundle {
    let mut b = Bundle::default();
    for w in [front, rear] {
        let from = math::transform_point(m, w.w.mount);
        let to = math::transform_point(m, add_xyz(w.w.mount, w.w.probe));
        b.probes[b.count] = Probe::between(from, to);
        b.count += 1;
    }
    let cofg = ch.cofg_world;
    b.probes[b.count] = Probe::between(cofg, add_xyz(cofg, scale(ch.vel, dt)));
    b.count += 1;
    let reach = front.w.probe_length + front.mv.radius;
    let down = scale(direction(m, UP), -reach);
    b.probes[b.count] = Probe::between(cofg, add_xyz(cofg, down));
    b.count += 1;

    // The sphere every tree node is tested against first: it has to cover every probe's whole length,
    // or the sweep quietly stops colliding.
    b.centre = cofg;
    let mut radius = F::ZERO;
    for probe in &b.probes[..b.count] {
        let end = add_xyz(probe.origin, scale(probe.dir, probe.length));
        radius = radius.max(length(sub_xyz(probe.origin, cofg))).max(length(sub_xyz(end, cofg)));
    }
    b.radius = radius;
    debug_assert!(b.count == PROBES);
    b
}

/// One surface out of a tire's table, by the index the ground query answered with. A tire with no
/// table at all — or an index past its end — gets a surface of all zeroes, which is a tire with no
/// grip and no drag rather than a panic.
fn surface(t: &phy::Tire, i: u8) -> phy::Surface {
    t.surfaces.get(i as usize).copied().unwrap_or_default()
}

/// The stick's angle and deflection, which [`mv::donut_roll`] reads.
///
/// **The port's own reading.** Both are the input layer's, which is not ported; the donut's own
/// ramps (S3's proof) say what the angle has to mean — nothing at straight ahead, all of it by
/// `DonutAngle`, held to `-pi/2`, and back down to nothing at `-pi` — so the angle here runs from 0
/// with the stick pushed forward through `-pi/2` at full lock to `-pi` pulled right back, and the two
/// sides of the stick are folded onto one, because a donut goes either way.
fn stick(s: (F, F)) -> (F, F) {
    let deflection = length(Vf::new(s.0.0, s.1.0, 0.0, 0.0)).min(F::ONE);
    let angle = F(libm::atan2f(fpu::neg(s.0.abs().0), s.1.0));
    (angle, deflection)
}

// --- the wheel module's two steps, driven ----------------------------------------------------------

/// One wheel's tire forces, with the four questions [`wheel::tire_forces`] asks answered.
///
/// It asks for the contact's velocity twice, and the second time is after the bottom-out shove has
/// changed the body's — but the callee is handed no way to write onto the wheel, so a caller cannot
/// answer the second ask from inside. So the step is run on a copy first: if the copy bottomed out
/// (it asked the transform to catch up, which is the one call only that path makes), the velocity is
/// measured again off the copy's shoved body, and the real run gets it. Both runs see the same
/// `chassis_velocity`, which is the one the shove itself reads, so the two answers land exactly where
/// the original's two do.
fn tire_forces(wh: &mut Wheel, mvc: &mut mv::Chassis, cofg_world: Vf, m: &Mat4, dt: F)
               -> Vec<wheel::Call> {
    wh.to_wheel();
    let view = |mvc: &mv::Chassis| wheel::Chassis {
        force: mvc.force,
        torque: mvc.torque,
        accel: mvc.accel,
        cofg: mvc.cofg,
        vel: mvc.vel,
    };
    let at = wh.w.contact_point;
    let ang = mvc.world_ang_vel;
    let spin = |vel: Vf| contact_velocity(vel, ang, at, cofg_world);
    wh.w.chassis_velocity = mvc.vel;
    wh.w.contact_velocity = spin(mvc.vel);

    let mut synced = false;
    {
        let (mut probe_w, mut probe_c) = (wh.w, view(mvc));
        let mut hinge = None;
        let mut ask = |call| answer(m, call, &mut hinge, &mut synced);
        wheel::tire_forces(&mut probe_w, &mut probe_c, &m.0, dt, &mut ask);
        drop(ask);
        if synced {
            wh.w.contact_velocity = spin(probe_c.vel);
        }
    }

    let mut wc = view(mvc);
    let mut hinge = None;
    let mut sync_again = false;
    let mut ask = |call| answer(m, call, &mut hinge, &mut sync_again);
    let calls = wheel::tire_forces(&mut wh.w, &mut wc, &m.0, dt, &mut ask);
    drop(ask);
    // The hinge's force is applied after the step rather than inside it, for the same reason: what is
    // between the two is the compression's rate limit, which does not read the chassis, so the two
    // orders cannot be told apart.
    if let Some((force, at)) = hinge {
        hinge_force(wh, &mut wc, m, force, at);
    }
    mvc.force = wc.force;
    mvc.torque = wc.torque;
    mvc.accel = wc.accel;
    mvc.vel = wc.vel;
    wh.from_wheel();
    calls
}

/// One of [`wheel::tire_forces`]' questions, answered. The three transform calls are arithmetic; the
/// hinge's force and the transform's catch-up are recorded for the caller.
fn answer(m: &Mat4, call: wheel::Call, hinge: &mut Option<(Vf, Vf)>, synced: &mut bool) -> Vf {
    let rot = Mat3::new(m.0[0], m.0[1], m.0[2]);
    match call {
        wheel::Call::Unrotate(v) => chassis::unrotate(&rot, v),
        wheel::Call::Rotate(v) => chassis::rotate(&rot, v),
        wheel::Call::Unpoint(v) => unpoint(m, v),
        wheel::Call::HingeForce(force, at) => {
            *hinge = Some((force, at));
            Vf::ZERO
        }
        wheel::Call::TransformSync => {
            *synced = true;
            Vf::ZERO
        }
        wheel::Call::GroundQuery(_)
        | wheel::Call::SurfaceDrag(_)
        | wheel::Call::SpinIntegrate(_)
        | wheel::Call::ContactVelocity => Vf::ZERO,
    }
}

/// One wheel's own integration: the two stand-ins and then [`wheel::integrate`]'s own steering.
///
/// The two run before the ported call rather than from inside it, because the steering hinge the
/// ported half integrates reads the torque the spin step puts on it — which is the order the original
/// runs them in (surface drag, spin, steering), so the only difference is where the `Call` list is
/// built.
fn wheel_integrate(wh: &mut Wheel, tire: &phy::Tire, brake: F, body: &Body, mvc: &mut mv::Chassis,
                   m: &Mat4, curve: &CurveTable, dt: F) -> Vec<wheel::Call> {
    wh.to_wheel();
    let pt = patch(wh, m, curve);
    let surf = surface(tire, wh.surface);
    let mut wc = wheel::Chassis {
        force: mvc.force,
        torque: mvc.torque,
        accel: mvc.accel,
        cofg: mvc.cofg,
        vel: mvc.vel,
    };
    surface_drag(wh, &pt, &surf, brake, &mut wc);
    spin_integrate(wh, &pt, &surf, body, dt, &mut wc, m, tire.body_tire_force_position, brake,
                   tire.use_parametric_curve, curve);
    let calls = wheel::integrate(&mut wh.w, dt);
    mvc.force = wc.force;
    mvc.torque = wc.torque;
    wh.from_wheel();
    calls
}

// --- one frame of the bike --------------------------------------------------------------------------

impl Ride {
    /// **One frame.** The four steps at the top of this file, in that order, and the ordered list of
    /// everything [`mv::step`] hands out to somebody else — which is what a test compares against
    /// [`mv::step`]'s own list to show that this spine has not drifted from it.
    pub fn frame(&mut self, p: &Phy, tables: &Tables, c: &Controls, dt: F,
                 world: &mut dyn World) -> Vec<mv::Call> {
        if c.respawn {
            let (at, facing) = (self.spawn, self.spawn_facing);
            self.place(at, facing);
        }
        let Ride {
            mv: ride, chassis, engine, air, front, rear, origin, node, held, near, touching, stunt,
            ..
        } = self;
        *touching = false;

        // 1 — the input layer's rate limit.
        held.step(c, &p.joystick, dt);

        // 2 — the chassis' measure, which is where the pitch and roll `air.rs` reads come from. The
        // euler angles are the one call it makes that this crate ports.
        let euler = math::quat_to_euler(chassis.quat, &tables.acos);
        chassis::measure(chassis, dt, euler);

        // The chassis' transform as the last frame left it — the no-hander's roll included, which the
        // body's quaternion only takes in at its next integration (P32).
        let body = held_transform(node, chassis.quat, *origin);

        let mut out = Out { chassis: body };

        // What the ground step takes as given: the chassis' transform's work, at the top of the frame.
        // Two of the three are frame constants, because the transform only moves when the body is
        // integrated and `mv::Step` is what the original's own stack frame is. [`mv::Step::local_accel`]
        // is **not** one of them: the original re-reads the accumulator and re-rotates it every
        // substep, so the substep loop below fills it in rather than this.
        let rot = out.mat3(Frame::Chassis);
        let forward_speed = F(chassis.local_vel.z());
        let reverse_force = chassis::rotate(&rot, mv::reverse_push(&p.mv, forward_speed));
        // The donut's angle and deflection are the pad read's, off the stick before the squaring.
        let raw = trick::stick(c.stick.0, c.stick.1, c.dpad);
        let (stick_angle, deflection) = stick((raw.x, raw.y));

        let step = mv::Step {
            dt,
            on_ground: air.on_ground,
            both_wheels: air.both_wheels,
            speed: forward_speed,
            stick_y: held.stick.1,
            stick_angle,
            deflection,
            lean: held.lean.0,
            gas: held.gas,
            brake: held.brake,
            // The clutch here is the rider's *lever* and not the rate-limited copy the engine
            // reads: the launch ramp is the only thing that looks at this field, and the original's
            // ramp reads the lever (`+0x644`), not `+0x65c`. Keeping the two apart is what stops the
            // idle rule below — which writes `+0x65c` — from rearming or stalling that ramp.
            clutch: c.clutch,
            gas_rate_limited: held.gas,
            reverse: c.reverse,
            balancing: c.balance,
            // Per substep, in the loop: see [`mv::g_peak`]'s call below.
            local_accel: mv::NOTHING,
            reverse_force,
            node_distance: *near,
            stick_x: held.stick.0,
            front_brake: held.front_brake,
            rear_brake: held.rear_brake,
            lean_fore_aft: held.lean.1,
        };

        // The ground step's view of the chassis: the same memory the original's is, so it lives
        // across the whole substep loop and is synced to the body only where the body is integrated.
        let mut mvc = mv::Chassis {
            speed: chassis.speed,
            force: chassis.force,
            // `+0x170` and `+0x180`, which are *not* the body's own `+0x70`/`+0x80`: these two are the
            // impact accumulators the ground step empties at the top of every frame and only a
            // bottomed-out suspension writes ([`wheel::tire_forces`]). Nothing outside the frame reads
            // them, so they live here and nowhere else — and the body's two keep their own values,
            // which the integrate writes from the force and the torque.
            accel: mv::NOTHING,
            ang_accel: mv::NOTHING,
            cofg: chassis.cofg,
            has_transform: true,
            vel: chassis.vel,
            flags: (chassis.ang_free[0], chassis.ang_free[1], chassis.ang_free[2]),
            euler,
            torque: chassis.torque,
            world_ang_vel: chassis.world_ang_vel,
            ang_vel: chassis.ang_vel,
        };

        let mut astep = air::Step {
            dt,
            heading: out.forward(Frame::Chassis),
            up: out.mat3(Frame::Chassis).rows[1].merge(Dest::W, Vf::ZERO),
            front_contact: front.mv.contact,
            rear_contact: rear.mv.contact,
            front_vel: Vf::ZERO,
            rear_vel: Vf::ZERO,
            // The squared stick, the flick, the d-pad's own flag on the fore-aft axis and the trick
            // buttons' group (`+0x9fc`) — which is what the backflip's decay tests, not a direction.
            stick: held.stick,
            stick_hold: held.flick,
            dpad_y: held.digital[1],
            dpad_dir: trick::group(c.trick) as i32,
            brake: held.brake,
            front_brake: held.front_brake,
            rear_brake: held.rear_brake,
            lean: held.lean.0,
            // The trick clocks' "a trick is in progress", not the button: the original's airborne
            // control reads the clocks' own word, which last frame's clocks wrote.
            stunt: *stunt,
            touching: false,
        };

        // 3 — the ground step, in `mv::step`'s own order.
        let mut calls = Vec::new();
        let s = &step;
        let mvp = &p.mv;

        macro_rules! bike {
            () => {
                mv::Bike {
                    ride: &mut *ride,
                    chassis: &mut mvc,
                    front: &mut front.mv,
                    rear: &mut rear.mv,
                    engine: &mut *engine,
                }
            };
        }
        macro_rules! air_chassis {
            () => {
                air::Chassis {
                    speed: mvc.speed,
                    pitch: F(euler.x()),
                    roll: F(euler.z()),
                    force: mvc.force,
                    moment: mvc.torque,
                    vel: mvc.vel,
                    local_vel: chassis.local_vel,
                    ang_vel: mvc.ang_vel,
                }
            };
        }

        mv::pose(mvp, &mut mvc, &mut front.mv, s);
        // The pose's steering command *is* the hinge's angle — one word on the front wheel. P11
        // bridged it here, once a frame; the bridge now lives where the two views of that word
        // actually cross, in [`Wheel::to_wheel`]/[`Wheel::from_wheel`], which run either side of
        // every ported substep. See `to_wheel` for why copying it in per substep is the aliasing
        // and not a choice.
        rear.mv.donut_scale = mv::donut_roll(mvp, &mvc, s);
        front.mv.spring = mv::spring(mvc.speed, mvp.front_spring_when_stopped, mvp.front_spring);
        rear.mv.spring = mv::spring(mvc.speed, mvp.rear_spring_when_stopped, mvp.rear_spring);
        ride.g_peak = mv::NOTHING;
        mvc.accel = mv::NOTHING;
        mvc.ang_accel = mv::NOTHING;
        // Rolling to a stop off the throttle: pull the clutch in for the rider so the engine does not
        // stall. The *rate-limited* throttle is what is tested, so this does not fire the instant the
        // rider lets go.
        //
        // **Where the clutch comes back out.** This writes the same word the input layer's rate
        // limit walks — the engine's own rate-limited clutch, [`Held::clutch`] — and that word
        // persists across the frame boundary, so letting it out is not a separate rule: next frame's
        // [`Held::step`] walks it from here back down to the rider's lever at `ClutchOutRateLimit`,
        // a lever's worth of travel in a quarter of a second on a bike whose file says 4. The rider
        // feeds the clutch out as the engine picks the bike up, which is what a rider does.
        //
        // The port used to keep two copies of that word — one walked by the input layer, one the
        // idle rule wrote — and pulling the clutch in wrote only the second. With nothing walking
        // *it*, a bike that had spent one frame at rest with the throttle shut had a clutch that
        // was fully in for ever (P10's `it_pulls_away_after_a_frame_off_the_throttle`); patching
        // that by copying the lever over the top every frame (P8) cured the latch but let the
        // clutch out in a single frame instead of feeding it. One word has neither fault.
        if s.speed < F(mv::IDLE_SPEED) && s.gas_rate_limited == F::ZERO {
            held.clutch = F::ONE;
        }
        // The float the engine reads through its own pointer (`bike+0x65c`).
        ride.clutch_in = held.clutch;
        let rpm = mv::road_rpm(engine, &rear.mv, s.speed);
        mv::shift(mvp, engine, rpm);
        mv::clutch(mvp, ride, &mut mvc, engine, s);

        let (n, h) = mv::substeps(s.dt);
        for k in 0..n {
            mvc.force = add_xyz(mvc.force, mvp.gravity);
            mv::stand_upright(mvp, &mut bike!(), s, &mut out);
            if s.balancing {
                mv::balance(mvp, &mut bike!(), s, &tables.mv(), &mut out, h);
            }
            mv::roll_pitch(mvp, &mut bike!(), s, &mut out, h);

            calls.push(mv::Call::Preload(h));
            astep.dt = h;
            astep.front_contact = front.mv.contact;
            astep.rear_contact = rear.mv.contact;
            astep.front_vel = chassis::unrotate(&out.mat3(Frame::Chassis), front.w.contact_velocity);
            astep.rear_vel = chassis::unrotate(&out.mat3(Frame::Chassis), rear.w.contact_velocity);
            astep.touching = *touching;
            let mut ac = air_chassis!();
            air::preload(&p.air, air, &mut ac, &astep);
            mvc.force = ac.force;
            mvc.torque = ac.moment;

            calls.push(mv::Call::Backflip(h));
            let mut ac = air_chassis!();
            air::backflip(&p.air, air, &mut ac, &astep);
            mvc.force = ac.force;
            mvc.torque = ac.moment;

            mv::engine_update(engine, &mut rear.mv, s.gas_rate_limited, ride.clutch_in, h);
            mvc.force = add_xyz(mvc.force, mv::drag(mvp, &mvc));

            calls.push(mv::Call::AirborneControl(h));
            let mut ac = air_chassis!();
            air::airborne_control(&p.air, air, &mut ac, &astep);
            mvc.force = ac.force;
            mvc.torque = ac.moment;

            if rear.mv.contact && s.reverse != 0.0 {
                let local = mv::reverse_push(mvp, s.speed);
                if mvc.has_transform {
                    calls.push(mv::Call::Rotate(local));
                    mvc.force = add_xyz(mvc.force, s.reverse_force);
                }
            }
            mv::wheel_clear(&mut front.mv);
            mv::wheel_clear(&mut rear.mv);
            front.w.swept_contact = 0;
            rear.w.swept_contact = 0;

            calls.push(mv::Call::Contacts(s.dt));
            contacts(chassis, &mut mvc, &out.chassis, front, rear, near, touching, s.dt, world);

            for (side, wh) in [(Side::Front, &mut *front), (Side::Rear, &mut *rear)] {
                calls.push(mv::Call::Suspension(side, h));
                wh.to_wheel();
                let mut query = |at: Vf| world.ground(at);
                wheel::suspension(&mut wh.w, &out.chassis.0, &tables.curve, &mut query);
                if wh.w.ray_hit != 0 {
                    wh.surface = world.surface(wh.w.contact_point);
                }
                wh.from_wheel();
            }

            for (side, wh) in [(Side::Front, &mut *front), (Side::Rear, &mut *rear)] {
                calls.push(mv::Call::TireForces(side, h));
                tire_forces(wh, &mut mvc, chassis.cofg_world, &out.chassis, h);
            }
            rear.mv.other_tire_down = front.mv.contact && !rear.mv.contact;

            let body = Body { inv_mass: chassis.inv_mass, inv_inertia: chassis.inv_inertia };
            calls.push(mv::Call::WheelIntegrate(Side::Front, h));
            let brake = held.front_brake;
            wheel_integrate(front, &p.front, brake, &body, &mut mvc, &out.chassis, &tables.curve, h);
            calls.push(mv::Call::WheelIntegrate(Side::Rear, h));
            let brake = held.rear_brake;
            wheel_integrate(rear, &p.rear, brake, &body, &mut mvc, &out.chassis, &tables.curve, h);

            mv::engine_post(engine, &mut rear.mv);

            // The collision record is opened per substep in the original and kept across the frame's
            // meshes; here one is made where it is swept, so there is nothing to open.
            calls.push(mv::Call::CollisionBegin(k == 0));
            calls.push(mv::Call::Unrotate(mvc.accel));
            // The answer to the call just handed over, and the whole of what the wreck test judges a
            // landing on: the impact accumulator at `+0x170`, brought back into the bike's own frame
            // by the transform **as it is now**, and kept per lane by magnitude over the frame's
            // substeps. The vector is the bottom-out shove's, so a bike whose suspension never
            // bottoms out holds g loads of exactly zero — which is what 300 frames of the original
            // idling off the line read on every row (S22).
            let local_accel = chassis::unrotate(&out.mat3(Frame::Chassis), mvc.accel);
            ride.g_peak = mv::g_peak(ride.g_peak, local_accel);

            calls.push(mv::Call::Integrate(h));
            chassis.force = mvc.force;
            chassis.torque = mvc.torque;
            chassis.cofg = mvc.cofg;
            chassis.vel = mvc.vel;
            chassis.ang_vel = mvc.ang_vel;
            chassis::set_flags(chassis, mvc.flags.0, mvc.flags.1, mvc.flags.2);
            integrate(chassis, h, &out.chassis, origin);
            out.chassis = math::quat_to_mat4(chassis.quat, *origin);
            mvc.force = chassis.force;
            mvc.torque = chassis.torque;
            mvc.vel = chassis.vel;
            mvc.ang_vel = chassis.ang_vel;
            mvc.world_ang_vel = chassis.world_ang_vel;

            if !s.balancing {
                ride.balance_noise_time = F::ZERO;
                ride.balance_point = F::ZERO;
                ride.balance_inertia = F::ZERO;
                ride.balance_command = F::ZERO;
                mv::no_hander(mvp, &mut bike!(), s, &tables.mv(), &mut out, h);
            }
        }
        // The transform is the engine's object and outlives the frame; the rows are what of it the
        // ride has to carry, the origin being `origin` already.
        *node = [out.chassis.0[0], out.chassis.0[1], out.chassis.0[2]];

        ride.collision_near = s.node_distance <= F(mv::COLLISION_NEAR);
        calls.push(mv::Call::SearchReset);
        calls.push(mv::Call::SearchGather);

        // 4 — the air state, once a frame and after the ground step, which is the order
        // `research/oracle.txt`'s A5 block measured.
        astep.dt = dt;
        astep.front_contact = front.mv.contact;
        astep.rear_contact = rear.mv.contact;
        astep.front_vel = chassis::unrotate(&out.mat3(Frame::Chassis), front.w.contact_velocity);
        astep.rear_vel = chassis::unrotate(&out.mat3(Frame::Chassis), rear.w.contact_velocity);
        astep.touching = *touching;
        let ac = air_chassis!();
        air::contact(&p.air, air, &ac, &astep, &tables.acos);

        calls
    }
}

/// The frame's one sweep, and what comes out of it.
///
/// The two wheel probes become contacts and go through [`chassis::wheel_contacts`], which is the
/// original's own distributor — including its rule that a contact forward of the centre of mass is
/// the front wheel's and anything else the rear's. The two body probes are resolved here: the travel
/// the prop took away comes out of the position and the part of the velocity that was going into the
/// surface is killed, which is what stops a rider against a prop instead of through it.
#[allow(clippy::too_many_arguments)] // one frame's worth of the bike, and all of it needed
fn contacts(ch: &mut chassis::Chassis, mvc: &mut mv::Chassis, m: &Mat4, front: &mut Wheel,
            rear: &mut Wheel, near: &mut F, touching: &mut bool, dt: F, world: &mut dyn World) {
    let b = bundle(ch, m, front, rear, dt);
    let mut record = Record::new();
    let look = world.solid(&b, &mut record);

    let mut cs: Vec<chassis::Contact> = Vec::new();
    for i in 0..b.count {
        if !record.hit[i] {
            continue;
        }
        if i < 2 {
            // A wheel's line. `select` is a distance along the bike and not a flag, so it is the
            // contact's own position in the chassis' frame, forward of the centre of mass.
            let local = unpoint(m, record.point[i]);
            cs.push(chassis::Contact {
                select: F(local.z()) - F(mvc.cofg.z()),
                normal: record.normal[i],
                point: record.point[i],
                // The collision node, the owner and the vehicle are the scene's, and the port has no
                // handle to put here: a prop is the world, which is what a zero owner means.
                node: 0,
                owner: 0,
                vehicle: 0,
                vehicle_kind: 0,
                listeners: 0,
            });
        } else {
            // A body probe. `Record::push`'s own arithmetic, per probe: the travel the triangle took
            // away, along the normal — times the probe's length, because the record's version is a
            // fraction of one unit of travel and not of the probe.
            let probe = b.probes[i];
            let normal = record.normal[i];
            let left = scale(probe.dir, F::ONE - record.fraction[i]);
            let blocked = scale(normal, dot3(left, normal));
            ch.impulse = sub_xyz(ch.impulse, scale(blocked, probe.length));
            let into = dot3(mvc.vel, normal).min(F::ZERO);
            mvc.vel = sub_xyz(mvc.vel, scale(normal, into));
            *touching = true;
        }
    }

    let mut fc = chassis::WheelContact::default();
    let mut rc = chassis::WheelContact::default();
    let mut rear_node = 0;
    chassis::wheel_contacts(&mut fc, &mut rc, &mut rear_node, look, &cs, dt);
    for (wc, wh) in [(fc, &mut *front), (rc, &mut *rear)] {
        if wc.touching != 0 {
            wh.w.contact_point = wc.point;
            wh.w.contact_normal = wc.normal;
            wh.w.swept_contact = 1;
            wh.mv.contact_normal = wc.normal;
            wh.mv.fresh.scalars[1] = 1;
            wh.mv.fresh.scalars[2] = wc.owner;
            wh.mv.fresh.scalars[3] = wc.vehicle;
        }
    }

    // What the ground step reads as `node_distance`: how close the nearest thing the sweep looked at
    // came, as a fraction of the probe that found it.
    *near = if record.hits > 0 { record.nearest } else { F::ONE };
}

/// The contact list [`air::hit_wreck`] reads: the bike's head ray ([`air::HEAD`]) swept against
/// everything solid at the transform the frame ended on.
///
/// **This composition is the port's**, like [`bundle`]'s. The original sweeps eleven body rays out of
/// the bike's model every substep and resolves what they hit; the port has no model, so it sweeps the
/// one ray that can wreck a rider who is not on a 50cc bike, once, after the frame, and resolves
/// nothing — the frame's own probes have already done the pushing. A prop is the world, so what it
/// hits is [`air::Other::Scenery`].
pub fn hits(ride: &Ride, world: &mut dyn World) -> Vec<air::Contact> {
    let m = ride.transform();
    let from = math::transform_point(&m, air::HEAD.0);
    let to = math::transform_point(&m, air::HEAD.1);
    let mut b = Bundle::default();
    b.probes[0] = Probe::between(from, to);
    b.count = 1;
    b.centre = from;
    b.radius = length(sub_xyz(to, from));
    let mut record = Record::new();
    world.solid(&b, &mut record);
    let reach = F(air::HEAD.1.y());
    if record.hit[0] {
        vec![air::Contact { mirrored: false, reach, other: air::Other::Scenery }]
    } else {
        Vec::new()
    }
}

/// Another rider, as this one's head ray sees it: where their body is and the two words of their
/// chassis the wreck test reads off a struck rider — its height (`+0x64`, the centre of gravity's
/// world `y`) and its velocity (`+0x260`), [`air::Other::Rider`]'s pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rival {
    /// The centre of the rider's body: the hub all eleven of the bike's body rays start at
    /// ([`air::HEAD`]`.0`), through the rival's own transform.
    pub centre: Vf,
    /// How far the body reaches round that hub: the head ray's own length.
    pub radius: F,
    pub height: F,
    pub vel: Vf,
}

impl Ride {
    /// What this bike is to another rider's head ray, as the frame left it.
    ///
    /// **The shape is the port's own.** In the original a rider's body rays strike the other bike's
    /// collidable, whose shape comes out of the bike's model; the port loads no model, so the body
    /// is a sphere about the body rays' own hub, as big as the longest of them — the reach the
    /// original's rays say a rider's body has, and no number that is not already measured.
    pub fn rival(&self) -> Rival {
        let m = self.transform();
        Rival {
            centre: math::transform_point(&m, air::HEAD.0),
            radius: length(sub_xyz(air::HEAD.1, air::HEAD.0)),
            height: F(self.chassis.cofg_world.y()),
            vel: self.chassis.vel,
        }
    }
}

/// Where along `from → to` (a fraction of it) the segment first enters the sphere, or `None`. A
/// segment that starts inside it enters at 0.
fn enters(from: Vf, to: Vf, centre: Vf, radius: F) -> Option<F> {
    let d = sub_xyz(to, from);
    let f = sub_xyz(from, centre);
    let a = dot3(d, d);
    let b = dot3(f, d);
    let c = dot3(f, f) - radius * radius;
    if c <= F::ZERO {
        return Some(F::ZERO);
    }
    let disc = b * b - a * c;
    if b >= F::ZERO || disc < F::ZERO || a == F::ZERO {
        return None;
    }
    let t = (F::ZERO - b - disc.sqrt()) / a;
    (t <= F::ONE).then_some(t)
}

/// [`hits`] in a session with other riders in it: the same head ray, swept against the world **and**
/// against every rider in `rivals`, and the one nearest along the ray kept.
///
/// One contact per ray, the nearest, is how the original's ray probe keeps its list
/// (`Record::one_per_ray`); that the bike's probe has the flag up as the plane's does is assumed, not
/// read. A rider struck is an [`air::Other::Rider`] by their [`Rival::height`] and [`Rival::vel`],
/// which is what lets [`air::hit_wreck`] answer [`air::Wreck::Joust`]. With no rivals this is
/// [`hits`], word for word: the world is swept the same way and nothing else is asked.
///
/// The struck rider's own record gets nothing back here. The original mirrors the contact into it
/// (kind 1), and that rider's wreck test skips mirrored entries, so the mirror cannot wreck them;
/// what else reads it is listed in the S54 journal.
pub fn hits_among(ride: &Ride, world: &mut dyn World, rivals: &[Rival]) -> Vec<air::Contact> {
    if rivals.is_empty() {
        return hits(ride, world);
    }
    let m = ride.transform();
    let from = math::transform_point(&m, air::HEAD.0);
    let to = math::transform_point(&m, air::HEAD.1);
    let mut b = Bundle::default();
    b.probes[0] = Probe::between(from, to);
    b.count = 1;
    b.centre = from;
    b.radius = length(sub_xyz(to, from));
    let mut record = Record::new();
    world.solid(&b, &mut record);
    let reach = F(air::HEAD.1.y());
    let mut best = record.hit[0].then(|| (record.fraction[0], air::Other::Scenery));
    for r in rivals {
        let Some(t) = enters(from, to, r.centre, r.radius) else { continue };
        // Strictly nearer, so the world keeps a tie and, among riders, the first in the list does.
        if best.is_none_or(|(near, _)| t < near) {
            best = Some((t, air::Other::Rider { height: r.height, vel: r.vel }));
        }
    }
    best.map(|(_, other)| vec![air::Contact { mirrored: false, reach, other }]).unwrap_or_default()
}

// --- the state hash ---------------------------------------------------------------------------------

/// **Every word of the ride, in declaration order.**
///
/// The walk is what makes a replay provable: two runs of the same inputs are the same ride only if
/// every leaf here matches, including the `w` lanes nothing reads (see [`crate::hash`]).
///
/// The parts are walked by this module's own private functions rather than by `impl State` on
/// [`mv::Ride`], [`chassis::Chassis`] and the rest, so that this task adds nothing to the four modules
/// it drives — each of them is somebody else's, and an `impl` left in one of them would be a second
/// copy of the walk the day its owner writes their own.
impl State for Ride {
    fn walk(&self, w: &mut Walk<'_>) {
        w.group("mv", |w| walk_mv_ride(w, &self.mv));
        w.group("chassis", |w| walk_chassis(w, &self.chassis));
        w.group("engine", |w| walk_engine(w, &self.engine));
        w.group("air", |w| walk_air(w, &self.air));
        w.group("front", |w| walk_wheel(w, &self.front));
        w.group("rear", |w| walk_wheel(w, &self.rear));
        w.vf("origin", self.origin);
        w.group("node", |w| {
            w.vf("right", self.node[0]);
            w.vf("up", self.node[1]);
            w.vf("forward", self.node[2]);
        });
        w.group("held", |w| {
            w.f("gas", self.held.gas);
            w.f("brake", self.held.brake);
            w.f("front_brake", self.held.front_brake);
            w.f("rear_brake", self.held.rear_brake);
            w.f("clutch", self.held.clutch);
            w.f("stick_x", self.held.stick.0);
            w.f("stick_y", self.held.stick.1);
            w.f("lean_x", self.held.lean.0);
            w.f("lean_y", self.held.lean.1);
            w.f("flick", self.held.flick);
            w.bool("digital_x", self.held.digital[0]);
            w.bool("digital_y", self.held.digital[1]);
            w.f("flick_now", self.held.ring.now);
            w.u8("flick_head", self.held.ring.head as u8);
            let ring = self.held.ring;
            let slots: [Slot; FLICK_SAMPLES] = std::array::from_fn(|k| Slot(ring.at[k], ring.value[k]));
            w.list("flick_ring", &slots);
        });
        w.vf("spawn", self.spawn);
        w.vf("spawn_facing", self.spawn_facing);
        w.f("near", self.near);
        w.bool("touching", self.touching);
        w.bool("stunt", self.stunt);
    }
}

fn walk_mv_ride(w: &mut Walk<'_>, r: &mv::Ride) {
    w.vf("g_peak", r.g_peak);
    w.f("clutch_timer", r.clutch_timer);
    w.f("clutch_scale_start", r.clutch_scale_start);
    w.f("clutch_cofg_weight", r.clutch_cofg_weight);
    w.f("clutch_in", r.clutch_in);
    w.bool("collision_near", r.collision_near);
    w.f("balance_inertia", r.balance_inertia);
    w.f("balance_command", r.balance_command);
    w.f("balance_point", r.balance_point);
    w.f("balance_noise_time", r.balance_noise_time);
    w.f("pitch_integral", r.pitch_integral);
    w.bool("stoppie", r.stoppie);
}

fn walk_chassis(w: &mut Walk<'_>, c: &chassis::Chassis) {
    w.f("speed", c.speed);
    w.vf("cofg_world", c.cofg_world);
    w.vf("accel", c.accel);
    w.vf("ang_accel", c.ang_accel);
    w.vf("euler", c.euler);
    for (i, row) in c.inv_inertia.rows.iter().enumerate() {
        w.group(INV_INERTIA[i], |w| w.vf("row", *row));
    }
    w.vf("force", c.force);
    w.vf("torque", c.torque);
    w.vf("impulse", c.impulse);
    w.vf("correction", c.correction);
    w.vf("cofg", c.cofg);
    w.vf("quat", c.quat);
    w.i32("awake", c.awake);
    w.i32("contacts", c.contacts);
    w.f("inv_mass", c.inv_mass);
    w.f("max_speed_sq", c.max_speed_sq);
    w.f("max_ang_speed_sq", c.max_ang_speed_sq);
    w.group("ang_vel_max", |w| triple(w, &c.ang_vel_max));
    w.group("ang_vel_min", |w| triple(w, &c.ang_vel_min));
    w.vf("vel", c.vel);
    w.vf("local_vel", c.local_vel);
    w.vf("world_ang_vel", c.world_ang_vel);
    w.vf("ang_vel", c.ang_vel);
    w.group("free", |w| {
        for (i, v) in c.free.iter().enumerate() {
            w.i32(AXIS[i], *v);
        }
    });
    w.group("ang_free", |w| {
        for (i, v) in c.ang_free.iter().enumerate() {
            w.i32(AXIS[i], *v);
        }
    });
    w.group("pos_hist", |w| {
        triple(w, &c.pos_hist.dx);
        w.group("prev", |w| triple(w, &c.pos_hist.prev));
    });
    w.group("vel_hist", |w| {
        triple(w, &c.vel_hist.dx);
        w.group("prev", |w| triple(w, &c.vel_hist.prev));
    });
    w.group("ang_hist", |w| {
        triple(w, &c.ang_hist.dx);
        w.group("prev", |w| triple(w, &c.ang_hist.prev));
    });
    w.group("quat_hist", |w| {
        quad(w, &c.quat_hist.dx);
        w.group("prev", |w| quad(w, &c.quat_hist.prev));
    });
}

fn walk_engine(w: &mut Walk<'_>, e: &mv::Engine) {
    w.i32("gear", e.gear);
    w.i32("gears", e.gears);
    w.f("final_drive", e.final_drive);
    w.group("gear_ratios", |w| {
        for (i, v) in e.gear_ratios.iter().enumerate() {
            w.group(RATIO[i], |w| w.f("ratio", *v));
        }
    });
    w.f("ratio", e.ratio);
    w.f("inv_ratio", e.inv_ratio);
    w.f("clutch_torque_scale", e.clutch_torque_scale);
    w.bool("clutch_locked", e.clutch_locked);
    w.f("rev_limit", e.rev_limit);
    w.f("idle_throttle", e.idle_throttle);
    w.f("engine_braking", e.engine_braking);
    w.f("inv_inertia", e.inv_inertia);
    w.f("clutch_static", e.clutch_static);
    w.f("clutch_kinetic", e.clutch_kinetic);
    w.f("min_rpm", e.min_rpm);
    w.group("curve_rpm", |w| {
        for (i, v) in e.curve_rpm.iter().enumerate() {
            w.group(POINT[i], |w| w.f("rpm", *v));
        }
    });
    w.group("curve_torque", |w| {
        for (i, v) in e.curve_torque.iter().enumerate() {
            w.group(POINT[i], |w| w.f("torque", *v));
        }
    });
    w.group("curve", |w| {
        w.u32("points", e.curve.points);
        w.u32("index", e.curve.index);
        w.f("frac", e.curve.frac());
    });
    w.f("torque", e.torque);
    w.f("wheel_torque", e.wheel_torque);
    w.f("rpm", e.rpm);
    w.f("road_rpm", e.road_rpm);
    w.f("torque_scale", e.torque_scale);
}

fn walk_air(w: &mut Walk<'_>, a: &air::Air) {
    w.bool("on_ground", a.on_ground);
    w.bool("both_wheels", a.both_wheels);
    w.bool("airborne", a.airborne);
    w.bool("was_airborne", a.was_airborne);
    w.f("hang_time", a.hang_time);
    w.vf("contact_vel", a.contact_vel);
    w.f("crossup", a.crossup);
    w.f("authority", a.authority);
    w.bool("preload_ready", a.preload_ready);
    w.bool("preload_firing", a.preload_firing);
    w.bool("preload_armed", a.preload_armed);
    w.f("preload_timer", a.preload_timer);
    w.vf("preload_force", a.preload_force);
    w.bool("flip_torque_applied", a.flip_torque_applied);
    w.bool("flipping", a.flipping);
    w.bool("flip_active", a.flip_active);
    w.bool("flip_armed", a.flip_armed);
    w.f("flip_grace", a.flip_grace);
    w.vf("flip_torque", a.flip_torque);
    w.vf("wreck_impact", a.wreck_impact);
}

fn walk_wheel(w: &mut Walk<'_>, wh: &Wheel) {
    w.group("mv", |w| {
        let m = &wh.mv;
        w.bool("balancing", m.balancing);
        w.f("donut_scale", m.donut_scale);
        w.f("spring", m.spring);
        w.f("max_steer_angle", m.max_steer_angle);
        w.f("steer_command", m.steer_command);
        w.f("inv_radius", m.inv_radius);
        w.f("spin", m.spin);
        w.bool("contact", m.contact);
        w.bool("other_tire_down", m.other_tire_down);
        w.f("drive_torque", m.drive_torque);
        w.f("radius", m.radius);
        w.bool("spin_from_torque", m.spin_from_torque);
        w.f("contact_force_fwd", m.contact_force_fwd);
        w.vf("contact_normal", m.contact_normal);
        w.bool("steered", m.steered);
        w.group("fresh", |w| {
            for (i, v) in m.fresh.scalars.iter().enumerate() {
                w.group(WORD[i], |w| w.u32("word", *v));
            }
            for (i, v) in m.fresh.vectors.iter().enumerate() {
                w.group(WORD[i], |w| w.vf("vector", *v));
            }
        });
    });
    w.group("w", |w| {
        let s = &wh.w;
        w.bool("has_ground", s.has_ground);
        w.vf("mount", s.mount);
        w.vf("mount_world", s.mount_world);
        w.vf("probe", s.probe);
        w.vf("probe_end", s.probe_end);
        w.vf("probe_end_world", s.probe_end_world);
        w.f("probe_length", s.probe_length);
        w.vf("ground_normal", s.ground_normal);
        w.i32("ray_steps", s.ray_steps);
        w.i32("ray_hit", s.ray_hit);
        w.i32("swept_contact", s.swept_contact);
        w.i32("terrain_wins", s.terrain_wins);
        w.vf("contact_point", s.contact_point);
        w.vf("contact_normal", s.contact_normal);
        w.f("max_compression", s.max_compression);
        w.f("max_damper_speed", s.max_damper_speed);
        w.f("extension_rate", s.extension_rate);
        w.f("spring_rate", s.spring_rate);
        w.f("damping_out", s.damping_out);
        w.f("damping_in", s.damping_in);
        w.bool("contact", s.contact);
        w.f("compression", s.compression);
        w.f("last_compression", s.last_compression);
        w.f("spring_force", s.spring_force);
        w.vf("load_point", s.load_point);
        w.vf("contact_velocity", s.contact_velocity);
        w.vf("chassis_velocity", s.chassis_velocity);
        w.bool("steered", s.steered);
        w.bool("balancing", s.balancing);
        w.f("steer_torque", s.steer_torque);
        w.f("steer_damping", s.steer_damping);
        w.f("steer_inv_inertia", s.steer_inv_inertia);
        w.f("max_steer_angle", s.max_steer_angle);
        w.f("min_steer_angle", s.min_steer_angle);
        w.f("steer_angle", s.steer_angle);
        w.f("steer_rate", s.steer_rate);
    });
    w.u8("surface", wh.surface);
    w.f("inv_inertia", wh.inv_inertia);
    w.vf("steer_axis", wh.steer_axis);
    w.group("slip", |w| {
        w.bool("spinning", wh.slip.spinning);
        w.bool("locked", wh.slip.locked);
        w.bool("sliding", wh.slip.sliding);
    });
}

/// Three floats under `x` · `y` · `z`, which is how every triple in the chassis is indexed.
fn triple(w: &mut Walk<'_>, v: &[F; 3]) {
    for (i, f) in v.iter().enumerate() {
        w.f(AXIS[i], *f);
    }
}

/// Four, for the quaternion history.
fn quad(w: &mut Walk<'_>, v: &[F; 4]) {
    for (i, f) in v.iter().enumerate() {
        w.f(QUAT[i], *f);
    }
}

// The walk's names have to be `&'static str`, so an index into a field's own list is spelled out.
const AXIS: [&str; 3] = ["x", "y", "z"];
const QUAT: [&str; 4] = ["x", "y", "z", "w"];
const INV_INERTIA: [&str; 3] = ["inv_inertia_x", "inv_inertia_y", "inv_inertia_z"];
const RATIO: [&str; 8] = ["g1", "g2", "g3", "g4", "g5", "g6", "g7", "g8"];
const POINT: [&str; mv::MAX_RPM_POINTS] =
    ["p0", "p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9", "p10", "p11", "p12", "p13", "p14",
     "p15"];
const WORD: [&str; 7] = ["w0", "w1", "w2", "w3", "w4", "w5", "w6"];

// --- the wreck, and getting back up ----------------------------------------------------------------

/// **The wreck bike's inertia tensor**, which no file holds.
///
/// A rigid-body part's inertia is worked out by its loader from the *model's* bounding box
/// ([`phy::Part::inertia`]), and this port has no model library, so the tensor the game builds for
/// `Wreckbike.phy` is taken from the running game instead: it is the diagonal the wreck body carries
/// in a freestyle session, read over PINE, with the three reciprocals the body actually integrates
/// with in the quadword after it. `Wreckbike.phy`'s own
/// `Weight = 250` and `InertiaScale = 1,1,1` imply a box of about 3.65 x 5.37 x 7.61 feet through the
/// loader's formula, which is a bike — so this is the measured value of a formula that is ported, not
/// a number of the port's own.
pub const WRECK_INERTIA: Vf = Vf([56.111691, 46.036716, 27.289017, 1.0]);

/// Which of `[Wrecks]`' seven throws a wreck reason uses.
///
/// **The port's own inference, and the one piece of this task with nothing behind it.** The names in
/// [`phy::Hit`] are the file's and the reasons are [`air::Wreck`]'s, and the mapping between them is
/// in neither: there is no string, no table and no key for it anywhere in the executable, and the
/// ragdoll code that would read one is not ported. What is written here is what the names say, and
/// only [`phy::Hit::Bottom`] — the reason the human's capture hit twice — is exercised by a test.
pub fn hit_of(reason: trick::Wreck) -> phy::Hit {
    match reason.0 {
        1 => phy::Hit::Front,          // air::Wreck::Front, thrown over the bars
        2 | 3 => phy::Hit::Side,       // Left, Right
        4 => phy::Hit::Back,           // Back
        5 => phy::Hit::Bottom,         // Flat — landed too hard, and the only one proved
        6 => phy::Hit::Front,          // Scenery — the rider's head into something, from in front
        8 => phy::Hit::FallOffFront,   // NoseDive
        0xb => phy::Hit::FallOffBack,  // Tail
        7 | 0xc => phy::Hit::Stunt,    // Joust, NoHander — came off doing something
        13 => phy::Hit::Front,         // state::Wreck::CANNONBALL, which is a hit from in front
        _ => phy::Hit::Bottom,         // state::Wreck::NO_CONTROL and anything new: put down
    }
}

/// **A wreck in progress**: the bike's wreck body tumbling, and the rider coming off it.
///
/// This is the half of a wreck the ride state machine ([`crate::state`]) does not do. The machine
/// keeps the reason and the clocks and decides when to put the rider back; what it *waits* for is a
/// signal it calls settling ([`crate::state::Sense::settles`]), and nothing in the port set that
/// before this. A [`Crash`] is what sets it, by simulating the two things the original has lying on
/// the dirt:
///
/// * **The bike**, as `Wreckbike.phy`'s one `[PhysicsPart1]` — a rigid body of its own, entered from
///   the bike's own velocity and spin by [`Crash::begin`] (a port of `bike_wreck_entry`'s body half)
///   and integrated by the same ported integrator the bike uses ([`chassis::integrate`]).
/// * **The rider**, as one point. The original's rider is an eighteen-part ragdoll with twenty-six
///   constraints running at 25 Hz, which is not ported and is not a small job; what is here is the
///   one thing the machine needs out of it — when the rider, having let go of the bars after the
///   throw's `HandHoldTime`, hits the ground.
///
/// # What says the rider has landed, and why it is that
///
/// Measured, not guessed. In the running game the settle gate (`player + 0xd3c`) comes on 18, 22 and
/// 26 ticks after the entry across three reason-5 wrecks (two driven live over PINE, one in the
/// human's capture) — 0.60 s, 0.73 s and 0.87 s. `HitBottomHandHoldTime` is 0.3 s and
/// `RiderFlailHeight` is 3 feet, and a 3-foot fall under the file's own gravity takes 0.43 s: 0.3 +
/// 0.43 = 0.73 s, which is the middle of the three to the frame. The spread either side is the
/// vertical velocity the rider leaves with, which is what this port carries off the wreck body.
///
/// Two models the same measurement **rules out**: the gate is not the wreck body coming to rest (the
/// body was still doing 42 feet a second when the gate came on), and it is not the throw force
/// applied to a whole rider (800 lbf for 0.1 s over the ragdoll's total mass is 11 feet a second
/// upwards, a 0.89 s flight, and a gate 10 ticks late). So the throw's force is **loaded and not
/// applied** here: it poses a ragdoll this port does not have, and applying it to a point moves the
/// one number that was measured the wrong way.
#[derive(Clone, Debug)]
pub struct Crash {
    /// The reason the machine entered with, and the throw it picks.
    pub reason: trick::Wreck,
    pub hit: phy::Hit,
    pub throw: phy::Throw,
    /// Seconds since the entry. The machine's own clock is restarted when its latch arms; this one
    /// is not, because the hand-hold time is measured from the entry.
    pub clock: F,
    /// The rider still has hold of the bars.
    pub holding: bool,
    /// What is left of the throw's `ForceTime` once the hands are off. Counted down and not spent —
    /// see the type's own docs.
    pub throwing: F,
    /// The bike's wreck body, and its transform's origin.
    pub body: chassis::Chassis,
    pub origin: Vf,
    /// Where the rider is and how fast, as one point, and whether they have hit the ground yet.
    pub rider: Vf,
    pub rider_vel: Vf,
    pub landed: bool,
    /// The height of the ground under the rider point, as this frame's step found it: what the
    /// wreck mode's settle test measures the rider's clearance from ([`Crash::clearance`]).
    pub ground: F,
    /// The wreck body has stopped, by `Wreckbike.phy`'s own `SpeedThreshold` and
    /// `AngularRateThreshold`. Not what the machine waits for — see [`Crash::settles`] — but it is
    /// what says the place [`Crash::rest`] hands back has stopped moving.
    pub resting: bool,
    /// Where the bike was pointing when it wrecked, flattened.
    pub facing: Vf,
}

impl Crash {
    /// **The wreck entered**, as the original's own wreck entry does its half of it: the wreck body is
    /// put where the bike is, and takes the bike's velocity and the bike's spin *in its own frame*.
    ///
    /// `inertia` is the body's tensor — [`WRECK_INERTIA`] off the disc, or [`phy::Part::inertia`] of a
    /// box. `spin` is the cannonball's: for reason 13 the original throws three random numbers in
    /// `[0, 1)` at the body's angular velocity instead of the bike's, which needs the game's own
    /// `rand` stream and so is handed in rather than drawn here. `None` uses the bike's spin, which is
    /// every other reason's path.
    pub fn begin(
        ride: &Ride,
        reason: trick::Wreck,
        part: &phy::Part,
        inertia: Vf,
        wrecks: &phy::Wrecks,
        spin: Option<Vf>,
    ) -> Crash {
        let hit = hit_of(reason);
        let m = ride.transform();
        let inv = |v: f32| if v == 0.0 { F::ZERO } else { F::ONE / F(v) };
        let body = chassis::Chassis {
            awake: 1,
            cofg: part.cofg,
            inv_mass: if part.mass == F::ZERO { F::ZERO } else { F::ONE / part.mass },
            inv_inertia: Mat3::new(
                Vf::new(inv(inertia.x()).0, 0.0, 0.0, 0.0),
                Vf::new(0.0, inv(inertia.y()).0, 0.0, 0.0),
                Vf::new(0.0, 0.0, inv(inertia.z()).0, 0.0),
            ),
            quat: ride.chassis.quat,
            cofg_world: math::transform_point(&m, part.cofg),
            // The entry's two copies: `chassis + 0x260` into the body's velocity and `chassis + 0x290`
            // — the spin in the bike's own frame — into the body's.
            vel: ride.chassis.vel,
            ang_vel: spin.unwrap_or(ride.chassis.ang_vel),
            free: [1; 3],
            ang_free: [1; 3],
            // The same stand-in limits the bike is built with, and for the same reason: a zero limit
            // is read as a standstill by the clamp. `Wreckbike.phy`'s own `BodyAngularRateLimit` is
            // one of the four keys nothing in the game reads.
            max_speed_sq: F(MAX_SPEED * MAX_SPEED),
            max_ang_speed_sq: F(MAX_ANG_SPEED * MAX_ANG_SPEED),
            ang_vel_max: [F(MAX_ANG_SPEED); 3],
            ang_vel_min: [F(-MAX_ANG_SPEED); 3],
            correction: chassis::CLEAR,
            ..chassis::Chassis::default()
        };
        let (forward, _) = ride.axes();
        let flat = Vf::new(forward.x(), 0.0, forward.z(), 0.0);
        Crash {
            reason,
            hit,
            throw: wrecks.throw(hit),
            clock: F::ZERO,
            holding: true,
            throwing: F::ZERO,
            body,
            origin: ride.origin,
            // While the hands are on the bars the rider is where the bike is; the flail height is
            // taken when they come off.
            rider: ride.origin,
            rider_vel: ride.chassis.vel,
            landed: false,
            ground: F(ride.origin.y()),
            resting: false,
            facing: if length(flat) > F::ZERO { normalize(flat) } else { ride.spawn_facing },
        }
    }

    /// **One frame of the wreck**: the hands, the body, and the rider.
    pub fn step(&mut self, dt: F, part: &phy::Part, wrecks: &phy::Wrecks, world: &mut dyn World) {
        self.clock = self.clock + dt;

        // The hands come off the bars once the hold time is up, and the rider leaves with the body's
        // velocity, from the flail height above where the body is.
        if self.holding && !(self.clock < self.throw.hand_hold_time) {
            self.holding = false;
            self.throwing = self.throw.force_time;
            self.rider = add_xyz(self.body.cofg_world, scale(UP, wrecks.rider_flail_height));
            self.rider_vel = self.body.vel;
        }

        // The body: its own gravity, the dirt under it, and the ported integrator.
        self.body.force = add_xyz(self.body.force, part.gravity);
        self.contact(dt, part, world);
        let entry = math::quat_to_mat4(self.body.quat, self.origin);
        integrate(&mut self.body, dt, &entry, &mut self.origin);
        self.resting = length(self.body.vel) <= part.speed_threshold
            && length(self.body.ang_vel) <= part.angular_rate_threshold;

        // The rider, as one point, until they reach the dirt.
        if !self.holding && !self.landed {
            if self.throwing > F::ZERO {
                self.throwing = self.throwing - dt;
            }
            let g = -(part.gravity_scale * phy::G);
            self.rider_vel = add_xyz(self.rider_vel, Vf::new(0.0, (g * dt).0, 0.0, 0.0));
            self.rider = add_xyz(self.rider, scale(self.rider_vel, dt));
            let (on, _) = world.ground(self.rider);
            self.ground = F(on.y());
            if self.rider.y() <= on.y() {
                self.rider = Vf::new(self.rider.x(), on.y(), self.rider.z(), 1.0);
                self.rider_vel = Vf::ZERO;
                self.landed = true;
            }
        } else if self.holding {
            self.rider = self.body.cofg_world;
            self.rider_vel = self.body.vel;
            self.ground = F(world.ground(self.rider).0.y());
        }
    }

    /// How far the rider is above the ground under them: what the wreck mode's settle test reads.
    ///
    /// The original's test is the main bone's height above `ground_query`'s answer straight under
    /// it; this port's rider is one point, so the point stands in for the bone.
    pub fn clearance(&self) -> F {
        F(self.rider.y()) - self.ground
    }

    /// The dirt under the wreck body, as a spring, a damper and a friction at one contact point.
    ///
    /// **The shape is the original's and the numbers are the file's; the solver is not ported.** The
    /// engine resolves a part against the world through its collision record, which this crate has no
    /// port of (`collide.rs` is the sweep, not the response). What is here is the contact the file is
    /// written for: `SpringStiffness` and `SpringDamping` against the penetration of the part's own
    /// `Radius` sphere, held to `MaxPenetration`, with `FrictionZone1` across it, each put in at the
    /// contact point so that it turns the body as well as pushing it, and each scaled by the file's
    /// own `SpringMomentScale` and `FrictionMomentScale`. The two rates go through the same
    /// [`phy::spring_rate`] and [`phy::damping`] the wheels' do, which is what makes a stiffness of 5
    /// hold a 250 lb body up.
    fn contact(&mut self, dt: F, part: &phy::Part, world: &mut dyn World) {
        if !part.collision_enabled {
            return;
        }
        let (on, normal) = world.ground(self.body.cofg_world);
        let depth = part.radius - dot3(sub_xyz(self.body.cofg_world, on), normal);
        if depth <= F::ZERO {
            return;
        }
        let m = math::quat_to_mat4(self.body.quat, self.origin);
        let at = on;
        let rate = phy::spring_rate(part.spring_stiffness, part.mass);
        let held = if depth < part.max_penetration { depth } else { part.max_penetration };
        let v = contact_velocity(self.body.vel, self.body.world_ang_vel, at, self.body.cofg_world);
        let closing = -dot3(v, normal);
        let load = rate * held + phy::damping(part.spring_damping, part.mass, rate) * closing;
        let load = if load > F::ZERO { load } else { F::ZERO };
        self.push(&m, scale(normal, load), at, part.spring_moment_scale);

        // The slide across the contact, held to what would stop it inside one step so that a body at
        // rest is not shoved about by its own friction.
        let across = sub_xyz(v, scale(normal, dot3(v, normal)));
        let speed = length(across);
        if speed > F::ZERO {
            let grip = part.friction_zone1 * load;
            let stop = speed * part.mass / dt;
            let f = if grip < stop { grip } else { stop };
            self.push(&m, scale(normal_of(across, speed), -f), at, part.friction_moment_scale);
        }
        self.body.contacts += 1;
    }

    /// A force into the wreck body at a point, with the part of its moment the file allows: the same
    /// shape as [`push`], which is how the wheels put theirs into the bike.
    fn push(&mut self, m: &Mat4, force: Vf, at: Vf, moment_scale: Vf) {
        let local = chassis::unrotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), force);
        let arm = sub_xyz(unpoint(m, at), self.body.cofg);
        self.body.force = add_xyz(self.body.force, force);
        let moment = cross(arm, local);
        self.body.torque = add_xyz(self.body.torque, moment.merge(Dest::XYZ, moment.mul(moment_scale)));
    }

    /// **What the ride state machine waits for**: the rider is on the ground.
    ///
    /// This is [`crate::state::Sense::settles`], which the port could not set before — see this
    /// type's own docs for what was measured and what that rules out.
    pub fn settles(&self) -> bool {
        self.landed
    }

    /// Where the bike goes back to, and which way it faces there: where the wreck body ended up, and
    /// the way the bike was pointing when it wrecked, flattened.
    ///
    /// **Measured for the position, the port's own for the facing.** The original's reset puts the
    /// rider back at the wreck and not at the spawn: in a live reason-5 wreck the respawned bike
    /// stood at `(4294.28, 136.99, 3473.11)` and the wreck body had come to rest at
    /// `(4294.79, 137.25, 3470.86)`, half a foot and two feet away, while its spawn was most of a lap
    /// behind it. Which way it points there is not measured — the game has a track to face along and
    /// this port does not — so it keeps the heading it wrecked with.
    pub fn rest(&self) -> (Vf, Vf) {
        (self.origin, self.facing)
    }
}

/// `across / speed`, where the length is already known: the slide's direction.
fn normal_of(across: Vf, speed: F) -> Vf {
    scale(across, F::ONE / speed)
}

impl State for Crash {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u8("reason", self.reason.0);
        w.i32("hit", self.hit as i32);
        w.group("throw", |w| {
            w.f("force_time", self.throw.force_time);
            w.f("hand_hold_time", self.throw.hand_hold_time);
            w.vf("force", self.throw.force);
            w.f("force_time_delta", self.throw.force_time_delta);
            w.f("hand_hold_time_delta", self.throw.hand_hold_time_delta);
            w.vf("force_delta", self.throw.force_delta);
        });
        w.f("clock", self.clock);
        w.bool("holding", self.holding);
        w.f("throwing", self.throwing);
        w.group("body", |w| walk_chassis(w, &self.body));
        w.vf("origin", self.origin);
        w.vf("rider", self.rider);
        w.vf("rider_vel", self.rider_vel);
        w.bool("landed", self.landed);
        w.f("ground", self.ground);
        w.bool("resting", self.resting);
        w.vf("facing", self.facing);
    }
}
