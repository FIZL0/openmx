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
use crate::camera::CurveTable;
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
use crate::wreck_bike::{NoObjects, WreckBike};
use ps2_float::{Acc, Dest, F, Field, Vf, fpu, libm, vu};

/// World up, with `w` of one, as every module here holds it.
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);
/// The quadword the game's rodata holds for "nothing": a point at the origin.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// How many probes the collision bundle carries, and what each one is for — see [`Ride::bundle`].
const PROBES: usize = 4;


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
pub(crate) fn bike_ang_limits(p: &Phy) -> (F, [F; 3], [F; 3]) {
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
    /// A split brake's two halves. The original's pad has no command for either (P73), so a pad
    /// leaves them at 0; [`Held`] brakes the front with the larger of this and the brake.
    pub front_brake: F,
    pub rear_brake: F,
    /// The clutch lever.
    pub clutch: F,
    /// Reverse, which is a command and not a gear: held, it pushes the bike backwards. No button of
    /// the original's is it (P73); a pad leaves it at 0.
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
    /// The rider is balancing on one wheel. The original's table has no such command (P73); a pad
    /// leaves it false.
    pub balance: bool,
    /// Put the bike back: on [`Ride::back`], the last spot the bike was kept fit to stand on (S66).
    pub respawn: bool,
    /// **The port's own (MD16):** Cross and Square as trick buttons, for the extra stunts a trick pack
    /// gives ([`trick::group_with`]). No binding of the original's is either; the shell sets them on
    /// the new scheme with MX vs. ATV's tricks on, and a rider whose set has no extras ignores them.
    pub extra: [bool; trick::EXTRA_BUTTONS],
}

impl Controls {
    /// What one pad is asking for, through the ported input layer and the shipped binding table.
    pub fn from_pad(pad: &crate::input::Pad) -> Controls {
        Controls::through(pad, &crate::bindings::Table::SHIPPED)
    }

    /// What one pad is asking for, every command looked up in `table` (P73) the way the original's
    /// bike reads it: the levers, the trick buttons and the d-pad by their value on the first
    /// binding, the reset chord by its two commands' events. A control the table leaves out of
    /// these commands does nothing here, and a command it leaves unbound reads nothing.
    ///
    /// What the original does not have is not read: it has no front brake of its own (the brake is
    /// both wheels), no reverse button (it reverses on gas and brake held together, slow) and no
    /// balance button, so those three stay at rest. The levers are still read as their strength and
    /// rate-limited by [`Held`]; the original's bike makes each a switch first (see the journal).
    /// The strengths and press timers come from [`crate::input::Pad`]'s ported poll. It lives here
    /// rather than in the shell so that a replay fixture and the shell derive the rider's hands from
    /// a pad the same way.
    pub fn through(pad: &crate::input::Pad, table: &crate::bindings::Table) -> Controls {
        use crate::bindings::Command;
        use crate::input::{Axis, AxisRange};
        let held = |c: Command| table.held(pad, c);
        let pressed = |c: Command| table.pressed(pad, c);
        // The eight-way code the trick table reads, from the d-pad's four commands: odd for a
        // single direction, and the game's own order round the d-pad. A diagonal is not derived,
        // because which of two held directions wins is the front end's business.
        let round = [Command::UP, Command::RIGHT, Command::DOWN, Command::LEFT];
        Controls {
            gas: table.value(pad, Command::GAS),
            brake: table.value(pad, Command::BRAKE),
            front_brake: F::ZERO,
            rear_brake: F::ZERO,
            clutch: table.value(pad, Command::CLUTCH),
            reverse: F::ZERO,
            // Through the player's own range, ±0.3, which is what the ride's stick consumers carry
            // in the original (I7, proved on a playback of the human's recording) — not the main
            // menu's ±0.2 that [`crate::input::Pad::axis`] hands out.
            stick: (pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), pad.axis_through(Axis::LeftY, &AxisRange::PLAYER)),
            dpad_y: held(Command::UP) || held(Command::DOWN),
            dpad_dir: round.iter().position(|c| held(*c)).map_or(0, |i| i as i32 * 2 + 1),
            stunt: held(Command::TRICK_1),
            // The three trick buttons, in the order the decoder tests them: the order is what numbers
            // the six groups and so which of the 24 slots a gesture names.
            trick: [held(Command::TRICK_1), held(Command::TRICK_2), held(Command::TRICK_3)],
            dpad: [held(Command::LEFT), held(Command::RIGHT), held(Command::UP), held(Command::DOWN)],
            balance: false,
            // The reset chord (P20): the original's hand reset is an event handler on two commands,
            // and fires on the press of either while the other is held, so whichever goes down second
            // is the tick the rider is put back. In the shipped table they are L1 and R1, which are
            // also the clutch and the third trick button, there as here. Both held from before, with
            // no press this frame, is not a chord.
            respawn: (pressed(Command::RESET_LEFT) && held(Command::RESET_RIGHT))
                || (pressed(Command::RESET_RIGHT) && held(Command::RESET_LEFT)),
            extra: [false; trick::EXTRA_BUTTONS],
        }
    }
}

impl Controls {
    /// These controls with the d-pad out of the ride (P93): the shell's, where the d-pad is the
    /// session markers' ([`crate::markers`]) and the left stick alone leans, steers, moves the rider
    /// fore and aft and is the direction half of a trick's gesture, which the original reads too. The
    /// d-pad reads as let go whatever else is held (P129): a trick button with it does no trick, and
    /// its gesture direction, eight-way code and digital stick are all at rest.
    /// [`Controls::through`] stays the original's, d-pad and all, which every replay of it rides on.
    pub fn stick_alone(self) -> Controls {
        Controls { dpad_y: false, dpad_dir: 0, dpad: [false; trick::DPAD], ..self }
    }

    /// These controls with the reset chord pressed *together* (P67): the shell's new layout, where L1
    /// is the clutch a rider holds through a launch and R1 is still the third trick button. The
    /// original's chord is the press of either half while the other is held, however long it has
    /// been, so a clutch held for a second and then a trick on R1 puts the bike back. Here the half
    /// that was already down counts only if it went down at most [`CHORD_TOGETHER_MS`] before the
    /// press; a clutch held longer is the clutch, and R1 on it is the trick alone. Both halves still
    /// go to the ride as the clutch and the trick. Read off the pad's own held timers, so a replay of
    /// the same reports puts the bike back on the same ticks. [`Controls::through`] stays the
    /// original's chord, which every replay of it rides on.
    pub fn chord_together(self, pad: &crate::input::Pad, table: &crate::bindings::Table) -> Controls {
        use crate::bindings::Command;
        let fresh = |c: Command| {
            table.control(pad.index as usize, c).is_some_and(|k| pad.controls[k.index()].held_ms <= CHORD_TOGETHER_MS)
        };
        let half = |pressed: Command, other: Command| {
            table.pressed(pad, pressed) && table.held(pad, other) && fresh(other)
        };
        let respawn = self.respawn
            && (half(Command::RESET_LEFT, Command::RESET_RIGHT) || half(Command::RESET_RIGHT, Command::RESET_LEFT));
        Controls { respawn, ..self }
    }
}

/// How long before the second half of the reset chord the first may have gone down under the new
/// layout ([`Controls::chord_together`], P67), in the input layer's milliseconds. The human's own
/// chord in recording2 is one 30 Hz tick apart; a quarter of a second is a deliberate two-button press
/// with room to spare, and well short of a clutch held to rev the engine up for a launch.
pub const CHORD_TOGETHER_MS: u32 = 250;

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
    /// `+0x64`: one over the wheel's spin inertia, which `wheel_set_tire` makes out of the radius
    /// alone ([`tire::spin_inertia`]).
    pub inv_inertia: F,
    /// `+0x68`: the file's `TireInertia`, which only the gyroscopic torque reads.
    pub gyro: F,
    /// `+0x1d0`: how far round the wheel has turned, wrapped to ±π.
    pub angle: F,
    /// `+0x190`: the chassis' velocity in its own frame, as the contact velocity last measured it.
    pub chassis_local_velocity: Vf,
    /// `+0x840` and `+0x850`: the contact's velocity in the [`tire::contact_frame`], and the tire's
    /// force in it (side, 0, forward).
    pub local_velocity: Vf,
    pub local_force: Vf,
    /// `+0x8d0..+0x910`: the contact frame the tire last pushed along.
    pub frame: [Vf; 4],
    /// `+0x234`: the side force per pound of load.
    pub side_per_load: F,
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
        let inertia = tire::spin_inertia(radius);
        Wheel {
            mv: mv::Wheel {
                max_steer_angle: t.max_steer_angle,
                radius,
                inv_radius: inertia.map_or(F::ZERO, |i| i.0),
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
            inv_inertia: inertia.map_or(F::ZERO, |i| i.2),
            gyro: t.tire_inertia,
            angle: F::ZERO,
            chassis_local_velocity: Vf::ZERO,
            local_velocity: Vf::ZERO,
            local_force: Vf::ZERO,
            frame: [Vf::ZERO; 4],
            side_per_load: F::ZERO,
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

// --- the put-back -----------------------------------------------------------------------------------

/// **Where a put-back sets the bike down** (S66): the original's record of the last good spot. Both
/// the L1+R1 chord and the machine's own reset after a wreck place the bike here, unchanged, and
/// [`Ride::keep_back`] rewrites it every frame the ground under the bike is fit to stand on — so a
/// put-back lands where the bike last was, and at the ride's start only when it has not been
/// anywhere else (`research/journal/2026-10-21-s66-hand-reset-position`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Back {
    /// On the ground: the probe's `x`, `z` and `w`, the ground's height.
    pub at: Vf,
    /// The heading with its height taken out, unit length, `w` the heading's.
    pub facing: Vf,
    /// The ground's normal there, as the ground query wrote it — the up the bike is stood on.
    pub up: Vf,
}

impl Default for Back {
    fn default() -> Back {
        Back { at: NOTHING, facing: Vf::new(0.0, 0.0, 1.0, 1.0), up: UP }
    }
}

/// Where the record's probe stands on the bike while it is riding: the bike collision record's sweep
/// offset, three feet up the chassis' own up axis, taken through the transform the frame starts with.
const BACK_PROBE: Vf = Vf::new(0.0, 3.0, 0.0, 1.0);
/// A spot whose ground normal is not steeper than this is not kept.
const BACK_LEVEL: f32 = 0.8;
/// The height the record's point is raised along the old up: a word of the bike's that its
/// constructor leaves zero and both captured tracks kept zero.
const BACK_RAISE: f32 = 0.0;
/// The clearance test's ring: fourteen probes — one straight down, twelve around at knee height, one
/// along the heading at head height — within forty feet.
const BACK_PROBES: usize = 14;
const BACK_REACH: f32 = 40.0;
/// The probe above the spot reaches this far over it, and the query around it is this wide.
const BACK_OVER: f32 = 10.0;
const BACK_OVER_REACH: f32 = 1000.0;
const DOWN: Vf = Vf::new(0.0, -1.0, 0.0, 1.0);

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
    /// Where a put-back sets the bike down (S66), kept up to date by [`Ride::keep_back`].
    pub back: Back,
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
    /// The bike's air drag is halved: the cannonball's throw halves it and the bike's put-back doubles
    /// it again ([`crate::off_map::drag`]). The original keeps the drag itself on the bike, a copy of
    /// the `.phy`'s; the port keeps the `.phy`'s on [`Phy`] and only this, which is the same value
    /// whenever the halving undoes exactly (every normal drag).
    pub drag_halved: bool,
    /// The speed limit, squared, a put-back gives the chassis back: the original's
    /// [`crate::off_map::RIDING_MAX_SPEED`] squared, which is the limit a bike is built with, unless a
    /// modifier that makes the bike faster raised both ([`crate::modifiers::Mods::ride`], P97).
    pub riding_speed_sq: F,
    /// **The port's own (MD16):** MX vs. ATV's moves' state, made on the first frame of a bike that
    /// carries a trick pack's numbers ([`Phy::tricks`]). `None` on every ride on the original's tricks,
    /// and the hash walk leaves it out then, so their hashes do not move.
    pub tricks: Option<Box<crate::trick_pack::State>>,
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
            // The constructor's record is the spawn, stood upright.
            back: Back { at, facing, up: UP },
            near: F::ONE,
            touching: false,
            stunt: false,
            drag_halved: false,
            riding_speed_sq: F(BIKE_MAX_SPEED_SQ),
            tricks: None,
        };
        ride.place(at, facing);
        ride
    }

    /// **The bike's numbers changed under a ride going on** (P98): what [`Ride::new`] copied out of the
    /// `.phy` at the start, taken again from `p`, and nothing the ride has done since touched. The
    /// engine's tuning (its curve, gearing, clutch and braking, with the gear's ratio worked out again
    /// for the gear it is in) and each wheel's damping, which is built off the file's spring; the rest
    /// of what the step reads of the file it reads off the `.phy` every tick. Everything the game
    /// modifiers ([`crate::modifiers`]) reach is one of those, so a ride retuned onto a modified file
    /// rides on, from where it is, as one built on it would. The integrator's speed limit is the
    /// ride's own and not the file's: [`crate::modifiers::Mods::rebike`] moves it.
    pub fn retune(&mut self, p: &Phy) {
        let fresh = Ride::new(p, (self.front.w.mount, self.rear.w.mount), self.spawn, self.spawn_facing);
        let (e, f) = (&mut self.engine, &fresh.engine);
        e.gears = f.gears;
        e.final_drive = f.final_drive;
        e.gear_ratios = f.gear_ratios;
        e.rev_limit = f.rev_limit;
        e.idle_throttle = f.idle_throttle;
        e.engine_braking = f.engine_braking;
        e.inv_inertia = f.inv_inertia;
        e.clutch_static = f.clutch_static;
        e.clutch_kinetic = f.clutch_kinetic;
        e.min_rpm = f.min_rpm;
        e.curve_rpm = f.curve_rpm;
        e.curve_torque = f.curve_torque;
        let gear = e.gear;
        mv::set_gear(e, gear);
        for (w, f) in [(&mut self.front, &fresh.front), (&mut self.rear, &fresh.rear)] {
            w.w.damping_out = f.w.damping_out;
            w.w.damping_in = f.w.damping_in;
        }
    }

    /// **Ride on as `other` rides** (P111, a teleport onto another player): `other`'s whole motion
    /// — where it is and which way it faces, the body's speed and spin and the integrator's history
    /// of them, the suspension, the wheels' spin, the engine's revs and gear, the air state and the
    /// controls as the rate limit left them — on this bike's own build. What is the bike's and not
    /// its motion stays this ride's: the `.phy`'s numbers [`Ride::new`] copied (the chassis' mass,
    /// inertia, centre of gravity and limits, the engine's tuning, each wheel's tyre, strut and
    /// steering), and what is the rider's (the spawn, the put-back record, the riding limit). Two
    /// rides on one build come out of it identical in everything but those last, so given the same
    /// controls they step on along one path.
    pub fn ride_as(&mut self, other: &Ride) {
        let own = std::mem::replace(self, other.clone());
        (self.spawn, self.spawn_facing, self.back) = (own.spawn, own.spawn_facing, own.back);
        (self.riding_speed_sq, self.drag_halved) = (own.riding_speed_sq, own.drag_halved);

        let (c, o) = (&mut self.chassis, &own.chassis);
        (c.inv_mass, c.inv_inertia, c.free, c.ang_free) = (o.inv_mass, o.inv_inertia, o.free, o.ang_free);
        (c.max_speed_sq, c.max_ang_speed_sq, c.ang_vel_max, c.ang_vel_min) =
            (o.max_speed_sq, o.max_ang_speed_sq, o.ang_vel_max, o.ang_vel_min);
        if c.cofg != o.cofg {
            // Another build's centre of gravity sits elsewhere in the same frame: put it there.
            c.cofg = o.cofg;
            let m = held_transform(&self.node, self.chassis.quat, self.origin);
            self.chassis.cofg_world = math::transform_point(&m, self.chassis.cofg);
        }

        let (e, o) = (&mut self.engine, &own.engine);
        (e.gears, e.final_drive, e.gear_ratios, e.rev_limit) = (o.gears, o.final_drive, o.gear_ratios, o.rev_limit);
        (e.idle_throttle, e.engine_braking, e.inv_inertia) = (o.idle_throttle, o.engine_braking, o.inv_inertia);
        (e.clutch_static, e.clutch_kinetic, e.min_rpm) = (o.clutch_static, o.clutch_kinetic, o.min_rpm);
        (e.curve_rpm, e.curve_torque, e.torque_scale) = (o.curve_rpm, o.curve_torque, o.torque_scale);
        let gear = e.gear.min(e.gears);
        mv::set_gear(e, gear);

        for (w, o) in [(&mut self.front, &own.front), (&mut self.rear, &own.rear)] {
            (w.mv.max_steer_angle, w.mv.radius, w.mv.inv_radius, w.mv.steered) =
                (o.mv.max_steer_angle, o.mv.radius, o.mv.inv_radius, o.mv.steered);
            let (b, k) = (&mut w.w, &o.w);
            (b.mount, b.probe, b.probe_length, b.max_compression) = (k.mount, k.probe, k.probe_length, k.max_compression);
            (b.max_damper_speed, b.extension_rate, b.spring_rate) = (k.max_damper_speed, k.extension_rate, k.spring_rate);
            (b.damping_out, b.damping_in, b.steered, b.steer_damping) = (k.damping_out, k.damping_in, k.steered, k.steer_damping);
            (b.steer_inv_inertia, b.max_steer_angle, b.min_steer_angle) =
                (k.steer_inv_inertia, k.max_steer_angle, k.min_steer_angle);
            (w.inv_inertia, w.gyro) = (o.inv_inertia, o.gyro);
        }
    }

    /// **Thrown from a spot** (P141, a teleport onto a player flying the plane): set down at `at`
    /// along `facing` and `up` as [`Ride::place_on`] sets it, and the body then moving at `vel`
    /// (`xyz`, feet a second) and not turning, in the air until the ground step finds the ground
    /// under it, as a bike leaving a jump is.
    pub fn launch(&mut self, at: Vf, facing: Vf, up: Vf, vel: Vf) {
        self.place_on(at, facing, up);
        let vel = Vf::ZERO.merge(Dest::XYZ, vel);
        self.chassis.vel = vel;
        self.chassis.local_vel = chassis::unrotate(&Mat3::new(self.node[0], self.node[1], self.node[2]), vel);
        self.chassis.speed = Acc::mula(vel.x(), vel.x()).madda(vel.y(), vel.y()).madd(vel.z(), vel.z()).sqrt();
    }

    /// The bike's put-back, its chassis' half ([`crate::off_map::unwreck`]), back to this ride's own
    /// riding limit: the original's, unless a modifier raised it.
    pub fn unwreck(&mut self) {
        crate::off_map::unwreck(&mut self.chassis);
        self.chassis.max_speed_sq = self.riding_speed_sq;
    }

    /// Put the bike down at a point, pointed a way, stopped: the respawn, less the ride state
    /// machine's own clocks (S5's, and P3's to wire).
    pub fn place(&mut self, at: Vf, facing: Vf) {
        self.place_on(at, facing, UP);
    }

    /// **The put-back** (S66): the bike set down on [`Ride::back`] — its point, its heading, and the
    /// ground's normal as the up, which the look-at keeps while the heading is turned square to it.
    pub fn place_back(&mut self) {
        let Back { at, facing, up } = self.back;
        self.place_on(at, facing, up);
    }

    /// [`Ride::place`] with an up axis: the look-at orthogonalises and keeps the up, the original's
    /// model setup with its two flags.
    pub fn place_on(&mut self, at: Vf, facing: Vf, up: Vf) {
        let m = math::basis(at, facing, up, true, false);
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

    /// **The put-back record's update** (S66), once a frame after the bike has moved: the original's
    /// race manager hands the bike its heading, and the bike looks under a probe for a spot fit to be
    /// put back on.
    ///
    /// `at` is the probe — while riding [`Ride::frame`]'s own (the sweep point the frame started
    /// with, at the body's height), while down the wreck body's centre — and `heading` the bike's
    /// forward row as the frame left it. The spot is the ground straight under the probe. It is kept
    /// only when nothing solid stands over it between the probe and the ground, nothing stands in the
    /// way of [`clear`]'s ring of probes around it, and the ground there is no steeper than
    /// [`BACK_LEVEL`]; otherwise the record keeps the last spot that was. In a world of several
    /// terrains the world must also keep the spot ([`World::keeps`], MD14). `true` when it moved.
    pub fn keep_back(&mut self, at: Vf, heading: Vf, world: &mut dyn World, curve: &CurveTable) -> bool {
        let Some(ground) = open_sky(at, world) else {
            return false;
        };

        let spot = Vf::new(at.x(), ground.0, at.z(), at.w());
        // A world of several terrains keeps no spot over a hole or by its edge (MD14).
        if !world.keeps(spot) {
            return false;
        }
        let Some((spot, normal)) = clear(spot, heading, world, curve) else {
            return false;
        };
        if !(F(normal.y()) > F(BACK_LEVEL)) {
            return false;
        }
        self.back = Back {
            at: add_xyz(spot, scale(self.back.up, F(BACK_RAISE))),
            facing: flat_unit(heading),
            up: normal,
        };
        true
    }

    /// **The put-back record's update in a cannonball** (S77): the original's own branch for a bike
    /// shot off the map, which keeps the record on the wreck bike — so the put-back lands where it
    /// came down inside the map, not at the edge it left.
    ///
    /// The race manager takes it while the request's reason is the cannonball and the wreck bike's
    /// collision record says it touched something this tick ([`crate::contact_search::Record::touching`]);
    /// the caller asks both. `over` is the probe the overhead test stands on, the wreck part's point
    /// as in any wreck; `at` and `forward` are the wreck bike's transform's translation and forward
    /// row. The spot handed to [`clear`] is the translation itself, at the node's height — the ground
    /// query moves it down — with a `w` the original leaves as its stack had it, which every captured
    /// tick had zero; the heading is the forward row flattened once here, which [`clear`] flattens
    /// again for its ring. The ground's normal goes straight into the record whether or not the spot
    /// is clear, and a clear spot is kept with no slope test and no raise. `true` when it moved.
    pub fn keep_back_thrown(&mut self, over: Vf, at: Vf, forward: Vf, world: &mut dyn World, curve: &CurveTable) -> bool {
        if open_sky(over, world).is_none() {
            return false;
        }
        // Nor while the wreck is over a world's hole or by its edge (MD14): the record, its up too,
        // is left as it was, so the put-back lands where the bike last was on ground.
        if !world.keeps(at) {
            return false;
        }
        let facing = flat_unit(forward);
        let spot = Vf::new(at.x(), at.y(), at.z(), 0.0);
        let (spot, normal, clear) = clearance(spot, facing, world, curve);
        self.back.up = normal;
        if clear {
            self.back.at = spot;
            self.back.facing = facing;
        }
        clear
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

/// `v` with its height taken out and the rest made unit length, `w` kept: the `vmul`/`vadday`/`vmaddz`
/// sum of squares and `vrsqrt` the put-back's heading is flattened with (S66).
fn flat_unit(v: Vf) -> Vf {
    let flat = Vf::new(v.x(), 0.0, v.z(), v.w());
    flat.merge(Dest::XYZ, flat.mul_q(vu::rsqrt(1.0, vu::dot3(flat.0, flat.0))))
}

/// **The put-back's clearance test** around a spot (S66), and the ground there.
///
/// Fourteen probes go out from the spot together: one straight down from ten feet over it, twenty
/// long; twelve level ones three feet up, twenty long, fanned out around the heading by the game's
/// `{cos, sin}` table; and one along the heading six and a half feet up, from twenty feet behind to
/// twenty ahead. The ground query then moves the spot onto the ground and says its normal. The probe
/// straight down only lifts the spot — onto anything solid it meets above the ground, with that
/// thing's normal; any of the other thirteen meeting something refuses the spot, and so does the world
/// saying the ring is [`World::crowded`] (S75), which the original's query answers in the same pass.
fn clear(spot: Vf, heading: Vf, world: &mut dyn World, curve: &CurveTable) -> Option<(Vf, Vf)> {
    let (spot, normal, clear) = clearance(spot, heading, world, curve);
    clear.then_some((spot, normal))
}

/// The ground under a probe, when nothing solid stands between it and the ground: the record
/// update's overhead test (S66), one probe from ten feet over the point straight down to the ground,
/// in a query a thousand feet wide. `None` is a roof — something met above the ground.
fn open_sky(at: Vf, world: &mut dyn World) -> Option<F> {
    let (under, _) = world.ground(at);
    let ground = F(under.y());
    let mut over = Bundle { count: 1, centre: at, radius: F(BACK_OVER_REACH), ..Bundle::default() };
    over.probes[0] = Probe {
        origin: Vf::new(at.x(), (F(at.y()) + F(BACK_OVER)).0, at.z(), 1.0),
        dir: DOWN,
        length: (F(at.y()) - ground) + F(BACK_OVER),
    };
    let mut found = Record::new();
    world.solid(&over, &mut found);
    (!(found.hit[0] && ground < F(found.point[0].y()))).then_some(ground)
}

/// [`clear`]'s whole answer: the spot on the ground (or lifted), its normal, and whether the spot is
/// clear. The spot and normal are what the original's test leaves in its outputs either way, which
/// the cannonball's branch reads (S77): its normal is the record's own word.
fn clearance(spot: Vf, heading: Vf, world: &mut dyn World, curve: &CurveTable) -> (Vf, Vf, bool) {
    let flat = flat_unit(heading);
    let (fx, fz) = (F(flat.x()), F(flat.z()));
    let y = F(spot.y());
    let mut ring = Bundle { count: BACK_PROBES, centre: spot, radius: F(BACK_REACH), ..Bundle::default() };
    ring.probes[0] = Probe {
        origin: Vf::new(spot.x(), (y + F(10.0)).0, spot.z(), 1.0),
        dir: DOWN,
        length: F(20.0),
    };
    let around = BACK_PROBES - 1;
    for i in 1..around {
        let a = ((F(fpu::from_i32(i as i32)) - F::ONE) * F(6.2831855)) / F(fpu::from_i32(around as i32));
        let (cos, sin) = curve.coarse(a);
        ring.probes[i] = Probe {
            origin: Vf::new(spot.x(), (y + F(3.0)).0, spot.z(), 1.0),
            dir: Vf::new((fz * sin + fx * cos).0, 0.0, (fz * cos - fx * sin).0, 1.0),
            length: F(20.0),
        };
    }
    let lift = Vf::new(6.5, 6.5, 6.5, 6.5).bc(Field::X);
    let back = Vf::new(20.0, 20.0, 20.0, 20.0).bc(Field::X);
    ring.probes[around] = Probe { origin: spot.add(UP.mul(lift)).sub(flat.mul(back)), dir: flat, length: F(40.0) };

    let (on, mut normal) = world.ground(spot);
    let mut spot = Vf::new(spot.x(), on.y(), spot.z(), spot.w());
    let mut found = Record::new();
    world.solid(&ring, &mut found);
    if found.hit[0] && F(spot.y()) < F(found.point[0].y()) {
        spot = Vf::new(spot.x(), found.point[0].y(), spot.z(), spot.w());
        normal = found.normal[0];
    }
    let refused = found.hit[1..BACK_PROBES].iter().any(|&hit| hit) || world.crowded(ring.centre, ring.radius);
    (spot, normal, !refused)
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

// --- what the tire does --------------------------------------------------------------------------

/// The wheel helper that measures its contact velocity: how fast the load point is moving — the
/// body's own velocity plus what the body's spin adds at the arm from its centre of mass.
///
/// `wheel_contact_velocity` takes its arm from the load point (`+0x140`) less the chassis' centre of
/// gravity in the world (`+0x60`), and adds `ω × arm` to the body's velocity — read off the
/// decompilation for S58, and not proved against the original on its own: the other-body half of
/// that function (a contact on something that moves) is not ported.
fn contact_velocity(vel: Vf, world_ang_vel: Vf, at: Vf, cofg_world: Vf) -> Vf {
    add_xyz(vel, cross(world_ang_vel, sub_xyz(at, cofg_world)))
}

/// The chassis transform's three questions, answered with its own arithmetic.
fn ask_transform(m: &Mat4, call: wheel::Call) -> Vf {
    let rot = Mat3::new(m.0[0], m.0[1], m.0[2]);
    match call {
        wheel::Call::Rotate(v) => chassis::rotate(&rot, v),
        wheel::Call::Unrotate(v) => chassis::unrotate(&rot, v),
        wheel::Call::Unpoint(v) => unpoint(m, v),
        _ => Vf::ZERO,
    }
}

/// The steering hinge's force, which [`wheel::tire_forces`] hands a steered wheel the bike is
/// balancing on: [`tire::hinge_force`] at a scale of exactly one.
fn hinge_force(wh: &mut Wheel, c: &mut wheel::Chassis, m: &Mat4, force: Vf, at: Vf) {
    tire::hinge_force(wh, c, F::ONE, force, at, &mut |call| ask_transform(m, call));
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
    let reach = belly_reach(m, cofg, front, rear);
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

/// How far [`bundle`]'s straight-down probe reaches below the centre of mass: as far as the
/// suspension plus a wheel, but never deeper, along the chassis' own down, than a wheel's radius
/// short of the shallower of the two suspension lines' ends (S89), so that on anything flat it only
/// catches once the suspension is a radius into its travel. A longer one finds a flat prop first —
/// free06's railway bed — and holds the bike up on its belly where neither wheel's line reaches it:
/// no contact, no tire force, and the bike slides. The port's own, like the rest of the bundle, until the original's
/// body rays replace it (S86/S87).
fn belly_reach(m: &Mat4, cofg: Vf, front: &Wheel, rear: &Wheel) -> F {
    let up = direction(m, UP);
    let mut reach = front.w.probe_length + front.mv.radius;
    for w in [front, rear] {
        let end = math::transform_point(m, add_xyz(w.w.mount, w.w.probe));
        reach = reach.min(dot3(sub_xyz(cofg, end), up) - w.mv.radius);
    }
    reach.max(F(BELLY_MIN_REACH))
}

/// The shortest [`belly_reach`]: a probe of no length sweeps nothing.
const BELLY_MIN_REACH: f32 = 0.05;

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
    wh.w.chassis_velocity = mvc.vel;

    // The arm is the load point's, and the step works the load point out before it asks for the
    // velocity, so the copy's answer is the one the real run's ask would have had (S58).
    let mut synced = false;
    {
        let (mut probe_w, mut probe_c) = (wh.w, view(mvc));
        let mut hinge = None;
        let mut ask = |call| answer(m, call, &mut hinge, &mut synced);
        wheel::tire_forces(&mut probe_w, &mut probe_c, &m.0, dt, &mut ask);
        drop(ask);
        let vel = if synced { probe_c.vel } else { mvc.vel };
        wh.w.contact_velocity =
            contact_velocity(vel, mvc.world_ang_vel, probe_w.load_point, cofg_world);
        if probe_w.contact {
            // `+0x190`: the chassis' velocity in its own frame, which the surface drag reads.
            wh.chassis_local_velocity = chassis::unrotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), vel);
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

/// One wheel's own integration: [`tire::surface_drag`], [`tire::spin_integrate`] and then
/// [`wheel::integrate`]'s own steering, which is the original's order.
///
/// The two run before the ported call rather than from inside it, because the steering hinge the
/// ported half integrates reads the torque the spin step puts on it; the only difference is where
/// the `Call` list is built.
fn wheel_integrate(wh: &mut Wheel, tire: &phy::Tire, brake: F, other_contact: bool,
                   mvc: &mut mv::Chassis, m: &Mat4, curve: &CurveTable, dt: F) -> Vec<wheel::Call> {
    wh.to_wheel();
    let surf = surface(tire, wh.surface);
    let mut wc = wheel::Chassis {
        force: mvc.force,
        torque: mvc.torque,
        accel: mvc.accel,
        cofg: mvc.cofg,
        vel: mvc.vel,
    };
    let rows = Mat3::new(m.0[0], m.0[1], m.0[2]);
    tire::surface_drag(wh, &surf, brake, &mut wc, &mut |v| chassis::rotate(&rows, v));
    let step = tire::Step {
        dt,
        brake,
        parametric: tire.use_parametric_curve,
        other_contact,
        arm: tire.body_tire_force_position,
        forward: m.0[2],
        ang_vel: mvc.ang_vel,
    };
    tire::spin_integrate(wh, &surf, &step, &mut wc, curve, &mut |call| ask_transform(m, call));
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
        self.frame_bumped(p, tables, c, dt, world, None)
    }

    /// [`Ride::frame`] with the other riders' contacts (S78): `bump`'s list is resolved against the
    /// chassis where the substep opens its collision record, which is the original's ground step's
    /// call. With none it is `frame`.
    pub fn frame_bumped(&mut self, p: &Phy, tables: &Tables, c: &Controls, dt: F,
                        world: &mut dyn World, mut bump: Option<&mut crate::rider_contact::Bump>) -> Vec<mv::Call> {
        if c.respawn {
            self.place_back();
        }
        let Ride {
            mv: ride, chassis, engine, air, front, rear, origin, node, held, near, touching, stunt,
            drag_halved, tricks, ..
        } = self;
        *touching = false;
        // MX vs. ATV's moves (MD16), only for a bike a trick pack is laid over.
        if p.tricks.is_some() && tricks.is_none() {
            *tricks = Some(Box::default());
        }

        // 1 — the input layer's rate limit.
        held.step(c, &p.joystick, dt);
        if let Some(t) = tricks.as_deref_mut() {
            let raw = trick::stick(c.stick.0, c.stick.1, c.dpad);
            t.side_flick = t.side.sample(raw.x, dt);
        }

        // 2 — the chassis' measure, which is where the pitch and roll `air.rs` reads come from. The
        // euler angles are the one call it makes that this crate ports.
        let euler = math::quat_to_euler(chassis.quat, &tables.acos);
        chassis::measure(chassis, dt, euler);

        // The chassis' transform as the last frame left it — the no-hander's roll included, which the
        // body's quaternion only takes in at its next integration (P32).
        let body = held_transform(node, chassis.quat, *origin);
        // The put-back's probe: the sweep's point, three feet up the frame as it started (S66).
        let swept = math::transform_point(&body, BACK_PROBE);

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
            // R1 alone: what MX vs. ATV's air moves are asked for under (MD16). Read only by them.
            modifier: tricks.is_some() && c.trick == [false, false, true],
            spinning: false,
            spin_active: false,
            spin_ramp: false,
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
            match (p.tricks.as_ref(), tricks.as_deref_mut()) {
                (Some(moves), Some(t)) => {
                    astep.spinning = t.spin.applied;
                    astep.spin_active = t.spin.active;
                    air::unleashed_rotate(&p.air, &mut t.flip_row, air, &mut ac, &astep);
                    air::three_sixty(&moves.spin, &p.air, air, &mut t.spin, &mut ac, &astep, t.side_flick, &tables.acos);
                    astep.spinning = t.spin.applied;
                    astep.spin_ramp = t.spin.ramp;
                }
                _ => air::backflip(&p.air, air, &mut ac, &astep),
            }
            mvc.force = ac.force;
            mvc.torque = ac.moment;

            mv::engine_update(engine, &mut rear.mv, s.gas_rate_limited, ride.clutch_in, h);
            mvc.force = add_xyz(mvc.force, mv::air_drag(crate::off_map::drag(mvp.drag, *drag_halved), &mvc));

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
                // The original's query writes the surface at the ray's last end, hit or miss (S89).
                if wh.w.has_ground
                    && let Some(s) = world.wheel_surface(wh.w.probe_end_world)
                {
                    wh.surface = s;
                }
                wh.from_wheel();
            }

            for (side, wh) in [(Side::Front, &mut *front), (Side::Rear, &mut *rear)] {
                calls.push(mv::Call::TireForces(side, h));
                tire_forces(wh, &mut mvc, chassis.cofg_world, &out.chassis, h);
            }
            rear.mv.other_tire_down = front.mv.contact && !rear.mv.contact;

            calls.push(mv::Call::WheelIntegrate(Side::Front, h));
            let (brake, other) = (held.front_brake, rear.w.contact);
            wheel_integrate(front, &p.front, brake, other, &mut mvc, &out.chassis, &tables.curve, h);
            calls.push(mv::Call::WheelIntegrate(Side::Rear, h));
            let (brake, other) = (held.rear_brake, front.w.contact);
            wheel_integrate(rear, &p.rear, brake, other, &mut mvc, &out.chassis, &tables.curve, h);

            mv::engine_post(engine, &mut rear.mv);

            // The collision record is opened per substep in the original and kept across the frame's
            // meshes; here one is made where it is swept, so there is nothing to open.
            calls.push(mv::Call::CollisionBegin(k == 0));
            if let Some(b) = bump.as_deref_mut() {
                let i = &chassis.inv_inertia.rows;
                let mut own = crate::contact::Ridden {
                    chassis: crate::contact::Vehicle {
                        node: crate::rider_contact::node(&out.chassis),
                        cofg: mvc.cofg,
                        mass: p.mass.mass,
                        inv_mass: chassis.inv_mass,
                        inv_inertia: [F(i[0].x()), F(i[1].y()), F(i[2].z())],
                        velocity: mvc.vel,
                        rates: mvc.ang_vel,
                        shock: mvc.accel,
                        twist: mvc.ang_accel,
                    },
                    position: chassis.cofg_world,
                    force: mvc.force,
                    torque: mvc.torque,
                };
                b.resolve(&mut own, k == 0);
                mvc.vel = own.chassis.velocity;
                mvc.ang_vel = own.chassis.rates;
                mvc.accel = own.chassis.shock;
                mvc.ang_accel = own.chassis.twist;
                mvc.force = own.force;
                mvc.torque = own.torque;
            }
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

        // The race manager's turn, after the bike has moved: the put-back record (S66).
        let probe = Vf::new(swept.x(), chassis.cofg_world.y(), swept.z(), swept.w());
        let heading = node[2];
        self.keep_back(probe, heading, world, &tables.curve);

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
    let look = world.solid(&b, &mut record) | world.groups(&b, &mut record);

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
    world.groups(&b, &mut record);
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
    world.groups(&b, &mut record);
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
        w.vf("back.at", self.back.at);
        w.vf("back.facing", self.back.facing);
        w.vf("back.up", self.back.up);
        w.f("near", self.near);
        w.bool("touching", self.touching);
        w.bool("stunt", self.stunt);
        if let Some(t) = &self.tricks {
            w.field("tricks", &**t);
        }
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
    w.f("gyro", wh.gyro);
    w.f("angle", wh.angle);
    w.vf("chassis_local_velocity", wh.chassis_local_velocity);
    w.vf("local_velocity", wh.local_velocity);
    w.vf("local_force", wh.local_force);
    for (i, row) in wh.frame.iter().enumerate() {
        w.vf(["frame0", "frame1", "frame2", "frame3"][i], *row);
    }
    w.f("side_per_load", wh.side_per_load);
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

/// Which of `[Wrecks]`' seven throws a wreck reason uses, or none.
///
/// **The rider body actor's wreck entry is what decides it** (S53, `crate::ragdoll`'s
/// `Ragdoll::wreck_enter`): its switch on the reason picks the throw, and this is that switch with
/// the bones left out. Three reasons are thrown by no throw. A scenery hit and a joust (6, 7) let go
/// of the bars at once and hand the bones the bike's velocity, and a cannonball (13) is a
/// cartwheel. Every reason the switch does not know (0, 9, 10, 14 and up) does nothing past the
/// common part. Before S53 this was inferred from the names, and 6, 13 and the unknown ones were
/// given throws they do not have.
pub fn hit_of(reason: trick::Wreck) -> Option<phy::Hit> {
    match reason.0 {
        1 => Some(phy::Hit::Front),
        2 | 3 => Some(phy::Hit::Side),
        4 => Some(phy::Hit::Back),
        5 => Some(phy::Hit::Bottom),
        8 => Some(phy::Hit::FallOffFront),
        0xb => Some(phy::Hit::FallOffBack),
        0xc => Some(phy::Hit::Stunt),
        _ => None,
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
/// * **The bike**, as `Wreckbike.phy`'s one `[PhysicsPart1]` — a rigid body of its own with its
///   collision object, entered from the bike's own velocity and spin by [`Crash::begin`] (a port of
///   `bike_wreck_entry`'s body half) and run as the original's world node runs it
///   ([`crate::wreck_bike`], S69): the contact search against the terrain, then the world's four
///   substeps of gravity, contact resolution and integration.
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
    pub hit: Option<phy::Hit>,
    /// The hit's throw, or all zeros for a reason with none: the hands are off at once.
    pub throw: phy::Throw,
    /// Seconds since the entry. The machine's own clock is restarted when its latch arms; this one
    /// is not, because the hand-hold time is measured from the entry.
    pub clock: F,
    /// The rider still has hold of the bars.
    pub holding: bool,
    /// What is left of the throw's `ForceTime` once the hands are off. Counted down and not spent —
    /// see the type's own docs.
    pub throwing: F,
    /// The bike's wreck body and its collision object, and its transform's origin.
    pub bike: WreckBike,
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
        // The body where the bike's centre of mass is, turned as the bike is, with the entry's two
        // copies: `chassis + 0x260` into the body's velocity and `chassis + 0x290` — the spin in the
        // bike's own frame — into the body's rates.
        let mut bike = WreckBike::new(part, inertia, math::transform_point(&m, part.cofg), ride.chassis.quat);
        let still = Vf::new(0.0, 0.0, 0.0, 1.0);
        bike.body.velocity = still.merge(Dest::XYZ, ride.chassis.vel);
        bike.body.rates = still.merge(Dest::XYZ, spin.unwrap_or(ride.chassis.ang_vel));
        let (forward, _) = ride.axes();
        let flat = Vf::new(forward.x(), 0.0, forward.z(), 0.0);
        Crash {
            reason,
            hit,
            throw: hit.map_or_else(phy::Throw::default, |h| wrecks.throw(h)),
            clock: F::ZERO,
            holding: true,
            throwing: F::ZERO,
            bike,
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
            self.rider = add_xyz(self.bike.body.position, scale(UP, wrecks.rider_flail_height));
            self.rider_vel = self.bike.body.velocity;
        }

        // The body: the world node's tick, against the terrain and the ecosystem's trees; the world's
        // other objects are not searched (see `wreck_bike`), and a part with its collision off
        // searches nothing.
        if !part.collision_enabled {
            self.bike.tick(dt, None, &mut NoObjects);
        } else if let (field, _, Some(trees)) = world.wooded() {
            self.bike.tick_wooded(dt, field.as_deref(), trees);
        } else {
            let field = world.heightfield();
            self.bike.tick(dt, field.as_deref(), &mut NoObjects);
        }
        self.origin = self.bike.origin();
        self.resting = length(self.bike.body.velocity) <= part.speed_threshold
            && length(self.bike.body.rates) <= part.angular_rate_threshold;

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
            self.rider = self.bike.body.position;
            self.rider_vel = self.bike.body.velocity;
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

impl State for Crash {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u8("reason", self.reason.0);
        w.i32("hit", self.hit.map_or(-1, |h| h as i32));
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
        w.field("body", &self.bike);
        w.vf("origin", self.origin);
        w.vf("rider", self.rider);
        w.vf("rider_vel", self.rider_vel);
        w.bool("landed", self.landed);
        w.f("ground", self.ground);
        w.bool("resting", self.resting);
        w.vf("facing", self.facing);
    }
}
