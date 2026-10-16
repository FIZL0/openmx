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
//! balance controller's rider-wobble reads ([`noise`]), the node's Gram-Schmidt ([`set_axes`]), the
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
use crate::wheel;
use crate::world::World;
use ps2_float::{Acc, Dest, F, Field, Vf, fpu, libm, vu};

/// World up, with `w` of one, as every module here holds it.
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);
/// The quadword the game's rodata holds for "nothing": a point at the origin.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// How many probes the collision bundle carries, and what each one is for — see [`Ride::bundle`].
const PROBES: usize = 4;

/// The deflection past which the stick counts as held, for the stick-hold clock `air.rs` reads.
const STICK_HELD: f32 = 0.5;

/// The integrator's speed limits, which the original takes from a vehicle setup this port does not
/// have. Wide enough that no ride reaches them: what they are for here is to keep
/// [`chassis::integrate`]'s clamp from reading a zero limit as a standstill.
const MAX_SPEED: f32 = 1000.0;
const MAX_ANG_SPEED: f32 = 100.0;

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
        use crate::input::{Axis, Control};
        let dpad = [Control::Up, Control::Right, Control::Down, Control::Left];
        Controls {
            gas: pad.strength(Control::Cross),
            brake: pad.strength(Control::Square),
            front_brake: pad.strength(Control::R2),
            rear_brake: F::ZERO,
            clutch: pad.strength(Control::L2),
            reverse: if pad.held(Control::R3) { F::ONE } else { F::ZERO },
            stick: (pad.axis(Axis::LeftX), pad.axis(Axis::LeftY)),
            dpad_y: pad.held(Control::Up) || pad.held(Control::Down),
            // The eight-way code the trick table reads: odd for a single direction, and the game's
            // own order round the d-pad. A diagonal is not derived, because which of two held
            // directions wins is the front end's business.
            dpad_dir: dpad.iter().position(|c| pad.held(*c)).map_or(0, |i| i as i32 * 2 + 1),
            stunt: pad.held(Control::Circle),
            balance: pad.held(Control::L3),
            respawn: pad.pressed(Control::Triangle),
        }
    }
}

/// The controls after the input layer's rate limit: what the bike is actually holding.
///
/// The original's input layer walks a table of bindings and moves each command towards what the pad
/// says at a rate the `.phy` gives (`[Joystick]`'s eight `…RateLimit` keys), so a stab at the throttle
/// is not a step. **The walk itself is this module's** — the binding table is the front end's and is
/// not ported — but the rates are the disc's, and a rate of zero is read as "no limit" rather than as
/// "never move", because that is what a `.phy` with the key missing has to mean.
#[derive(Clone, Copy, Debug, Default)]
pub struct Held {
    pub gas: F,
    pub brake: F,
    pub front_brake: F,
    pub rear_brake: F,
    pub clutch: F,
    pub stick: (F, F),
    /// How long the stick has been held past [`STICK_HELD`], which is what `air.rs` reads as
    /// `stick_hold`.
    pub stick_hold: F,
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
        self.stick.0 = toward(self.stick.0, c.stick.0, j.x_axis_rate_limit, dt);
        self.stick.1 = toward(self.stick.1, c.stick.1, j.y_axis_rate_limit, dt);
        let held = F(STICK_HELD) < (self.stick.0 * self.stick.0 + self.stick.1 * self.stick.1);
        self.stick_hold = if held { self.stick_hold + dt } else { F::ZERO };
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
                // The suspension line: straight down the chassis' own `y`, as far as the strut goes.
                probe: Vf::new(0.0, -travel.0, 0.0, 0.0),
                probe_length: travel,
                ground_normal: UP,
                max_compression: travel,
                max_damper_speed: t.max_compression_rate,
                extension_rate: t.max_decompression_rate,
                spring_rate: phy::spring_rate(t.spring, mass),
                // The damping ratios the file gives are of the spring's own rate.
                damping_out: phy::spring_rate(t.rebound_damping_ratio * t.spring, mass),
                damping_in: phy::spring_rate(t.bounce_damping_ratio * t.spring, mass),
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
        }
    }

    /// The ground step's view into the wheel module's, before the wheel module runs.
    fn to_wheel(&mut self) {
        self.w.contact = self.mv.contact;
        self.w.balancing = self.mv.balancing;
        self.w.steered = self.mv.steered;
        self.w.spring_rate = self.mv.spring;
        self.w.max_steer_angle = self.mv.max_steer_angle;
        self.w.min_steer_angle = -self.mv.max_steer_angle;
    }

    /// And back, after it has. Only the four words the ground step reads: the contact flag, the
    /// contact normal both roll angles are measured off, and the two flags
    /// `Wheel::ray_hit`/`Wheel::swept_contact` sit in — `+0xd0` and `+0xd4`, the first two of
    /// [`mv::Fresh`]'s seven. The other five and the three quadwords are the suspension's own
    /// workings, which `move.rs` empties and never reads.
    fn from_wheel(&mut self) {
        self.mv.contact = self.w.contact;
        self.mv.contact_normal = self.w.contact_normal;
        self.mv.fresh.scalars[0] = self.w.ray_hit as u32;
        self.mv.fresh.scalars[1] = self.w.swept_contact as u32;
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
    /// The bike's own scene node — three axis rows — which [`mv::no_hander`] rolls about its forward
    /// axis so the rider can let go of the bars.
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
                // The integrator's own limits. The original's come from the vehicle's setup, which
                // is not ported; a zero would pin the bike where it stands, because the clamp reads
                // a zero as "no faster than nothing". These are wide enough that neither can fire
                // on a bike — a stand-in, and listed as one.
                max_speed_sq: F(MAX_SPEED * MAX_SPEED),
                max_ang_speed_sq: F(MAX_ANG_SPEED * MAX_ANG_SPEED),
                ang_vel_max: [F(MAX_ANG_SPEED); 3],
                ang_vel_min: [F(-MAX_ANG_SPEED); 3],
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
        math::quat_to_mat4(self.chassis.quat, self.origin)
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
/// Three of the four are arithmetic the port can do — the transpose, a matrix row, a Gram-Schmidt —
/// and the fourth, the smooth noise, is a table the engine builds at start-up that this port does not
/// have. The two frames are the chassis' orientation, which is a copy taken at the top of the frame
/// and refreshed after every integration, and the bike's node, which is the array the ride carries.
struct Out<'a> {
    chassis: Mat4,
    node: &'a mut [Vf; 3],
}

impl Out<'_> {
    fn mat3(&self, frame: Frame) -> Mat3 {
        match frame {
            Frame::Chassis => Mat3::new(self.chassis.0[0], self.chassis.0[1], self.chassis.0[2]),
            Frame::Node => Mat3::new(self.node[0], self.node[1], self.node[2]),
        }
    }
}

impl mv::Outside for Out<'_> {
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
        *self.node = set_axes(forward, up);
    }
}

/// The transform helper that sets a frame's axes: its three rows from a forward and an up axis, by
/// Gram-Schmidt — row0 is `forward × up`, row1 the up and row2 the forward, which is the pair
/// `orientation_axes` reads back out.
///
/// **This arrangement of the cross products is the port's.** The original hands five more registers
/// to an orthonormalising callee that is not ported, so which of the three rows it
/// trusts and which it rebuilds is not proved; what is proved (S17) is the two axes it is handed and
/// in what order. Taking the forward as given and rebuilding the up is the arrangement that keeps the
/// bike pointing where the no-hander aimed it.
fn set_axes(forward: Vf, up: Vf) -> [Vf; 3] {
    let f = normalize(forward.merge(Dest::W, Vf::ZERO));
    let r = normalize(cross(f, up));
    let u = cross(r, f);
    [r, u, f]
}

/// The engine's shared one-dimensional smooth noise, which while the bike is
/// balancing is the rider's own unsteadiness.
///
/// **A stand-in.** The original reads tables built at start-up, which are not on the disc in a form
/// this port can read, so what is here is value noise with the same shape: `[-1, 1]`, continuous, and
/// a feature per unit of `t`. S17's proof of [`mv::balance`] hooks the real one on both sides, so the
/// controller around it is the original's and only the wobble's waveform is this.
fn noise(t: F) -> F {
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
fn integrate(ch: &mut chassis::Chassis, dt: F, origin: &mut Vf) -> Vec<chassis::Call> {
    let entry = math::quat_to_mat4(ch.quat, *origin);
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

/// A force into the chassis at a point, the way [`wheel::tire_forces`] puts one in: the force itself
/// in the world, and the moment it makes about the centre of mass in the body's own frame.
fn push(c: &mut wheel::Chassis, m: &Mat4, force: Vf, at: Vf) {
    let local_force = chassis::unrotate(&Mat3::new(m.0[0], m.0[1], m.0[2]), force);
    let arm = sub_xyz(unpoint(m, at), c.cofg);
    c.force = add_xyz(c.force, force);
    c.torque = add_xyz(c.torque, cross(arm, local_force));
}

/// The wheel's surface drag: what the surface under it takes out of it — the rolling
/// resistance against the patch's slide, and the brake's torque against the spin.
///
/// **A stand-in.** Every number in it is the disc's: `WheelDrag` and `MaxBrakingTorque` out of the
/// tire's own per-surface table, and the load out of the suspension. The *shape* is this module's —
/// the original's own version is named in `research/oracle.txt` and is not ported. Two things it is careful about: the
/// resistance is proportional to the slide only up to one unit a second, so a stopped wheel is not
/// shoved by it; and the brake cannot turn the wheel backwards inside one substep.
fn surface_drag(wh: &mut Wheel, pt: &Patch, surf: &phy::Surface, brake: F, dt: F,
                c: &mut wheel::Chassis, m: &Mat4) {
    if !wh.w.contact {
        return;
    }
    let drag = surf.wheel_drag * pt.load * clamp_unit(pt.roll);
    push(c, m, scale(pt.forward, -drag), wh.w.load_point);

    let spin = wh.mv.spin;
    let torque = brake * surf.max_braking_torque;
    let step = (torque * wh.inv_inertia * dt).min(spin.abs());
    wh.mv.spin = if spin < F::ZERO { spin + step } else { spin - step };
}

/// The wheel's spin integration: the wheel's own spin, and what the tire puts through its
/// contact patch.
///
/// **A stand-in**, and the largest one in this module. Its numbers are all the disc's — the
/// surface's `MaxDriveForce`, `MaxSideForce` and `WheelFriction`, the tire's `TireInertia` and
/// `TireRadius` — and the engine's torque reaches it through [`mv::Wheel::drive_torque`], which the
/// ported [`mv::engine_update`] filled in. What is this module's is the model:
///
/// - A wheel on the ground rolls: its spin is the patch's own speed over the radius. What it can put
///   down along itself is the drive torque turned into a force at the patch, capped by the surface's
///   drive limit and then by the grip the load allows; whatever the grip would not take spins the
///   wheel up instead, which is wheelspin.
/// - Across itself it answers the slide with the same grip, capped by the surface's side limit — less
///   whatever the donut command asked for, which is how the back end is let go.
/// - A wheel in the air just spins up under the engine's torque.
///
/// It also sets the two fields [`mv::engine_update`] reads back: the forward contact force, and
/// whether the spin is the torque's or the ground's.
fn spin_integrate(wh: &mut Wheel, pt: &Patch, surf: &phy::Surface, dt: F,
                  c: &mut wheel::Chassis, m: &Mat4) {
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
    let drive = (drive_torque * wh.mv.inv_radius).clamp(-surf.max_drive_force, surf.max_drive_force);
    let along = drive.clamp(-grip, grip);
    let cap = (surf.max_side_force * (F::ONE - wh.mv.donut_scale)).max(F::ZERO);
    let across = (-pt.side * grip).clamp(-cap, cap);
    push(c, m, add_xyz(scale(pt.forward, along), scale(pt.right, across)), wh.w.load_point);

    // Rolling, plus whatever the grip would not take: the spin the road asks for, and the slip the
    // tire's own inertia lets it run away with.
    let slip = ((drive - along) * wh.mv.radius) * wh.inv_inertia * dt;
    wh.mv.spin = pt.roll * wh.mv.inv_radius + slip;
    wh.mv.spin_from_torque = false;
    wh.mv.contact_force_fwd = along;
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
fn wheel_integrate(wh: &mut Wheel, tire: &phy::Tire, brake: F, mvc: &mut mv::Chassis, m: &Mat4,
                   curve: &CurveTable, dt: F) -> Vec<wheel::Call> {
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
    surface_drag(wh, &pt, &surf, brake, dt, &mut wc, m);
    spin_integrate(wh, &pt, &surf, dt, &mut wc, m);
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
            mv: ride, chassis, engine, air, front, rear, origin, node, held, near, touching, ..
        } = self;
        *touching = false;

        // 1 — the input layer's rate limit.
        held.step(c, &p.joystick, dt);

        // 2 — the chassis' measure, which is where the pitch and roll `air.rs` reads come from. The
        // euler angles are the one call it makes that this crate ports.
        let euler = math::quat_to_euler(chassis.quat, &tables.acos);
        chassis::measure(chassis, dt, euler);

        // The bike's node follows the body's heading but keeps its own roll, which is the no-hander's
        // one piece of state. **The port's own:** in the original the attachment walk puts the node
        // where the body is, and that walk runs after the ground step (`research/oracle.txt`, A5) and
        // is not ported.
        let body = math::quat_to_mat4(chassis.quat, *origin);
        *node = set_axes(body.0[2], if node[1].to_bits() == Vf::ZERO.to_bits() { body.0[1] } else { node[1] });

        let mut out = Out { chassis: body, node };

        // What the ground step takes as given, all of it the chassis' transform's work and all of it a
        // frame constant — the transform only moves when the body is integrated, and `mv::Step` is
        // what the original's own stack frame is.
        let rot = out.mat3(Frame::Chassis);
        let local_accel = chassis::unrotate(&rot, chassis.accel);
        let forward_speed = F(chassis.local_vel.z());
        let reverse_force = chassis::rotate(&rot, mv::reverse_push(&p.mv, forward_speed));
        let (stick_angle, deflection) = stick(held.stick);

        let step = mv::Step {
            dt,
            on_ground: air.on_ground,
            both_wheels: air.both_wheels,
            speed: forward_speed,
            stick_y: held.stick.1,
            stick_angle,
            deflection,
            lean: held.stick.0,
            gas: held.gas,
            brake: held.brake,
            clutch: held.clutch,
            gas_rate_limited: held.gas,
            reverse: c.reverse,
            balancing: c.balance,
            local_accel,
            reverse_force,
            node_distance: *near,
            stick_x: c.stick.0,
            front_brake: held.front_brake,
            rear_brake: held.rear_brake,
            lean_fore_aft: held.stick.1,
        };

        // The ground step's view of the chassis: the same memory the original's is, so it lives
        // across the whole substep loop and is synced to the body only where the body is integrated.
        let mut mvc = mv::Chassis {
            speed: chassis.speed,
            force: chassis.force,
            accel: chassis.accel,
            ang_accel: chassis.ang_accel,
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
            stick: held.stick,
            stick_hold: held.stick_hold,
            dpad_y: c.dpad_y,
            dpad_dir: c.dpad_dir,
            brake: held.brake,
            front_brake: held.front_brake,
            rear_brake: held.rear_brake,
            lean: held.stick.0,
            stunt: c.stunt,
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
        rear.mv.donut_scale = mv::donut_roll(mvp, &mvc, s);
        front.mv.spring = mv::spring(mvc.speed, mvp.front_spring_when_stopped, mvp.front_spring);
        rear.mv.spring = mv::spring(mvc.speed, mvp.rear_spring_when_stopped, mvp.rear_spring);
        ride.g_peak = mv::NOTHING;
        mvc.accel = mv::NOTHING;
        mvc.ang_accel = mv::NOTHING;
        if s.speed < F(mv::IDLE_SPEED) && s.gas_rate_limited == F::ZERO {
            ride.clutch_in = F::ONE;
        }
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

            calls.push(mv::Call::WheelIntegrate(Side::Front, h));
            let brake = held.front_brake;
            wheel_integrate(front, &p.front, brake, &mut mvc, &out.chassis, &tables.curve, h);
            calls.push(mv::Call::WheelIntegrate(Side::Rear, h));
            let brake = held.rear_brake;
            wheel_integrate(rear, &p.rear, brake, &mut mvc, &out.chassis, &tables.curve, h);

            mv::engine_post(engine, &mut rear.mv);

            // The collision record is opened per substep in the original and kept across the frame's
            // meshes; here one is made where it is swept, so there is nothing to open.
            calls.push(mv::Call::CollisionBegin(k == 0));
            calls.push(mv::Call::Unrotate(mvc.accel));
            ride.g_peak = mv::g_peak(ride.g_peak, s.local_accel);

            calls.push(mv::Call::Integrate(h));
            chassis.force = mvc.force;
            chassis.torque = mvc.torque;
            chassis.accel = mvc.accel;
            chassis.ang_accel = mvc.ang_accel;
            chassis.cofg = mvc.cofg;
            chassis.vel = mvc.vel;
            chassis.ang_vel = mvc.ang_vel;
            chassis::set_flags(chassis, mvc.flags.0, mvc.flags.1, mvc.flags.2);
            integrate(chassis, h, origin);
            out.chassis = math::quat_to_mat4(chassis.quat, *origin);
            mvc.force = chassis.force;
            mvc.torque = chassis.torque;
            mvc.accel = chassis.accel;
            mvc.ang_accel = chassis.ang_accel;
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
            w.f("stick_hold", self.held.stick_hold);
        });
        w.vf("spawn", self.spawn);
        w.vf("spawn_facing", self.spawn_facing);
        w.f("near", self.near);
        w.bool("touching", self.touching);
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
