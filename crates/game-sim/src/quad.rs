//! **The ATV's own ground steps** (MD37b), ported from MX vs. ATV Unleashed's PS2 ELF (SLUS-21104),
//! the only game that has the quad: MX Unleashed has no ATV, so every proof here is against
//! SLUS-21104's code (`tests/atv_quad.rs`), never MX Unleashed's.
//!
//! MX vs. ATV's quad is its bike with four wheels: the same `.phy` reader, engine, air, trick and wreck
//! steps, run per substep by the physics world. What the quad replaces in the bike's ground substep
//! is here:
//!
//! - [`stand_upright`] in place of the bike's stand-upright and balance: the four contact normals'
//!   mean, and no balance wobble at all.
//! - [`roll_pitch`]: the wheelie and stoppie controllers, read off a rear and a front wheel of the
//!   pair, and turning both front wheels' steering off while either holds the quad up.
//! - [`ground_pitch_torque`]: the bike's own push under the quad's guard.
//! - [`rear_pair_flag`]: the flag the tire forces leave on both rear wheels when a front wheel is down
//!   and neither rear is.
//!
//! Around them, the quad's three physics-world listeners (MD64):
//!
//! - [`ground_head`], the PreProcess: the steering, the donut and the springs, each written to both
//!   wheels of the pair.
//! - [`ground_step`], the Force body: the order the pieces above and the bike's own steps run in, on
//!   four wheels, with [`wheel_contacts`] spreading the frame's contacts onto the four.
//! - [`air_state`], the PostProcess: the bike's contact step on four wheels, without its no-hander.
//!
//! What they call of SLUS-21104's bike steps (engine, preload, backflip, air drag, airborne control,
//! the wheels' suspension, tire forces and integration, the input limiter, the gearbox and clutch) is
//! not ported here: each is a [`Call`] handed to a [`Moto`], in the original's order. The update tail,
//! the tire `.phy` reader, the wreck, put-back and net unpack are filed (MD64a, MD64b, MD65).
//!
//! The quad is described by what the steps read, [`Quad`] and [`Body`], not by its layout: the field
//! map is research/oracle.txt's (`# MD37`, `# MD37a`, `# MD37b`).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::math::quat_axes;
use ps2_float::vu::{Dest, Field};
use ps2_float::{F, Vf, fpu, fpu::Acc};

/// The four wheels, in the quad's order: the steered pair, then the driven pair.
pub const LEFT_FRONT: usize = 0;
pub const RIGHT_FRONT: usize = 1;
pub const LEFT_REAR: usize = 2;
pub const RIGHT_REAR: usize = 3;

/// Below this speed (ft/s, either way) the quad is held from sliding sideways down a slope.
const STAND_UPRIGHT_SPEED: f32 = 10.0;
/// The wheelie needs more than this speed and the stick at least this far back.
const WHEELIE_SPEED: f32 = 5.0;
const WHEELIE_LEAN: f32 = 0.5;
/// The stoppie starts above this speed with the stick fully forward, and ends at or under the end
/// speed or with the stick back past the end lean.
const STOPPIE_SPEED: f32 = 15.0;
const STOPPIE_LEAN: f32 = -1.0;
const STOPPIE_END_LEAN: f32 = -0.1;
const STOPPIE_END_SPEED: f32 = 7.0;
/// The pitch integrator's rate, and the stoppie's yaw gain: both 500 on the quad (the bike's stoppie
/// yaw gain is 1000).
const PITCH_GAIN_RATE: f32 = 500.0;

/// The rodata quadword a zero-length direction is replaced with.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// One wheel, as the quad's own steps see it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    /// The wheel takes the steering (cleared on both fronts while a wheelie or stoppie holds the quad).
    pub steered: bool,
    /// The rear-pair flag [`rear_pair_flag`] sets.
    pub rear_pair: bool,
    /// The wheel is on the ground.
    pub touching: bool,
    /// The ground's normal under it. `w` comes along: the sums keep the first wheel's.
    pub normal: Vf,
    /// The velocity at its contact patch, in the quad's frame (the PostProcess reads it).
    pub contact_vel: Vf,
    /// The fronts: held up for the balance, the steering lock and the steering command.
    pub balancing: bool,
    pub max_steer_angle: F,
    pub steer_command: F,
    /// The rears: how much donut the stick asks for.
    pub donut: F,
    /// The spring rate for this speed, the pair's own stopped/moving map.
    pub spring: F,
    /// One over the radius: the left rear's is what the automatic gearbox reads.
    pub inv_radius: F,
}

/// What the quad's steps read and keep of the quad itself.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quad {
    /// On the ground at all.
    pub on_ground: bool,
    /// The flags the wheel contacts leave: the whole quad down, a front down, a rear down.
    pub all_down: bool,
    pub front_down: bool,
    pub rear_down: bool,
    /// The bike's balance runs when its two balance words agree; the quad has no balance, and its
    /// stand-upright runs only when they do not.
    pub balancing: bool,
    /// The velocity at the contact patch, in the quad's frame: the first touching wheel's, else the
    /// body's own. Its `z` is the signed speed along the quad ([`Quad::speed`]).
    pub contact_vel: Vf,
    /// The brake (Square) and the throttle (Cross), and the throttle after its rate limit.
    pub brake: F,
    pub gas: F,
    pub gas_limited: F,
    /// The chassis' euler angles, pitch first ([`Quad::pitch`]).
    pub euler: Vf,
    /// The chassis' angular velocity in the world.
    pub world_ang_vel: Vf,
    /// The front and rear brakes and the clutch lever as the PreProcess split them.
    pub front_brake: F,
    pub rear_brake: F,
    pub clutch_in: F,
    /// The stick, sideways and fore and aft (forward is negative).
    pub lean: F,
    pub lean_fore_aft: F,
    /// The pitch controllers' integral, and whether a stoppie is on.
    pub pitch_integral: F,
    pub stoppie: bool,
    /// The ground push's gate, target speed and gain (the bike's `[Mass Properties]` pitch-torque keys).
    pub push: bool,
    pub push_target: F,
    pub push_gain: F,
    /// What gates the ground push on the quad.
    pub push_gate: F,
    /// The physics world's gravity.
    pub gravity: Vf,
    pub wheels: [Wheel; 4],

    /// The steps' shared guard: the quad has a vehicle state, and it is not one of the two (3 and 4)
    /// in which the PreProcess, Force and PostProcess all return at once.
    pub active: bool,
    /// Set, the Force body skips the quad's own controllers and the bike's (it still runs the four
    /// wheels), and the PreProcess zeroes the steering instead of commanding it.
    pub control_off: bool,
    /// The body's speed, the length of its velocity (the signed one is [`Quad::speed`]).
    pub speed_len: F,
    /// The steering command the PreProcess leaves for the bike's steps, the same as both fronts get.
    pub steer_command: F,
    /// How long the stick has been held at full lock, for [`Params::full_lock`].
    pub full_lock_timer: F,
    /// The stick's angle and how far it is deflected (the donut reads them).
    pub stick_angle: F,
    pub deflection: F,
    /// The peak-rates vector the Force body's last call holds maxima in, reset every PreProcess.
    pub attitude: Vf,

    /// The PostProcess's: the body's velocity in the quad's frame; the quad's forward and up axes
    /// (only `xyz` written); whether it was and is in the air, and for how long; the crossup angle.
    pub local_vel: Vf,
    pub heading: Vf,
    pub up: Vf,
    pub was_airborne: bool,
    pub airborne: bool,
    pub hang_time: F,
    pub crossup: F,
    /// The backflip's and the 360's "rotating" flags: either stops the crossup being measured.
    pub flipping: bool,
    pub spinning: bool,
    /// Control authority after a flip, and the time it ramps back over: the crossup ramps with it.
    pub authority: F,
    pub authority_time: F,
    /// The collision record's flag the PostProcess sets when the quad's up axis is at or below a
    /// quarter (on its side or over).
    pub tipped: bool,
    /// Where the quad's scene node is, as the node asked it, and the copy kept beside it.
    pub node_pos: Vf,
    pub node_pos_kept: Vf,
}

/// The quad's wheelie and stoppie keys (the bike's, from the same `.phy` sections).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    pub max_pitch_command: F,
    pub pitch_gain: F,
    pub pitch_rate_gain: F,
    pub inv_wheelie_top_speed: F,
    pub max_stoppie_pitch_command: F,
    pub stoppie_pitch_gain: F,
    pub stoppie_yaw_rate_scale: F,
    pub stoppie_top_speed: F,
    /// The steering's speed fade: `steer_scale_1` up to `steer_speed_1`, `steer_scale_2` from
    /// `steer_speed_2` on, read against [`Quad::speed`].
    pub steer_speed_1: F,
    pub steer_scale_1: F,
    pub steer_speed_2: F,
    pub steer_scale_2: F,
    /// The full-lock option: with the stick held hard over, `full_lock_angle` of the lock ramps in over
    /// `full_lock_time` and the speed fade only scales the rest.
    pub full_lock: bool,
    pub full_lock_angle: F,
    pub full_lock_time: F,
    /// The donut: under this speed, the two stick angles its ramps end at.
    pub max_donut_speed: F,
    pub donut_angle: F,
    pub donut_angle_back: F,
    /// Each pair's spring rate stopped and moving.
    pub front_spring: (F, F),
    pub rear_spring: (F, F),
}

/// The quad's rigid body: what the steps push on and read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// The force and moment accumulators. Only `xyz` is ever added to.
    pub force: Vf,
    pub moment: Vf,
    /// The orientation quaternion the body's world/local rotations use.
    pub orientation: Vf,
    /// The angular velocity in the body's own frame (the controllers read `x`, the pitch rate).
    pub ang_vel: Vf,
    /// The velocity in the world.
    pub velocity: Vf,
    /// A quadword of the body's the PreProcess resets to (0, 0, 0, 1) every frame; what reads it is
    /// not traced.
    pub reset_each_frame: Vf,
}

fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::splat(s.0).bc(Field::X)))
}

fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `a·b` over `xyz`, on the FPU's accumulator: `mula`, `madda`, `madd`.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

impl Body {
    /// The body's add-force: `xyz` into the force accumulator.
    pub fn add_force(&mut self, f: Vf) {
        self.force = add_xyz(self.force, f);
    }

    /// The body's add-moment: `xyz` into the moment accumulator.
    pub fn add_moment(&mut self, m: Vf) {
        self.moment = add_xyz(self.moment, m);
    }

    /// A world vector into the body's frame by its orientation `q`, as the body's world-moment call
    /// does it on VU0 before adding the moment: `q* v q` in two `vopmula`/`vmadd*`/`vopmsub` rounds,
    /// one accumulator each. `w` is `v`'s.
    pub fn to_local(&self, v: Vf) -> Vf {
        let q = self.orientation;
        let acc = Vf::madd(Vf::opmula(v, q), v, q.bc(Field::W));
        let t = Vf::opmsub(acc, q, v);
        let qv = q.mul(v);
        let acc = Vf::opmula(t, q);
        let acc = Vf::madd(acc, q, qv.bc(Field::X));
        let acc = Vf::madd(acc, q, qv.bc(Field::Y));
        let acc = Vf::madd(acc, q, qv.bc(Field::Z));
        let acc = Vf::madd(acc, t, q.bc(Field::W));
        v.merge(Dest::XYZ, Vf::opmsub(acc, q, t))
    }

    /// A body-frame vector into the world, the same two rounds with the operands the other way about
    /// (the body's local-force call). `w` is `v`'s.
    pub fn to_world(&self, v: Vf) -> Vf {
        let q = self.orientation;
        let acc = Vf::madd(Vf::opmula(q, v), v, q.bc(Field::W));
        let t = Vf::opmsub(acc, v, q);
        let qv = q.mul(v);
        let acc = Vf::opmula(q, t);
        let acc = Vf::madd(acc, q, qv.bc(Field::X));
        let acc = Vf::madd(acc, q, qv.bc(Field::Y));
        let acc = Vf::madd(acc, q, qv.bc(Field::Z));
        let acc = Vf::madd(acc, t, q.bc(Field::W));
        v.merge(Dest::XYZ, Vf::opmsub(acc, t, q))
    }

    /// A world moment, brought into the body's frame and added.
    pub fn add_world_moment(&mut self, m: Vf) {
        let local = self.to_local(m);
        self.add_moment(local);
    }

    /// A body-frame force, taken into the world and added.
    pub fn add_local_force(&mut self, f: Vf) {
        let world = self.to_world(f);
        self.add_force(world);
    }

    /// The body's forward axis (its rotation's `z` row), `w` set to 1 as the stand-upright does.
    fn forward(&self) -> Vf {
        let (f, _) = quat_axes(self.orientation);
        Vf::new(f[0].0, f[1].0, f[2].0, 1.0)
    }
}

/// **The quad's stand-upright**: a quad rolling almost to a stop on a slope does not slide sideways
/// down it, and with the brake on does not slide at all.
///
/// The bike's does this off its front wheel's ground normal; the quad takes the mean of all four
/// wheels' normals (summed `((LF + RF) + LR) + RR` on VU0, times a quarter), touching or not. It runs
/// with the whole quad down, not on the balance words' agreement, off the throttle and below 10 ft/s
/// either way. The slide is gravity less its part into that ground; its part across the quad's forward
/// (flattened into the ground, renormalised, or [`NOTHING`] if that has no length) is cancelled by a
/// force, and its part along the quad is cancelled too when the brake is on.
///
/// The across force is scaled by the original's map of the speed from `0..10` onto `1..1` — always one
/// — negated.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn stand_upright(q: &Quad, body: &mut Body) {
    if !q.all_down || !q.balancing_differs() || q.gas != F::ZERO || !(q.speed().abs() < F(STAND_UPRIGHT_SPEED)) {
        return;
    }
    let w = &q.wheels;
    let sum = add_xyz(add_xyz(add_xyz(w[LEFT_FRONT].normal, w[RIGHT_FRONT].normal), w[LEFT_REAR].normal), w[RIGHT_REAR].normal);
    let n = scale(sum, F(0.25));
    let g = q.gravity;
    let slide = sub_xyz(g, scale(n, dot3(n, g)));

    let fwd = body.forward();
    let fwd = sub_xyz(fwd, scale(n, dot3(fwd, n)));
    let len2 = dot3(fwd, fwd);
    let fwd = if len2 == F::ZERO {
        NOTHING
    } else {
        let k = F::ONE.rsqrt(len2);
        Vf::new((F(fwd.x()) * k).0, (F(fwd.y()) * k).0, (F(fwd.z()) * k).0, fwd.w())
    };

    let along = scale(fwd, dot3(fwd, slide));
    let across = sub_xyz(slide, along);
    let mut force = scale(across, F(fpu::neg(unit_map(q.speed()).0)));
    if q.brake != F::ZERO {
        force = sub_xyz(force, along);
    }
    body.add_force(force);
}

/// The original's `map_clamp(speed, 0, 10, 1, 1)` as it compiled it: a guard on `0 == 10` that never
/// fires, the divide, then `1 + t·(1 − 1)` through the accumulator, clamped to `[1, 1]` with `max.s`
/// and `min.s`.
fn unit_map(speed: F) -> F {
    let (lo, hi, one) = (F::ZERO, F(STAND_UPRIGHT_SPEED), F::ONE);
    let t = (speed - lo) / (hi - lo);
    let v = Acc::adda(F::ZERO, one).madd(t, one - one);
    v.max(one.min(one)).min(one.max(one))
}

impl Quad {
    /// The signed speed along the quad, ft/s: the contact velocity's `z`.
    pub fn speed(&self) -> F {
        F(self.contact_vel.z())
    }

    /// The chassis' euler pitch.
    pub fn pitch(&self) -> F {
        F(self.euler.x())
    }

    fn balancing_differs(&self) -> bool {
        !self.balancing
    }

    /// The front wheel the stoppie reads (the left if it touches, else the right), and the rear wheel
    /// the wheelie reads, the same way.
    fn front_normal(&self) -> Vf {
        if self.wheels[LEFT_FRONT].touching { self.wheels[LEFT_FRONT].normal } else { self.wheels[RIGHT_FRONT].normal }
    }
    fn rear_normal(&self) -> Vf {
        if self.wheels[LEFT_REAR].touching { self.wheels[LEFT_REAR].normal } else { self.wheels[RIGHT_REAR].normal }
    }

    fn steer_fronts(&mut self, on: bool) {
        self.wheels[LEFT_FRONT].steered = on;
        self.wheels[RIGHT_FRONT].steered = on;
    }
}

/// **The quad's wheelie and stoppie** (its roll/pitch step): pitch the quad to where the stick asks
/// while it is up on one pair of wheels.
///
/// Off the ground nothing runs and the stoppie ends.
///
/// - **Wheelie**: a rear down, on the (rate-limited) throttle, above 5 ft/s, no rear brake, no clutch,
///   the stick at least half back. The target pitch is `(2·lean − 1)·MaxPitchCommand`; the error
///   `gain·(pitch − target) − pitch rate` integrates at 500/s, and the moment `(integral +
///   rate gain·error)` is faded out towards the top speed (`1 − speed/top`, clamped to `[0, 1]`) and
///   pushed about the quad's own `x`. A yaw term about a rear wheel's normal is built and scaled by a
///   literal zero (the bike's stoppie has the same term live), and still goes through the body's
///   world-moment call. Both fronts stop steering; with no wheelie they steer and the integral clears.
/// - **Stoppie**: latched with a front down, the front brake on, above 15 ft/s and the stick fully
///   forward. The target is the bike's, faded in by speed over `0..StoppieTopSpeed` (zero when that is
///   zero); the same integral and moment, then a yaw moment `500·(StoppieYawRateScale·lean − ω·n)`
///   about a front wheel's normal `n`, in the world. Both fronts stop steering. It unlatches with a
///   rear down, the front brake off, the stick back past −0.1 or at 7 ft/s or less. With no stoppie
///   on, the fronts steer and the integral clears.
///
/// The original keeps the integral through the FPU's accumulator: `0 + integral`, then `+ dt·(500·e)`.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn roll_pitch(p: &Params, q: &mut Quad, body: &mut Body, dt: F) {
    if !q.on_ground {
        q.stoppie = false;
        return;
    }
    let lean = q.lean_fore_aft;

    let wheelie = q.rear_down
        && !(q.gas_limited <= F::ZERO)
        && !(q.speed() <= F(WHEELIE_SPEED))
        && q.rear_brake == F::ZERO
        && q.clutch_in == F::ZERO
        && !(lean < F(WHEELIE_LEAN));
    if wheelie {
        let target = F(2.0) * lean * p.max_pitch_command - p.max_pitch_command;
        let err = p.pitch_gain * (q.pitch() - target) - F(body.ang_vel.x());
        q.pitch_integral = Acc::adda(F::ZERO, q.pitch_integral).madd(dt, F(PITCH_GAIN_RATE) * err);
        let fade = (F::ONE - q.speed() * p.inv_wheelie_top_speed).min(F::ONE).max(F::ZERO);
        let tx = fade * Acc::adda(F::ZERO, q.pitch_integral).madd(p.pitch_rate_gain, err);
        body.add_moment(Vf::new(tx.0, 0.0, 0.0, 0.0));
        // The yaw rate about the rear normal is measured and thrown away: the scale is a literal zero.
        let n = q.rear_normal();
        let _dead = dot3(q.world_ang_vel, n);
        body.add_world_moment(scale(n, F::ZERO));
        q.steer_fronts(false);
    } else {
        q.steer_fronts(true);
        q.pitch_integral = F::ZERO;
    }

    if q.front_down
        && !q.stoppie
        && !(q.front_brake <= F::ZERO)
        && !(q.speed() <= F(STOPPIE_SPEED))
        && lean <= F(STOPPIE_LEAN)
    {
        q.stoppie = true;
    }
    if !q.stoppie {
        q.steer_fronts(true);
        q.pitch_integral = F::ZERO;
        return;
    }

    let fade = if F::ZERO == p.stoppie_top_speed {
        F::ZERO
    } else {
        let t = (q.speed() - F::ZERO) / (p.stoppie_top_speed - F::ZERO);
        let v = Acc::adda(F::ZERO, F::ZERO).madd(t, F::ONE - F::ZERO);
        v.max(F::ZERO.min(F::ONE)).min(F::ZERO.max(F::ONE))
    };
    let target = fade * Acc::adda(F::ZERO, p.max_stoppie_pitch_command).madd(F(2.0) * lean, p.max_stoppie_pitch_command);
    let err = p.stoppie_pitch_gain * (q.pitch() - target) - F(body.ang_vel.x());
    q.pitch_integral = Acc::adda(F::ZERO, q.pitch_integral).madd(dt, F(PITCH_GAIN_RATE) * err);
    let tx = Acc::adda(F::ZERO, q.pitch_integral).madd(p.pitch_rate_gain, err);
    body.add_moment(Vf::new(tx.0, 0.0, 0.0, 0.0));

    let want = p.stoppie_yaw_rate_scale * q.lean;
    let n = q.front_normal();
    let yaw = F(PITCH_GAIN_RATE) * (want - dot3(q.world_ang_vel, n));
    body.add_world_moment(scale(n, yaw));
    q.steer_fronts(false);

    let end = q.rear_down || q.front_brake == F::ZERO || !(lean <= F(STOPPIE_END_LEAN)) || q.speed() <= F(STOPPIE_END_SPEED);
    if end {
        q.stoppie = false;
    }
}

/// **The quad's ground push**: with a rear down and its gate open, the bike's own push runs — with its
/// flag set, a force `gain·(target − speed)` along the quad's own `z`, taken into the world. On
/// the bike it is the same push under the bike's guard.
pub fn ground_pitch_torque(q: &Quad, body: &mut Body) {
    if !q.rear_down || q.push_gate == F::ZERO {
        return;
    }
    if !q.push {
        return;
    }
    let z = q.push_gain * (q.push_target - q.speed());
    body.add_local_force(NOTHING.merge(Dest::Z, Vf::new(0.0, 0.0, z.0, 0.0)));
}

/// **The rear-pair flag**, set on both rear wheels after the tire forces: a front down (either) and
/// both rears up — the quad on its front wheels. Cleared on both otherwise.
pub fn rear_pair_flag(q: &mut Quad) {
    let w = &q.wheels;
    let on = (w[LEFT_FRONT].touching || w[RIGHT_FRONT].touching) && !w[LEFT_REAR].touching && !w[RIGHT_REAR].touching;
    q.wheels[LEFT_REAR].rear_pair = on;
    q.wheels[RIGHT_REAR].rear_pair = on;
}

// --- the three listeners: PreProcess, Force, PostProcess --------------------------------------------

/// The stick counts as held at full lock from here (either way).
const FULL_LOCK: f32 = 0.987;
/// The donut needs the stick deflected past this.
const DONUT_DEFLECTION: f32 = 0.9;
/// The springs' speed map, stopped to moving (ft/s).
const SPRING_STOPPED: f32 = 14.6666;
const SPRING_MOVING: f32 = 29.3332;
const HALF_PI: f32 = 1.5707964;
const PI: f32 = 3.1415927;
/// The crossup is only measured above this speed.
const CROSSUP_SPEED: f32 = 10.0;
/// At or below this, the up axis's `y` says the quad is on its side.
const TIPPED_UP: f32 = 0.25;

/// What the quad's three listeners call of SLUS-21104's bike and vehicle steps, and this module does
/// not port, in the order the original calls them. `F`s are the `dt` handed over; `usize`s the wheel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// The PreProcess's: the bike's input limiter (first), its automatic gearbox (handed the left
    /// rear's one-over-radius), its clutch, and the virtual it ends on.
    InputLimiter,
    AutoShift { dt: F, inv_radius: F },
    Clutch(F),
    HeadTail,
    /// The Force body's: the vehicle's gravity into the body (first, always), then — with control on —
    /// the bike's preload, backflip, air spin, the engine, the air drag and the airborne control.
    Gravity,
    Preload(F),
    Backflip(F),
    SpinAir(F),
    EngineUpdate(F),
    AirDrag(F),
    AirborneControl(F),
    /// The four wheels, in the Force body's rounds: each cleared, the contacts spread
    /// ([`wheel_contacts`]), each suspension, each tire's forces, then (after [`rear_pair_flag`]) each
    /// integrated.
    WheelClear(usize),
    Contacts,
    Suspension(usize, F),
    TireForces(usize, F),
    WheelIntegrate(usize, F),
    /// The engine's rev limiter and the peak-rates hold, the Force body's last two.
    RevLimit,
    PeakRates,
    /// The PostProcess's: the vehicle's own post step, then the scene node asked where it is (into
    /// [`Quad::node_pos`]).
    VehiclePost(F),
    NodePosition,
}

/// Whoever runs the calls the quad's listeners make. A `Vec<Call>` just lists them.
pub trait Moto {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body);
}

impl Moto for Vec<Call> {
    fn call(&mut self, c: Call, _q: &mut Quad, _body: &mut Body) {
        self.push(c);
    }
}

/// The original's inlined map and clamp: `x` from `a..b` onto `out0..out1`, clamped between the two
/// outputs whichever way round they are. Callers test `a == b` themselves.
fn map_clamp(x: F, a: F, b: F, out0: F, out1: F) -> F {
    crate::air::map_clamp(x, a, b, out0, out1)
}

/// **The quad's PreProcess**: the bike's ground head with every wheel write made to both wheels of
/// the pair.
///
/// The input limiter first; off the ground the front brake is let go. With the balance words equal
/// both fronts are held up; otherwise they are not, and unless control is off they get the steering
/// command — the stick times the steering lock times the speed fade (with the full-lock option, the
/// full-lock angle ramped in over its time while the stick is hard over, and the fade on the rest of
/// the lock only); with control off both fronts' commands are zeroed.
///
/// The donut goes on both rears: all four down, under the donut speed, the stick deflected past 0.9
/// and the throttle open, then the bike's two ramps on the stick angle either side of −π/2. Each pair
/// gets its spring rate off the speed (14.6666 to 29.3332 ft/s), the peak-rates vector and a body
/// quadword are reset, and the gearbox, clutch and the ending virtual are called.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn ground_head(p: &Params, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    if !q.active {
        return;
    }
    m.call(Call::InputLimiter, q, body);
    if !q.on_ground {
        q.front_brake = F::ZERO;
    }
    if q.balancing {
        q.wheels[LEFT_FRONT].balancing = true;
        q.wheels[RIGHT_FRONT].balancing = true;
    } else {
        q.wheels[LEFT_FRONT].balancing = false;
        q.wheels[RIGHT_FRONT].balancing = false;
        if q.control_off {
            q.wheels[LEFT_FRONT].steer_command = F::ZERO;
            q.wheels[RIGHT_FRONT].steer_command = F::ZERO;
        } else {
            let fade = if p.steer_speed_1 == p.steer_speed_2 {
                p.steer_scale_1
            } else {
                map_clamp(q.speed(), p.steer_speed_1, p.steer_speed_2, p.steer_scale_1, p.steer_scale_2)
            };
            let lock = q.wheels[LEFT_FRONT].max_steer_angle;
            let command = if !p.full_lock {
                lock * (q.lean * fade)
            } else {
                q.full_lock_timer = if q.lean.abs() < F(FULL_LOCK) { F::ZERO } else { q.full_lock_timer + dt };
                let ramp = F::ONE.min(q.full_lock_timer / p.full_lock_time);
                Acc::mula(q.lean, p.full_lock_angle * ramp).madd(q.lean * fade, lock - p.full_lock_angle)
            };
            q.wheels[RIGHT_FRONT].steer_command = command;
            q.wheels[LEFT_FRONT].steer_command = command;
            q.steer_command = command;
        }
    }

    let donut = if !q.all_down
        || !(q.speed_len < p.max_donut_speed)
        || q.deflection <= F(DONUT_DEFLECTION)
        || q.gas == F::ZERO
    {
        F::ZERO
    } else {
        donut(p, q.stick_angle)
    };
    q.wheels[LEFT_REAR].donut = donut;
    q.wheels[RIGHT_REAR].donut = donut;

    let (stopped, moving) = (F(SPRING_STOPPED), F(SPRING_MOVING));
    let front = map_clamp(q.speed_len, stopped, moving, p.front_spring.0, p.front_spring.1);
    let rear = map_clamp(q.speed_len, stopped, moving, p.rear_spring.0, p.rear_spring.1);
    q.wheels[LEFT_FRONT].spring = front;
    q.wheels[LEFT_REAR].spring = rear;
    q.wheels[RIGHT_FRONT].spring = front;
    q.wheels[RIGHT_REAR].spring = rear;

    q.attitude = NOTHING;
    body.reset_each_frame = NOTHING;
    let inv_radius = q.wheels[LEFT_REAR].inv_radius;
    m.call(Call::AutoShift { dt, inv_radius }, q, body);
    m.call(Call::Clutch(dt), q, body);
    m.call(Call::HeadTail, q, body);
}

/// The donut's two ramps: nothing at straight ahead up to all of it at the donut angle, all of it
/// to −π/2, and back down past the back angle to nothing at −π. An empty ramp gives the end it
/// would have reached.
fn donut(p: &Params, a: F) -> F {
    let (half, pi) = (F(-HALF_PI), F(-PI));
    if a < half {
        let d = p.donut_angle_back;
        if a < d {
            if d == pi { F::ONE } else { map_clamp(a, d, pi, F::ONE, F::ZERO) }
        } else if half == d {
            F::ZERO
        } else {
            map_clamp(a, half, d, F::ZERO, F::ONE)
        }
    } else {
        let d = p.donut_angle;
        if a < d {
            if d == half { F::ONE } else { map_clamp(a, d, half, F::ONE, F::ZERO) }
        } else if F::ZERO == d {
            F::ZERO
        } else {
            map_clamp(a, F::ZERO, d, F::ZERO, F::ONE)
        }
    }
}

/// **The quad's Force body**: the bike's ground substep on four wheels.
///
/// Gravity first. With control on: [`stand_upright`] and [`roll_pitch`] in place of the bike's
/// stand-upright, balance and roll/pitch, then the bike's preload, backflip, air spin, engine, air
/// drag and airborne control, then [`ground_pitch_torque`]. Then, control or not: the four wheels
/// cleared, the contacts spread ([`wheel_contacts`]), the four suspensions, the four tire forces,
/// [`rear_pair_flag`], the four integrations, the rev limiter and the peak rates. Each round goes left
/// front, right front, left rear, right rear.
pub fn ground_step(p: &Params, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    if !q.active {
        return;
    }
    m.call(Call::Gravity, q, body);
    if !q.control_off {
        stand_upright(q, body);
        roll_pitch(p, q, body, dt);
        for c in [Call::Preload(dt), Call::Backflip(dt), Call::SpinAir(dt), Call::EngineUpdate(dt), Call::AirDrag(dt), Call::AirborneControl(dt)] {
            m.call(c, q, body);
        }
        ground_pitch_torque(q, body);
    }
    for w in 0..4 {
        m.call(Call::WheelClear(w), q, body);
    }
    m.call(Call::Contacts, q, body);
    for w in 0..4 {
        m.call(Call::Suspension(w, dt), q, body);
    }
    for w in 0..4 {
        m.call(Call::TireForces(w, dt), q, body);
    }
    rear_pair_flag(q);
    for w in 0..4 {
        m.call(Call::WheelIntegrate(w, dt), q, body);
    }
    m.call(Call::RevLimit, q, body);
    m.call(Call::PeakRates, q, body);
}

/// One contact of the frame's collision record, as the quad's distributor reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// Where the contact is along and across the quad: `z` above zero is a front wheel's, `x` above
    /// zero a right wheel's.
    pub at: Vf,
    pub normal: Vf,
    pub point: Vf,
    /// The collision node hit; the body it belongs to (zero for the world) and that body's
    /// touch-listener count; the vehicle it belongs to and that vehicle's kind (only kind 2 counts).
    pub node: u32,
    pub owner: u32,
    pub listeners: i32,
    pub vehicle: u32,
    pub vehicle_kind: i32,
}

/// **The quad's contact distributor**, the bike's on four wheels: the record cleared and swept;
/// then, if the sweep found anything to look at, each contact's owner's listeners told it was
/// touched, and the contact set on the wheel its place picks — `z > 0` a front, else a rear; `x > 0`
/// the right, else the left. The last rear contact's node is kept (`rear_node`; zero when none).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn wheel_contacts(
    wheels: &mut [crate::chassis::WheelContact; 4],
    rear_node: &mut u32,
    look: bool,
    cs: &[Contact],
) -> Vec<crate::chassis::ContactCall> {
    use crate::chassis::{ContactCall, set_contact};
    *rear_node = 0;
    let mut calls = vec![ContactCall::ClearRecord, ContactCall::Collide];
    if !look {
        return calls;
    }
    for c in cs {
        if c.owner != 0 {
            for i in 0..c.listeners {
                calls.push(ContactCall::Notify(c.owner, i));
            }
        }
        let vehicle = if c.vehicle != 0 && c.vehicle_kind == 2 { c.vehicle } else { 0 };
        // `c.ole` against zero: anything not at or below it, NaN too, is the right or the front.
        let side = usize::from(!(F(c.at.x()) <= F::ZERO));
        let wheel = if !(F(c.at.z()) <= F::ZERO) { LEFT_FRONT + side } else { LEFT_REAR + side };
        let at = crate::chassis::Contact { normal: c.normal, point: c.point, owner: c.owner, ..Default::default() };
        set_contact(&mut wheels[wheel], &at, vehicle);
        if wheel >= LEFT_REAR {
            *rear_node = c.node;
        }
    }
    calls
}

/// **The quad's PostProcess**: the bike's air state on four wheels, with no no-hander.
///
/// The speed (the velocity's length), the velocity into the quad's frame, the forward and up axes and
/// the euler angles off the orientation, the angular velocity into the world. On the ground with any
/// wheel down, all down with all four, a front or a rear down with either of the pair; the hang time
/// restarts on leaving the ground and counts while off it; the contact velocity is the first touching
/// wheel's (left front, right front, left rear, right rear), else the body's own. The crossup is the
/// bike's — the flattened heading against the flattened velocity, through the arc-cosine table — when
/// neither flipping nor spinning and above 10 ft/s, ramped with the authority after a flip. The
/// collision record is told the quad is tipped when its up axis's `y` is at or below 0.25. Then the
/// vehicle's post step and the scene node's position, kept in a second copy.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn air_state(q: &mut Quad, body: &mut Body, dt: F, table: &crate::air::AcosTable, m: &mut impl Moto) {
    use crate::air::{cross, normalize};
    if !q.active {
        return;
    }
    let v = body.velocity;
    q.speed_len = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(v.x(), v.x()), v.y(), v.y()), v.z(), v.z())));
    q.local_vel = body.to_local(v);
    let (fwd, up) = quat_axes(body.orientation);
    q.heading = Vf::new(fwd[0].0, fwd[1].0, fwd[2].0, q.heading.w());
    q.up = Vf::new(up[0].0, up[1].0, up[2].0, q.up.w());
    q.euler = crate::math::quat_to_euler(body.orientation, table);
    q.world_ang_vel = body.to_world(body.ang_vel);

    q.was_airborne = q.airborne;
    let t = q.wheels.map(|w| w.touching);
    q.on_ground = t.iter().any(|&t| t);
    q.all_down = t.iter().all(|&t| t);
    q.front_down = t[LEFT_FRONT] || t[RIGHT_FRONT];
    q.rear_down = t[LEFT_REAR] || t[RIGHT_REAR];
    q.airborne = !q.on_ground;
    if !q.was_airborne && q.airborne {
        q.hang_time = F::ZERO;
    }
    if q.airborne {
        q.hang_time = q.hang_time + dt;
    }
    q.contact_vel = match q.wheels.iter().find(|w| w.touching) {
        Some(w) => w.contact_vel,
        None => q.local_vel,
    };

    let heading = normalize(Vf::new(q.heading.x(), 0.0, q.heading.z(), 1.0));
    let mut angle = F::ZERO;
    if !q.flipping && !q.spinning && !(q.speed_len <= F(CROSSUP_SPEED)) {
        let vel = normalize(Vf::new(v.x(), 0.0, v.z(), 1.0));
        let c = cross(heading, vel);
        let sin = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(c.x(), c.x()), c.y(), c.y()), c.z(), c.z())));
        let sin = clamp_unit(clamp_unit(clamp_unit(sin)));
        angle = F(HALF_PI) - table.at(sin);
        if c.y() < 0.0 {
            angle = angle * F(-1.0);
        }
    }
    q.crossup = if !(q.authority <= F::ZERO) {
        if q.authority_time == F::ZERO {
            F::ZERO
        } else {
            map_clamp(q.authority, q.authority_time, F::ZERO, F::ZERO, angle)
        }
    } else {
        angle
    };
    q.tipped = q.up.y() <= TIPPED_UP;

    m.call(Call::VehiclePost(dt), q, body);
    m.call(Call::NodePosition, q, body);
    q.node_pos_kept = q.node_pos;
}

/// `[-1, 1]` as the original's compares and moves: anything unordered comes back as 1.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn clamp_unit(c: F) -> F {
    if !(c <= F::ONE) {
        F::ONE
    } else if c < F(-1.0) {
        F(-1.0)
    } else {
        c
    }
}
