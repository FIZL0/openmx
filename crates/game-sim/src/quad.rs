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
//! And the quad's once-a-frame [`update`] (MD64a): the bike's vehicle update, then the quad's own
//! tail on four wheels (hazard surfaces, rumble, the wheelie and stoppie clocks, the ride stats).
//!
//! What they call of SLUS-21104's bike steps (engine, preload, backflip, air drag, airborne control,
//! the wheels' suspension, tire forces and integration, the input limiter, the gearbox and clutch) is
//! not ported here: each is a [`Call`] handed to a [`Moto`], in the original's order.
//!
//! The quad's [`put_back`] and [`net_unpack`] (MD65): the bike's put-back on four wheels, and a remote
//! quad set from its network snapshot. The PC slots it had not paired are MD65b's.
//!
//! The quad's wreck body (MD65a): the init's last block, [`build_wreck_body`], makes a ray probe off
//! `WreckATVm.col` that the quad's other collidables ignore, and the wreck entry and the unwreck
//! [`swap_rays`] the quad's collision between it and the riding rays.
//!
//! The quad's small slots (MD65b): the level setup's [`remap_surfaces`], the [`surface_under`] and
//! [`strut_travel`] answers, the [`trigger_segments`] the trigger probe is shaped to, and the
//! [`settle_wheels`] onto the ground.
//!
//! The quad's once-a-frame animation driver, [`animate`] (MD65c): the clip fade, the stunt it plays
//! (the stand-upright's guard word) and the riding pose's blends (put-back's settle word), with the
//! clips, node pose and hung nodes as [`Call`]s.
//!
//! And the quad's tire reader, [`tire_phy`] (MD64b): `[Front Tire]` and `[Rear Tire]` into the four
//! wheels ([`TireSetup`]), with the original's left-to-right copies and their quirks.
//!
//! The quad is described by what the steps read, [`Quad`] and [`Body`], not by its layout: the field
//! map is research/oracle.txt's (`# MD37`, `# MD37a`, `# MD37b`, `# MD64b`, `# MD65`).

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
    /// What the scene node answers the net unpack's [`Call::NodeLocal`] with: the body's quadword
    /// at `+0xb0` brought into the node's frame (what that quadword is was not traced).
    pub node_local: Vf,
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
    /// The centre of mass in the world, and the correction a bottomed-out wheel takes its closing
    /// speed out of (MD75a: only the wheels' steps, run by [`Steps`], read and write them).
    pub position: Vf,
    pub correction: Vf,
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
    InputLimiter(F),
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
    /// The update's ([`update`]): the bike's own vehicle update (first); the vehicle's hazard-surface
    /// clock, handed the longest of the four wheels' surface times and whether a wheel is sunk in one;
    /// the pad's rumble (`soft` the gentler of the two, `strength` 0..255, for 0.2 s); the rider's
    /// wheelie or stoppie stunt started (the stunt id) and ended; the player's countdown run out, then
    /// each of the player's listeners and each of the race's watchers told so; and the message posted
    /// last (its id).
    BikeVehicleUpdate(F),
    HazardSurface { dt: F, time: F, sunk: bool },
    Rumble { soft: bool, strength: i32 },
    StartStunt(u32),
    EndStunt,
    CountdownOut,
    CountdownListener(u32),
    CountdownWatcher(u32),
    Post(u32),
    /// The put-back's ([`put_back`], MD65): each wheel's contact and suspension state reset; the
    /// quad's attachment taken off what it hangs on; the vehicle's own put-back (`place` and `keep`
    /// handed on as they came, `flags` the quad's flag byte as the vehicle's put-back sees it); the
    /// attachment hung back on; the two frame nodes asked for their world frames, and each hung node
    /// given one (`hung` its place in the list, `frame` 0 or 1); the network snapshot packed; the
    /// actor's listeners told it moved.
    WheelReset(usize),
    Detach,
    VehiclePutBack { place: bool, keep: bool, flags: u8 },
    Reattach,
    ReadFrame(usize),
    SetFrame { hung: usize, frame: usize },
    NetPack,
    ActorMoved,
    /// The net unpack's ([`net_unpack`], MD65): the scene node turned to the snapshot's orientation;
    /// the node asked for a quadword of the body's in its own frame (into [`Quad::node_local`]); and,
    /// after the wheel rounds, the vehicle's own end of the unpack.
    NodeOrientation,
    NodeLocal,
    NetPost(F),
    /// What the bike steps [`Steps`] runs call in turn (MD75a): the preload's cue posted (category 8,
    /// id 14, the bike's `+0xd20` handed on); the bike's wreck entered (the reason) when a backflip
    /// lands mid-clip — the stunt's start and end are [`Call::StartStunt`] and [`Call::EndStunt`];
    /// and a wheel's surface drag and spin, which its integrate runs before the steering hinge.
    PreloadCue,
    Wreck(u8),
    SurfaceDrag(usize, F),
    SpinIntegrate(usize, F),
    /// What the gravity [`Steps`] runs calls in turn (MD75b): the collision record's virtual, handed 1.
    GravityRecord,
    /// What the vehicle post [`Steps`] runs calls in turn (MD75b1, [`crate::atv_moto::vehicle_post`]):
    /// with the quad's origin outside the world, the body's reset (vt `+0x13c`) and the respawn (vt
    /// `+0xe8`); then the spatial update (vt `+0xe4`), the node's bounds when bit 1 of the flag byte is
    /// set (`atv_node_bounds`), and the collision sync (vt `+0x124`).
    BodyReset,
    Respawn,
    SpatialUpdate,
    NodeBounds,
    CollisionSync,
    /// What those three [`Steps`] runs call in turn (MD75c): the quad's node's bounds gathered again
    /// (its vt `+0x94`) when they are stale ([`crate::atv_moto::Scene::bounds_stale`]; MD75c3: `Steps`
    /// runs it, [`crate::atv_moto::Scene::rebound`]); a box moved in
    /// the world's spatial database (`atv_spatial_move`) — the quad's own entry (`node` false: its node's
    /// box, reaching 0) or its node's ([`crate::atv_moto::Scene::database_box`]); and a collidable
    /// brought up to date (`atv_part_sync`) and the nearby list gathered round the active one
    /// ([`crate::atv_world`]'s gather, kind 3), from [`crate::atv_moto::collision_sync`].
    NodeRebound,
    SpatialMove { node: bool, centre: Vf, half: Vf, reach: F },
    PartSync(Part),
    Gather(Part),
    /// What the part sync [`Steps`] runs calls in turn (MD75c2, [`crate::atv_sync::part_sync`]): the
    /// part's entry (its `+0x18`) moved in the world's spatial database (`atv_spatial_move`) to its box,
    /// reaching its reach.
    PartMove { part: Part, centre: Vf, half: Vf, reach: F },
    /// The wreck body's ([`build_wreck_body`], [`swap_rays`], MD65a): a ray-probe part made; its flag
    /// byte set from a flags word; it hung in the quad's scene tree; the wreck rays loaded from
    /// [`WRECK_FILE`] on the quad's scene node, then readied; a part shown in or hidden from the scene,
    /// taken off the collision world it is in, or put in the quad's world.
    PartMade(Part),
    PartFlags(Part, u32),
    PartHung(Part),
    PartLoad(Part),
    PartReady(Part),
    PartHide(Part),
    PartShow(Part),
    PartOffWorld(Part),
    PartInWorld(Part),
    /// The quad's level setup's ([`remap_surfaces`], MD65b): a global flag set, then the bike's shared
    /// level setup, handed the level.
    LevelSetup,
    /// The animation driver's ([`animate`], MD65c), in its order: in vehicle states 3 and 4 the
    /// biplane's base driver's ([`base_animate`], MD65c1) — the quad put between two of its network
    /// snapshots (vt `+0x3ec`; `older` and `newer` are places in [`NetClock::times`], `t` how far from
    /// the older to the newer, clamped to `-0.9..1.9`) and the net unpack (vt `+0x3f0`, handed `dt`:
    /// [`net_unpack`] is its port); the rider's animation stopped; one clip played or two blended (`clip`s are places
    /// in [`Anim`]'s clip table, `weight` and `mix` clamped to `0..1`, `fade` [`Anim::fade`]); the
    /// model's nodes posed; then [`Call::ReadFrame`] and [`Call::SetFrame`] as the put-back's, and the
    /// attachment's box moved in the spatial database.
    NetInterpolate { older: usize, newer: usize, t: F },
    NetUnpack(F),
    /// The net interpolation's ([`net_interpolate`], MD65c2): with the snapshot more than 16 ft from
    /// where the quad was put, the quad's put-back (vt `+0x3a8`, handed place and keep clear:
    /// [`put_back`] is its port).
    NetPutBack,
    AnimStop,
    PlayClip { clip: i32, weight: F, fade: F },
    BlendClips { clips: [i32; 2], weight: F, mix: F, fade: F },
    PoseNodes,
    AttachmentMoved,
}

/// Whoever runs the calls the quad's listeners make. A `Vec<Call>` just lists them.
pub trait Moto {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body);
    /// MD75e: the frame's whole rigid body before a listener runs, and after, for the words of it a
    /// runner keeps a copy of (the last position, the enabled flag).
    fn body_in(&mut self, _b: &crate::atv_body::Body) {}
    fn body_out(&mut self, _b: &mut crate::atv_body::Body) {}
    /// MD105a: the [`update`]'s [`Tail`] before each call it makes, and after, for a runner that keeps
    /// it where the calls read and write it (the original reads each word where it uses it).
    fn tail_out(&mut self, _t: &Tail) {}
    fn tail_in(&mut self, _t: &mut Tail) {}
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
    m.call(Call::InputLimiter(dt), q, body);
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
pub(crate) fn donut(p: &Params, a: F) -> F {
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

// --- MD64a: the update ------------------------------------------------------------------------------

/// The stick's rumble: above this speed, and only past the harder of two suspension squeezes.
const RUMBLE_SPEED: f32 = 10.0;
const RUMBLE_HARD: f32 = 0.95;
const RUMBLE_SOFT: f32 = 0.73;
/// The rumble's strength off the squeeze (× 255), and the least each kind gives.
const RUMBLE_HARD_SCALE: f32 = 0.45;
const RUMBLE_SOFT_SCALE: f32 = 0.5;
const RUMBLE_FULL: f32 = 255.0;
const RUMBLE_HARD_LEAST: i32 = 95;
const RUMBLE_SOFT_LEAST: i32 = 40;
/// A wheelie or a stoppie is a stunt after this long on the one pair (s).
const ONE_PAIR: f32 = 1.0;
/// Off the ground for longer than this, a wheelie or a stoppie is over (s).
const ONE_PAIR_HANG: f32 = 0.1;
/// The player flag that stops the ride clock.
const RIDE_CLOCK_STOPPED: u32 = 0x200;

/// One entry of the quad's surface hazard table, one per surface: whether riding on it is a hazard,
/// and how long the vehicle's clock lets the quad stay on it (−1: no time at all).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    pub hazard: bool,
    pub time: F,
}

/// The player the quad belongs to, as the update reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Player {
    /// Its flags; [`RIDE_CLOCK_STOPPED`] holds the ride clock.
    pub flags: u32,
    /// Its distance this frame, added to a running wheelie or stoppie's.
    pub step: F,
    /// A word whose fall to zero the update counts (guessed: not traced to a writer).
    pub word: u32,
    /// Its speed, which the quad keeps the top of.
    pub speed: F,
    /// A countdown the update runs while the race's clock is at or below zero.
    pub countdown: F,
    /// How many listeners it tells when the countdown runs out.
    pub listeners: u32,
}

/// The race the player is in, as the update reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Race {
    /// The countdown only runs while this is at or below zero (guessed: the race's start clock).
    pub clock: F,
    /// How many watchers it tells when the player's countdown runs out.
    pub watchers: u32,
}

/// What the quad's [`update`] reads and keeps beyond [`Quad`]. The field map is research/oracle.txt's
/// `# MD64a`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tail {
    /// The vehicle state's mode, or none (no state: only the bike's update runs). The rumble and the
    /// second countdown run in mode 1 alone.
    pub vehicle_state: Option<u32>,
    /// The rider's hold: set, the update does nothing at all.
    pub rider_held: bool,
    /// Below this speed (the contact velocity's `z`), a wheelie or a stoppie neither starts nor runs.
    pub stunt_speed: F,
    pub surfaces: [Surface; 16],
    /// Each wheel's surface (an index into `surfaces`), its sunk flag (guessed from use), and its
    /// suspension's squeeze.
    pub wheel_surface: [u8; 4],
    pub wheel_sunk: [bool; 4],
    pub wheel_squeeze: [F; 4],
    /// Some wheel is on a hazard surface.
    pub on_hazard: bool,
    /// A request word kept for one frame only: still set with its last-frame copy set, it is cleared.
    pub request: u32,
    pub request_was: u32,
    /// The pad to rumble, and whether rumble is on for it.
    pub pad: bool,
    pub rumble_on: bool,
    /// The wheelie (rear pair only) and stoppie (front pair only) clocks: the stunt starts once one
    /// passes [`ONE_PAIR`].
    pub wheelie_clock: F,
    pub stoppie_clock: F,
    pub wheelie_stunt: u32,
    pub stoppie_stunt: u32,
    /// The running wheelie's and stoppie's time and distance, and the bests of each.
    pub wheelie_time: F,
    pub stoppie_time: F,
    pub wheelie_dist: F,
    pub stoppie_dist: F,
    pub best_wheelie_time: F,
    pub best_stoppie_time: F,
    pub best_wheelie_dist: F,
    pub best_stoppie_dist: F,
    pub player: Option<Player>,
    pub race: Option<Race>,
    /// The ride stats: time ridden, top speed, distance forward, and how often the player's word fell
    /// to zero (with the word as last seen).
    pub ride_time: F,
    pub top_speed: F,
    pub distance: F,
    pub word_falls: u32,
    pub word_was: u32,
    /// The scene node's up and forward rows: the distance is measured along the forward row flattened,
    /// or the up row when the forward one points straight up or down.
    pub node_up: Vf,
    pub node_forward: Vf,
    /// A second countdown (mode 1 only): on, it counts down until below zero, then turns itself off.
    pub second_on: u32,
    pub second_countdown: F,
    /// A clock that only counts.
    pub clock: F,
    /// The message the update posts last.
    pub post: u32,
}

/// **The quad's update**, once a frame: the bike's vehicle update, then the quad's own tail.
///
/// With no vehicle state it is the bike's update alone, and with the rider held nothing runs. Else,
/// after the bike's update, in order:
/// - the hazard surfaces: on one when any wheel's surface is a hazard, sunk when also any wheel is
///   sunk; the vehicle's hazard clock is handed the longest of the four wheels' surface times;
/// - the one-frame request cleared once it has been seen;
/// - the rumble, above 10 ft/s in mode 1 and not wrecked: the four suspensions' biggest squeeze past
///   0.95 rumbles hard (0.45 × 255 × squeeze, 95 at the least), past 0.73 soft (0.5 × 255, 40);
/// - unless wrecked (when both clocks are zeroed): on the rear pair alone and not slow, the wheelie
///   clock runs, and past a second the wheelie stunt starts (when its time is not yet running) and its
///   time and distance run, keeping their bests; the same for the stoppie on the front pair alone;
///   down on all four, or slow on the ground, or in the air past 0.1 s, a running stunt is ended and
///   both clocks, times and distances are zeroed; anything else leaves them as they are;
/// - with a player: the ride clock (unless the player holds it), the top speed, the distance forward
///   (the speed with the front pair down, else the body's velocity along the node's flattened forward
///   row; never backward), the word's falls counted, and the player's countdown, which runs out by
///   telling the player's listeners and the race's watchers;
/// - the second countdown in mode 1, the clock, and the message posted.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn update(t: &mut Tail, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    let Some(mode) = t.vehicle_state else {
        tail_call(t, Call::BikeVehicleUpdate(dt), q, body, m);
        return;
    };
    if t.rider_held {
        return;
    }
    let slow = q.speed() < t.stunt_speed;
    let (node_up, node_forward) = (t.node_up, t.node_forward);
    tail_call(t, Call::BikeVehicleUpdate(dt), q, body, m);

    let surface = t.wheel_surface.map(|s| t.surfaces[usize::from(s)]);
    t.on_hazard = surface.iter().any(|s| s.hazard);
    let sunk = t.on_hazard && t.wheel_sunk.iter().any(|&s| s);
    let time = fpu_max(fpu_max(surface[RIGHT_REAR].time, surface[LEFT_REAR].time), surface[RIGHT_FRONT].time);
    let time = fpu_max(surface[LEFT_FRONT].time, time);
    tail_call(t, Call::HazardSurface { dt, time, sunk }, q, body, m);

    if t.request != 0 && t.request_was != 0 {
        t.request = 0;
    }
    t.request_was = t.request;

    if !(q.speed_len <= F(RUMBLE_SPEED)) && mode == 1 && !q.control_off {
        let sq = t.wheel_squeeze;
        let squeeze = fpu_max(fpu_max(sq[LEFT_FRONT], sq[RIGHT_FRONT]), fpu_max(sq[LEFT_REAR], sq[RIGHT_REAR]));
        let (hard, strength) = if !(squeeze <= F(RUMBLE_HARD)) {
            (true, rumble_strength(squeeze, RUMBLE_HARD_SCALE, RUMBLE_HARD_LEAST))
        } else if !(squeeze <= F(RUMBLE_SOFT)) {
            (false, rumble_strength(squeeze, RUMBLE_SOFT_SCALE, RUMBLE_SOFT_LEAST))
        } else {
            (false, 0)
        };
        if strength > 0 && t.pad && t.rumble_on {
            tail_call(t, Call::Rumble { soft: !hard, strength }, q, body, m);
        }
    }

    if q.control_off {
        t.wheelie_clock = F::ZERO;
        t.stoppie_clock = F::ZERO;
    } else if !q.front_down && q.rear_down && !slow {
        one_pair(t, q, body, dt, m, true);
    } else if q.front_down && !q.rear_down && !slow {
        one_pair(t, q, body, dt, m, false);
    } else if (q.front_down && q.rear_down) || (!q.airborne && slow) || (q.airborne && !(q.hang_time <= F(ONE_PAIR_HANG))) {
        if !(t.wheelie_time <= F::ZERO) || !(t.stoppie_time <= F::ZERO) {
            tail_call(t, Call::EndStunt, q, body, m);
        }
        t.wheelie_clock = F::ZERO;
        t.stoppie_clock = F::ZERO;
        t.wheelie_time = F::ZERO;
        t.stoppie_time = F::ZERO;
        t.wheelie_dist = F::ZERO;
        t.stoppie_dist = F::ZERO;
    }

    if let Some(mut p) = t.player {
        if p.flags & RIDE_CLOCK_STOPPED == 0 {
            t.ride_time = t.ride_time + dt;
        }
        if !(p.speed <= t.top_speed) {
            t.top_speed = p.speed;
        }
        let mut forward = if q.front_down {
            q.speed()
        } else {
            let row = if node_forward.y() <= -1.0 || 1.0 <= node_forward.y() { node_up } else { node_forward };
            let dir = crate::air::normalize(Vf::new(row.x(), 0.0, row.z(), row.w()));
            let v = body.velocity;
            Acc::adda(F::ZERO, F(v.z()) * F(dir.z())).madd(v.x(), dir.x())
        };
        if forward < F::ZERO {
            forward = F::ZERO;
        }
        t.distance = Acc::adda(F::ZERO, t.distance).madd(forward, dt);
        if t.word_was != 0 && p.word == 0 {
            t.word_falls = t.word_falls.wrapping_add(1);
        }
        t.word_was = p.word;
        if let Some(race) = t.race.filter(|r| !(p.countdown <= F::ZERO) && r.clock <= F::ZERO) {
            p.countdown = p.countdown - dt;
            if p.countdown <= F::ZERO {
                t.player = Some(p);
                tail_call(t, Call::CountdownOut, q, body, m);
                for i in 0..p.listeners {
                    tail_call(t, Call::CountdownListener(i), q, body, m);
                }
                for i in 0..race.watchers {
                    tail_call(t, Call::CountdownWatcher(i), q, body, m);
                }
                p = t.player.unwrap_or(p);
            }
        }
        t.player = Some(p);
    }

    if t.second_on != 0 && mode == 1 {
        if t.second_countdown < F::ZERO {
            t.second_on = 0;
        } else {
            t.second_countdown = t.second_countdown - dt;
        }
    }
    t.clock = t.clock + dt;
    tail_call(t, Call::Post(t.post), q, body, m);
}

/// `max.s`.
fn fpu_max(a: F, b: F) -> F {
    a.max(b)
}

/// The rumble's strength: `255 × (scale × squeeze)` truncated, and `least` when that is under it.
fn rumble_strength(squeeze: F, scale: f32, least: i32) -> i32 {
    let s = (F(RUMBLE_FULL) * (F(scale) * squeeze)).to_i32();
    if s <= least { least } else { s }
}

/// A wheelie's (`wheelie`) or a stoppie's frame on the one pair: its clock runs, and past a second
/// the stunt starts if its time is not running yet, and its time and distance run with their bests.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn one_pair(t: &mut Tail, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto, wheelie: bool) {
    let step = t.race.and(t.player).map(|p| p.step);
    let stunt = if wheelie { t.wheelie_stunt } else { t.stoppie_stunt };
    let clock = if wheelie { &mut t.wheelie_clock } else { &mut t.stoppie_clock };
    *clock = *clock + dt;
    if *clock <= F(ONE_PAIR) {
        return;
    }
    if (if wheelie { t.wheelie_time } else { t.stoppie_time }) <= F::ZERO {
        tail_call(t, Call::StartStunt(stunt), q, body, m);
    }
    let (time, best_time, dist, best_dist) = if wheelie {
        (&mut t.wheelie_time, &mut t.best_wheelie_time, &mut t.wheelie_dist, &mut t.best_wheelie_dist)
    } else {
        (&mut t.stoppie_time, &mut t.best_stoppie_time, &mut t.stoppie_dist, &mut t.best_stoppie_dist)
    };
    *time = *time + dt;
    if !(*time <= *best_time) {
        *best_time = *time;
    }
    if let Some(step) = step {
        *dist = *dist + step;
        if !(*dist <= *best_dist) {
            *best_dist = *dist;
        }
    }
}

/// One of the [`update`]'s calls, its [`Tail`] handed to the runner before and taken back after.
fn tail_call(t: &mut Tail, c: Call, q: &mut Quad, body: &mut Body, m: &mut impl Moto) {
    m.tail_out(t);
    m.call(c, q, body);
    m.tail_in(t);
}

// --- the tire reader (MD64b) ----------------------------------------------------------------------

/// One surface's row of a wheel's tables, in the three setters' argument order: the drive setter's
/// eight (`SideForce`, `SlippingSideForce`, `MaxDriveForce`, `MaxSideForce`, `MaxSlippingSideForce`,
/// `SkidDriveForce`, `SkidSideForce`, `SkidSlippingSideForce`), the side setter's thirteen
/// (`SlippingSideForce`, `BrakingSideForce` and `SideForce` `RoundOff`/`DropOff`/`Scale`/`SpeedScale`,
/// then `DonutsSideForceSpeedScale`) and the drag setter's three (`MaxBrakingTorque`, `WheelFriction`,
/// `WheelDrag`). The wheel keeps each as a table of sixteen surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TireSurface {
    pub drive: [F; 8],
    pub side: [F; 13],
    pub drag: [F; 3],
}

impl TireSurface {
    fn of(s: &crate::phy::Surface) -> TireSurface {
        TireSurface {
            drive: [
                s.side_force,
                s.slipping_side_force,
                s.max_drive_force,
                s.max_side_force,
                s.max_slipping_side_force,
                s.skid_drive_force,
                s.skid_side_force,
                s.skid_slipping_side_force,
            ],
            side: [
                s.slipping_side_force_round_off,
                s.slipping_side_force_drop_off,
                s.slipping_side_force_scale,
                s.slipping_side_force_speed_scale,
                s.braking_side_force_round_off,
                s.braking_side_force_drop_off,
                s.braking_side_force_scale,
                s.braking_side_force_speed_scale,
                s.side_force_round_off,
                s.side_force_drop_off,
                s.side_force_scale,
                s.side_force_speed_scale,
                s.donuts_side_force_speed_scale,
            ],
            drag: [s.max_braking_torque, s.wheel_friction, s.wheel_drag],
        }
    }
}

/// What the tire reader sets on one of the four wheels (and the two things of the wheel it reads:
/// where the wheel is mounted, and whether it has a sweep shape). Whatever the reader does not reach
/// keeps what it had: the wheel's constructor's, or the last read's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelTire {
    /// `UseParametricCurve` as the ini's integer read gives it (any number, not only 0 or 1).
    pub parametric: i32,
    /// The suspension setter's: travel, the two rate limits, the spring rate (the file's rate times
    /// the quad's weight) and the two damping rates against it.
    pub strut_compression: F,
    pub max_compression_rate: F,
    pub max_decompression_rate: F,
    pub spring: F,
    pub damping_in: F,
    pub damping_out: F,
    /// The tire setter's: the spin inertia and its inverse (positive radius only), `TireInertia`, the
    /// radius, the two widths it is handed (24 and 6), the tire's mass, one over the radius (positive
    /// radius only) and a scale nothing reads.
    pub spin_inertia: F,
    pub inv_spin_inertia: F,
    pub tire_inertia: F,
    pub radius: F,
    pub ply: F,
    pub width: F,
    pub tire_mass: F,
    pub inv_radius: F,
    pub ply_scale: F,
    /// The steering setter's: damping, inertia and its inverse (a positive inertia only), torque.
    pub steer_damping: F,
    pub steer_inertia: F,
    pub inv_steer_inertia: F,
    pub steer_torque: F,
    /// The steering lock, radians, and its negative.
    pub max_steer_angle: F,
    pub min_steer_angle: F,
    /// `BodyTireForcePosition`, and the rear's `OtherTireNormalLoadScale`.
    pub force_position: Vf,
    pub other_load_scale: F,
    /// Where the wheel is mounted (read), and the derived setter's rest point: the mount, a radius
    /// lower.
    pub mount: Vf,
    pub rest: Vf,
    /// The wheel has a sweep shape (read): the derived setter rebuilds it.
    pub sweep: bool,
    pub surfaces: [TireSurface; crate::phy::MAX_SURFACES],
}

/// What the tire reader sets: the four wheels, each pair's spring rate stopped and moving (as
/// [`Params::front_spring`] and [`Params::rear_spring`]), and the front's mud per surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TireSetup {
    pub wheels: [WheelTire; 4],
    pub front_spring: (F, F),
    pub rear_spring: (F, F),
    pub mud_time: [F; crate::phy::MAX_SURFACES],
    pub mud_amount: [F; crate::phy::MAX_SURFACES],
}

/// The suspension setter: the rates follow the spring it is handed, so the last call wins.
fn set_suspension(w: &mut WheelTire, t: &Keys, spring: F, mass: F) {
    w.strut_compression = t.strut;
    w.max_compression_rate = t.max_compression;
    w.max_decompression_rate = t.max_decompression;
    w.spring = crate::phy::spring_rate(spring, mass);
    w.damping_in = crate::phy::damping(t.bounce, mass, w.spring);
    w.damping_out = crate::phy::damping(t.rebound, mass, w.spring);
}

/// The tire setter, with the 6 and 24 the reader always hands it.
fn set_tire(w: &mut WheelTire, radius: F, inertia: F) {
    w.radius = radius;
    w.width = F(6.0);
    w.ply = F(24.0);
    w.tire_inertia = inertia;
    w.ply_scale = crate::tire::tire_ply_scale();
    w.tire_mass = crate::tire::tire_mass(radius);
    if let Some((inv_radius, spin, inv_spin)) = crate::tire::spin_inertia(radius) {
        w.inv_radius = inv_radius;
        w.spin_inertia = spin;
        w.inv_spin_inertia = inv_spin;
    }
}

/// The steering setter: damping, inertia (only a positive one), torque.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compare
fn set_steering(w: &mut WheelTire, damping: F, inertia: F, torque: F) {
    w.steer_damping = damping;
    w.steer_torque = torque;
    if F::ZERO < inertia {
        w.steer_inertia = inertia;
        w.inv_steer_inertia = F::ONE / inertia;
    }
}

/// The derived setter: the sweep shape (if any) rebuilt as the segment from the strut's top down to a
/// radius below the mount, and the rest point a radius under the mount.
fn set_derived(w: &mut WheelTire) -> Option<[Vf; 2]> {
    w.rest = w.mount;
    w.rest.0[1] = (F(w.mount.y()) - w.radius).0;
    w.sweep.then(|| [Vf::new(0.0, w.strut_compression.0, 0.0, 1.0), Vf::new(0.0, (-w.radius).0, 0.0, 1.0)])
}

/// One section's scalar keys, with the reader's defaults.
struct Keys {
    parametric: i32,
    strut: F,
    max_compression: F,
    max_decompression: F,
    radius: F,
    inertia: F,
    spring: F,
    spring_stopped: F,
    bounce: F,
    rebound: F,
}

impl Keys {
    fn read(s: crate::phy::Section<'_>) -> Keys {
        Keys {
            parametric: s.int("UseParametricCurve").unwrap_or(0),
            strut: s.float_or("StrutCompression", F::ONE),
            max_compression: s.float_or("MaxCompressionRate", F(20.0)),
            max_decompression: s.float_or("MaxDecompressionRate", F::ONE),
            radius: s.float_or("TireRadius", F::ONE),
            inertia: s.float_or("TireInertia", F::ONE),
            spring: s.float_or("Spring", F::ZERO),
            spring_stopped: s.float_or("SpringWhenStopped", F::ZERO),
            bounce: s.float_or("BounceDampingRatio", F::ZERO),
            rebound: s.float_or("ReboundDampingRatio", F::ZERO),
        }
    }
}

/// **The quad's tire reader**: `[Front Tire]` into the two front wheels and `[Rear Tire]` into the two
/// rear ones. `mass` is the quad's (its weight over 32.2), which the spring rates are scaled by. It
/// returns the segment each wheel's sweep shape was rebuilt to, for the wheels that have one.
///
/// The left wheel of each pair is read into and the right one copies it, with these quirks of the
/// original kept:
/// - the stopped spring is set first and read back, then the moving one, so the damping rates are the
///   moving spring's;
/// - `MaxSteerAngle` (degrees) is the front's only; the rears keep their own lock, the right rear
///   taking the left rear's;
/// - the left rear gets no steering at all, and the right rear gets **the front's** steering keys;
/// - the front's mud keys go to the quad, one pair per surface, and the front's donut scale is zero;
/// - a `[Rear Tire]` with no `NumberOfSurfaces` reads the front's count (the same stack word).
pub fn tire_phy(t: &mut TireSetup, ini: &crate::phy::Ini, mass: F) -> [Option<[Vf; 2]>; 4] {
    use crate::phy::{DEG, MAX_SURFACES, Surface};
    const UNIT_W: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
    let mut sweeps = [None; 4];

    let s = ini.section("Front Tire");
    let k = Keys::read(s);
    let (steer_torque, steer_damping, steer_inertia) = (
        s.float_or("SteeringTorque", F::ZERO),
        s.float_or("SteeringDamping", F::ZERO),
        s.float_or("SteeringInertia", F::ZERO),
    );
    let lf = &mut t.wheels[LEFT_FRONT];
    lf.parametric = k.parametric;
    lf.max_steer_angle = s.float_or("MaxSteerAngle", F::ZERO);
    lf.force_position = s.vector("BodyTireForcePosition", UNIT_W);
    set_suspension(lf, &k, k.spring_stopped, mass);
    let stopped = lf.spring;
    set_suspension(lf, &k, k.spring, mass);
    t.front_spring = (stopped, lf.spring);
    set_tire(lf, k.radius, k.inertia);
    set_steering(lf, steer_damping, steer_inertia, steer_torque);
    sweeps[LEFT_FRONT] = set_derived(lf);
    lf.max_steer_angle = lf.max_steer_angle * F(DEG);
    lf.min_steer_angle = -lf.max_steer_angle;
    let count = s.int("NumberOfSurfaces");
    for i in 1..=count.unwrap_or(0).min(MAX_SURFACES as i32) {
        let row = Surface::read(s, i, true);
        let j = (i - 1) as usize;
        t.mud_time[j] = row.mud_time;
        t.mud_amount[j] = row.mud_amount;
        for w in [LEFT_FRONT, RIGHT_FRONT] {
            t.wheels[w].surfaces[j] = TireSurface::of(&row);
        }
    }
    let lf = t.wheels[LEFT_FRONT];
    let rf = &mut t.wheels[RIGHT_FRONT];
    rf.parametric = lf.parametric;
    rf.force_position = lf.force_position;
    rf.max_steer_angle = lf.max_steer_angle;
    rf.min_steer_angle = lf.min_steer_angle;
    set_suspension(rf, &k, k.spring, mass);
    set_tire(rf, k.radius, k.inertia);
    set_steering(rf, steer_damping, steer_inertia, steer_torque);
    sweeps[RIGHT_FRONT] = set_derived(rf);

    let s = ini.section("Rear Tire");
    let k = Keys::read(s);
    let lr = &mut t.wheels[LEFT_REAR];
    lr.parametric = k.parametric;
    lr.force_position = s.vector("BodyTireForcePosition", UNIT_W);
    lr.other_load_scale = s.float_or("OtherTireNormalLoadScale", F::ONE);
    set_suspension(lr, &k, k.spring_stopped, mass);
    let stopped = lr.spring;
    set_suspension(lr, &k, k.spring, mass);
    t.rear_spring = (stopped, lr.spring);
    set_tire(lr, k.radius, k.inertia);
    sweeps[LEFT_REAR] = set_derived(lr);
    for i in 1..=s.int("NumberOfSurfaces").or(count).unwrap_or(0).min(MAX_SURFACES as i32) {
        let row = TireSurface::of(&Surface::read(s, i, false));
        for w in [LEFT_REAR, RIGHT_REAR] {
            t.wheels[w].surfaces[(i - 1) as usize] = row;
        }
    }
    let lr = t.wheels[LEFT_REAR];
    let rr = &mut t.wheels[RIGHT_REAR];
    rr.parametric = lr.parametric;
    rr.force_position = lr.force_position;
    rr.max_steer_angle = lr.max_steer_angle;
    rr.min_steer_angle = lr.min_steer_angle;
    rr.other_load_scale = lr.other_load_scale;
    set_suspension(rr, &k, k.spring, mass);
    set_tire(rr, k.radius, k.inertia);
    set_steering(rr, steer_damping, steer_inertia, steer_torque);
    sweeps[RIGHT_REAR] = set_derived(rr);
    sweeps
}

// --- MD65: the put-back and the net unpack ----------------------------------------------------------

/// What the put-back reads and writes beyond [`Quad`]. The field map is research/oracle.txt's `# MD65`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PutBack {
    /// The quad's flag byte: its [`HIDDEN_BIT`] is cleared across the vehicle's put-back and set
    /// again after, if it was set.
    pub flags: u8,
    /// The nodes hung on the quad, by which frame each takes: 0 the first frame node's, 1 the
    /// second's, anything else neither.
    pub hung: Vec<i32>,
    /// Where the player is put back to, as three words; the first one zero is none.
    pub respawn: [u32; 3],
    /// The player's own copy of it, written only with a player and a respawn point; `None`, no
    /// player.
    pub player_respawn: Option<[u32; 3]>,
    /// A word the put-back sets to a half (what reads it was not traced).
    pub settle: F,
    /// The update's one-frame request word ([`Tail::request`]), set by the put-back.
    pub request: u32,
}

/// The flag-byte bit the vehicle's put-back must not see set.
pub const HIDDEN_BIT: u8 = 0x2;
/// What the put-back leaves in [`PutBack::settle`].
const SETTLE: f32 = 0.5;

/// **The quad's put-back**: the bike's own put-back override, word for word, on four wheels.
///
/// The four wheels reset, the attachment taken off, then the vehicle's put-back (with the flag
/// byte's [`HIDDEN_BIT`] cleared for it, and set again after if it was set), the attachment hung back
/// on, the two frame nodes' world frames read and handed to each hung node by its kind. The quad is
/// then on the ground, not airborne, with no crossup, [`PutBack::settle`] a half and the request
/// word set; the network snapshot is packed, the player given the respawn point (when there is a
/// player and a point), and the actor's listeners told.
pub fn put_back(pb: &mut PutBack, q: &mut Quad, body: &mut Body, place: bool, keep: bool, m: &mut impl Moto) {
    for w in 0..4 {
        m.call(Call::WheelReset(w), q, body);
    }
    m.call(Call::Detach, q, body);
    let hidden = pb.flags & HIDDEN_BIT != 0;
    if hidden {
        pb.flags &= !HIDDEN_BIT;
    }
    m.call(Call::VehiclePutBack { place, keep, flags: pb.flags }, q, body);
    if hidden {
        pb.flags = pb.flags & !HIDDEN_BIT | HIDDEN_BIT;
    }
    m.call(Call::Reattach, q, body);
    m.call(Call::ReadFrame(0), q, body);
    m.call(Call::ReadFrame(1), q, body);
    for (i, &kind) in pb.hung.iter().enumerate() {
        if kind == 0 || kind == 1 {
            m.call(Call::SetFrame { hung: i, frame: kind as usize }, q, body);
        }
    }
    q.on_ground = true;
    q.airborne = false;
    pb.settle = F(SETTLE);
    q.crossup = F::ZERO;
    pb.request = 1;
    m.call(Call::NetPack, q, body);
    if pb.player_respawn.is_some() && pb.respawn[0] != 0 {
        pb.player_respawn = Some(pb.respawn);
    }
    m.call(Call::ActorMoved, q, body);
}

/// What the net unpack reads and writes beyond [`Quad`] and [`Body`]. The field map is
/// research/oracle.txt's `# MD65`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Remote {
    /// The network snapshot: where the quad is, its orientation and its velocity.
    pub position: Vf,
    pub orientation: Vf,
    pub velocity: Vf,
    /// The body's centre of mass in its own frame, and where the unpack puts it in the world.
    pub com_offset: Vf,
    pub com: Vf,
    /// The steering's lag filter: its value, its time constant and the fraction it last moved by.
    pub steer: F,
    pub steer_time: F,
    pub steer_fraction: F,
    /// The left front's negative steering lock (the tire reader's −MaxSteerAngle).
    pub min_steer_angle: F,
    /// Each wheel's spin angle (kept within ±π) and rate.
    pub spin: [F; 4],
    pub spin_rate: [F; 4],
}

/// The steering the snapshot's node quadword asks for, per unit of its `x`.
const NET_STEER: f32 = 0.5;
const ONE_OVER_TWO_PI: f32 = 0.15915494;
const TWO_PI: f32 = 6.2831855;

/// A spin angle back within ±π: less the whole turns in it (truncated), then one turn more either
/// way if it is still past π.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub(crate) fn wrap_angle(a: F) -> F {
    let turns = F(fpu::from_i32((F(ONE_OVER_TWO_PI) * a).to_i32()));
    let w = Acc::adda(F::ZERO, a).msub(F(TWO_PI), turns);
    if !(w <= F(PI)) {
        w - F(TWO_PI)
    } else if w < -F(PI) {
        F(TWO_PI) + w
    } else {
        w
    }
}

/// **The quad's net unpack**: a remote quad put where its network snapshot says, its wheels spun
/// and steered to match, and the four wheels run against the ground.
///
/// The body takes the snapshot's velocity and orientation, the node's position (and its kept copy)
/// the snapshot's position, and the centre of mass that position plus the body's offset turned into
/// the world. The body's own angular velocity is zeroed and the speed is the velocity's length. The
/// scene node is turned to the orientation; each wheel spins at the velocity's forward part (in the
/// quad's frame) over its radius, its angle integrated and wrapped to ±π. The steering is half the
/// node's answer's `x` within the left front's lock, lagged by the filter (the fraction `dt` over
/// its time, at most one) into the left front's command and the quad's, which the right front
/// copies. Then the Force body's wheel rounds without the integrate (clear, [`wheel_contacts`],
/// suspension, tire forces), the ground flags off the four wheels, and the vehicle's end of the
/// unpack.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn net_unpack(r: &mut Remote, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    body.velocity = r.velocity;
    body.orientation = r.orientation;
    q.node_pos_kept = r.position;
    r.com = add_xyz(r.position, body.to_world(r.com_offset));
    q.node_pos = q.node_pos_kept;
    body.ang_vel = NOTHING;
    let v = body.velocity;
    q.speed_len = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(v.x(), v.x()), v.y(), v.y()), v.z(), v.z())));
    m.call(Call::NodeOrientation, q, body);
    let forward = F(body.to_local(body.velocity).z());
    let spin = |r: &mut Remote, q: &Quad, w: usize| {
        r.spin_rate[w] = forward * q.wheels[w].inv_radius;
        r.spin[w] = wrap_angle(Acc::adda(F::ZERO, r.spin[w]).madd(r.spin_rate[w], dt));
    };
    spin(r, q, LEFT_FRONT);
    m.call(Call::NodeLocal, q, body);
    let target = (F(NET_STEER) * F(q.node_local.x())).max(r.min_steer_angle).min(q.wheels[LEFT_FRONT].max_steer_angle);
    r.steer_fraction = dt.min(r.steer_time) / r.steer_time;
    r.steer = Acc::adda(F::ZERO, r.steer).madd(r.steer_fraction, target - r.steer);
    q.wheels[LEFT_FRONT].steer_command = r.steer;
    q.steer_command = q.wheels[LEFT_FRONT].steer_command;
    spin(r, q, RIGHT_FRONT);
    q.wheels[RIGHT_FRONT].steer_command = q.steer_command;
    spin(r, q, LEFT_REAR);
    spin(r, q, RIGHT_REAR);
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
    q.was_airborne = q.airborne;
    let t = q.wheels.map(|w| w.touching);
    q.on_ground = t.iter().any(|&b| b);
    q.all_down = t.iter().all(|&b| b);
    q.front_down = t[LEFT_FRONT] || t[RIGHT_FRONT];
    q.rear_down = t[LEFT_REAR] || t[RIGHT_REAR];
    q.airborne = !q.on_ground;
    m.call(Call::NetPost(dt), q, body);
}

// --- the physics frame (MD75) ---------------------------------------------------------------------

/// SLUS-21104's physics world as the quad's frame sees it: one rigid body, the quad's three
/// listeners, no constraints.
struct Frame<'a, M> {
    p: &'a Params,
    q: &'a mut Quad,
    body: &'a mut crate::atv_body::Body,
    table: &'a crate::air::AcosTable,
    m: &'a mut M,
}

impl<M: Moto> Frame<'_, M> {
    /// A listener on what it reads of the body, written back after.
    fn listen(&mut self, f: impl FnOnce(&Params, &mut Quad, &mut Body, &crate::air::AcosTable, &mut M)) {
        let mut b = self.body.view();
        self.m.body_in(self.body);
        f(self.p, self.q, &mut b, self.table, self.m);
        self.body.set_view(&b);
        self.m.body_out(self.body);
    }
}

impl<M: Moto> crate::biplane::Dynamics for Frame<'_, M> {
    fn count(&self, list: crate::biplane::List) -> i32 {
        i32::from(list != crate::biplane::List::Constraints)
    }
    fn preforce(&mut self, _: i32, dt: F) {
        self.listen(|p, q, b, _, m| ground_head(p, q, b, dt, m));
    }
    fn force(&mut self, _: i32, h: F) {
        self.listen(|p, q, b, _, m| ground_step(p, q, b, h, m));
    }
    fn enabled(&self, _: i32) -> bool {
        self.body.enabled
    }
    fn integrate(&mut self, _: i32, h: F) {
        self.body.integrate(h);
    }
    fn end_substep(&mut self, _: i32) {
        self.body.end_substep();
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, _: i32, dt: F) {
        self.listen(|_, q, b, table, m| air_state(q, b, dt, table, m));
    }
}

/// **The quad's physics frame**: SLUS-21104's world step over the quad alone. The world is
/// MX Unleashed's ([`crate::biplane::World::step`]; the game's has 1 iteration at 115 a second, so
/// four substeps at 30 frames a second): [`ground_head`] once with the frame's `dt`, then each
/// substep [`ground_step`], the body's [`crate::atv_body::Body::integrate`] and
/// [`crate::atv_body::Body::end_substep`] (while the body is enabled); then [`air_state`] with the
/// frame's `dt`.
///
/// In the game the world's lists hold the other riders' listeners too (the second quad's PreProcess
/// and four more), run between the quad's own; they are not part of this. SLUS-21104's world hands
/// the constraints the substep's `h` as well, which MX Unleashed's does not: the quad's world has
/// none.
pub fn frame(
    world: &crate::biplane::World,
    p: &Params,
    q: &mut Quad,
    body: &mut crate::atv_body::Body,
    dt: F,
    table: &crate::air::AcosTable,
    m: &mut impl Moto,
) {
    world.step(dt, &mut Frame { p, q, body, table, m });
}

// --- the bike's steps on the quad (MD75a) -----------------------------------------------------------

/// What SLUS-21104's bike steps keep on the quad beyond [`Quad`] and [`Body`], in the steps' own
/// types: the quad is the bike with four wheels, and its words sit where the bike's do.
///
/// Where a word is in both, [`Quad`]'s (or [`Body`]'s) is the one that counts: [`Steps`] copies it
/// into the step's types before each step and back out after. The pairs are the levers and the
/// stick's lean ([`crate::ride::Held`]'s gas, brakes, clutch, lean and reverse are
/// [`Quad::gas_limited`], [`Quad::front_brake`], [`Quad::rear_brake`], [`Quad::clutch_in`],
/// [`Quad::lean`]/[`Quad::lean_fore_aft`] and [`Quad::push_gate`]), the air state's `on_ground`,
/// `contact_vel`, `crossup`, `authority` and `flipping`, and each wheel's steering, contact, normal,
/// steering lock and angle ([`Wheel::steer_command`]), spring rate and contact velocity.
#[derive(Clone, Debug)]
pub struct Bike {
    /// The input limiter's: the rate limiter, the gas cut and how long it holds, the `[Joystick]`
    /// rates, the stick as the pad left it (the limiter leaves it as it is), the clutch lever as
    /// pressed (the clutch ramp reads it) and the start gate's penalty ([`crate::ride::Controls::gate_cut`]).
    pub held: crate::ride::Held,
    pub gas_cut: crate::handling::step::GasCut,
    pub cut_time: F,
    pub joystick: crate::phy::Joystick,
    pub stick: crate::trick::Stick,
    pub clutch: F,
    pub gate_cut: bool,
    /// The clutch ramp's keys, clock and the engine's multiplier it writes.
    pub clutch_keys: crate::r#move::Params,
    pub ride: crate::r#move::Ride,
    pub engine: crate::r#move::Engine,
    /// The air steps' keys (its `control_authority_time` is [`Quad::authority_time`]'s) and the
    /// preload's and backflip's state ([`crate::air::Air`]).
    pub air_keys: crate::air::Params,
    pub air: crate::air::Air,
    /// How long the stick has been held, and the preload cue's latch.
    pub stick_hold: F,
    pub preload_cue: bool,
    /// The 360's torque went in, and the 360 is on: the backflip's lip refuses either.
    pub spin_applied: bool,
    pub spin_active: bool,
    /// The collision record's: the quad touches something, and what it struck is of the class the
    /// mid-clip wreck's reason asks about.
    pub touching: bool,
    pub struck_class: bool,
    /// The trick buttons' byte ([`crate::handling::trick_nibble`] in its low nibble), the scene
    /// node's right and up axes' heights, the rider's stunt id and current stunt (the backflip reads
    /// `+0x1528`, the airborne control `+0x1524`), the backflip's clip and its stunt's id.
    pub buttons: u8,
    pub right_y: F,
    pub up_y: F,
    pub stunt_id: i32,
    pub rider_stunt: i32,
    pub flip_clip: crate::handling::FlipClip,
    pub flip_stunt: u32,
    /// `[Airborne Control] GasGyroTorque` and the gyro options.
    pub gas_gyro_torque: F,
    pub gyro: crate::handling::Gyro,
    /// The four wheels as SLUS-21104's wheel steps see them ([`crate::atv_wheel`]), what stands under
    /// each, and the body's centre of mass in its own frame.
    pub wheels: [crate::wheel::Wheel; 4],
    pub more: [crate::atv_wheel::More; 4],
    pub ground: [crate::atv_wheel::Ground; 4],
    pub cofg: Vf,
    /// MD75's: the words of each wheel the wheel integrate's surface drag and spin read and keep
    /// ([`crate::atv_wheel::Spin`]). Its spin, torque, spinning flag and forward force are
    /// [`crate::atv_moto::Parts::drive`]'s and its rear-pair flag, donut and `1/r` [`Wheel`]'s, which
    /// the steps copy in and back; its brake is [`Quad::front_brake`] or [`Quad::rear_brake`].
    pub spins: [crate::atv_wheel::Spin; 4],
    /// MD75b's ([`crate::atv_moto`]): the engine's wheels, the control-off pose, the body's last
    /// position and enabled word, the quad's flag byte and record. The gearbox's shift points, the
    /// head and tail's poses and travels and the drag are [`Bike::clutch_keys`]', the engine's state
    /// and gearbox [`Bike::engine`]'s.
    pub parts: crate::atv_moto::Parts,
    /// MD75e1's: the collision record and its contacts, which the gravity's call resolves
    /// ([`crate::atv_contact::Held`]); `None`, and the call goes to `rest`, as before MD75e1.
    pub contacts: Option<crate::atv_contact::Held>,
}

/// The other wheel of each wheel's pair (its `+0xbc`): the left front and left rear, the right front
/// and right rear.
const PARTNER: [usize; 4] = [2, 3, 0, 1];

/// The engine's sine and cosine table, which the spin step's contact frame turns the steering by:
/// built once, as the startup code builds it.
fn curve() -> &'static crate::camera::CurveTable {
    static CURVE: std::sync::OnceLock<crate::camera::CurveTable> = std::sync::OnceLock::new();
    CURVE.get_or_init(crate::camera::CurveTable::build)
}

/// **The quad's [`Moto`] with SLUS-21104's bike steps ported**: the input limiter and clutch
/// ([`crate::handling::step`]), the preload, backflip and airborne control ([`crate::handling`]),
/// each wheel's suspension and tire forces ([`crate::atv_wheel`], whose contact velocity and hinge
/// force [`crate::atv_wheel::run`] answers) and its integrate: the surface drag, the spin (MD75,
/// [`crate::atv_wheel::spin_integrate`]) and the steering hinge, run on the quad's fields through
/// [`Bike`].
///
/// Every other call, and what those steps call in turn and is not ported (the stunt's start and end,
/// the wreck, the preload's cue), goes to `rest` in the original's
/// order. `ground` is the world's ground query the suspension asks (the probe's end and the wheel's
/// contact record in, the point on the ground and its normal out; on Desert Oasis,
/// [`crate::atv_ground::Terrain::ground`]). The stunt start always answers yes, as SLUS-21104's does.
/// `sweep` (MD86) is the world the distributor's wheel rays are swept over ([`Sweep`]); with none
/// (`()`), [`Call::Contacts`] goes to `rest`.
pub struct Steps<R, G, S = ()> {
    pub bike: Bike,
    pub rest: R,
    pub ground: G,
    pub sweep: S,
}

/// **What the quad's wheel rays are swept over** (MD86), for [`Call::Contacts`]: the trigger probe
/// stood at `frame` (the quad's scene node's world frame), then searched over what is near it. The
/// probe's hit bit and the record's contacts; `None` for no world, and the call is not answered.
///
/// The contacts are to have no owner and no vehicle: what a wheel stands on is
/// [`Bike::ground`]'s ([`crate::atv_wheel::Ground`]), which a contact does not carry, and nobody is
/// told it was touched. Desert Oasis's ([`crate::atv_search::Oasis`]) belong to neither.
///
/// MD75c2: the same world answers [`Call::Gather`], the collision sync's gather of the nearby list
/// round the active probe, stood at `centre` with its `reach` (`false`: not answered).
pub trait Sweep {
    fn sweep(&mut self, frame: &crate::ray_probe::Matrix) -> Option<(bool, Vec<Contact>)>;

    fn gather(&mut self, _centre: Vf, _reach: F) -> bool {
        false
    }

    /// F52: whether the contacts go onto a bike's two wheels ([`crate::handling::bike::wheel_contacts`]:
    /// front or rear by `z` alone, at [`crate::handling::bike::FRONT`] and `REAR`) rather than the
    /// quad's four ([`wheel_contacts`]).
    fn two_wheels(&self) -> bool {
        false
    }
}

impl Sweep for () {
    fn sweep(&mut self, _frame: &crate::ray_probe::Matrix) -> Option<(bool, Vec<Contact>)> {
        None
    }
}

impl<S: Sweep + ?Sized> Sweep for &mut S {
    fn sweep(&mut self, frame: &crate::ray_probe::Matrix) -> Option<(bool, Vec<Contact>)> {
        (**self).sweep(frame)
    }

    fn gather(&mut self, centre: Vf, reach: F) -> bool {
        (**self).gather(centre, reach)
    }

    fn two_wheels(&self) -> bool {
        (**self).two_wheels()
    }
}

impl<R: Moto, G: FnMut(Vf, &mut u8) -> (Vf, Vf), S: Sweep> Moto for Steps<R, G, S> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body) {
        // MD75e: the backflip reads the node's right and up rows' heights where the node keeps them
        // (`+0x84`, `+0x94`), so a call that brings the node's world frame up to date moves them too.
        let rows = self.bike.parts.scene.nodes[0].world;
        self.answer(c, q, body);
        let w = self.bike.parts.scene.nodes[0].world;
        if w[1].to_bits() != rows[1].to_bits() || w[2].to_bits() != rows[2].to_bits() {
            (self.bike.right_y, self.bike.up_y) = (F(w[1].y()), F(w[2].y()));
        }
    }

    fn body_in(&mut self, b: &crate::atv_body::Body) {
        self.bike.parts.last_position = b.last_position;
        self.bike.parts.body_on = b.enabled;
        if let Some(h) = &mut self.bike.contacts {
            h.body = Some(*b);
        }
    }

    fn body_out(&mut self, b: &mut crate::atv_body::Body) {
        b.last_position = self.bike.parts.last_position;
        // MD75e1: what the record's resolution wrote of the body that the listener's view does not
        // carry (the view's words are put back already, and are the same in both).
        if let Some(w) = self.bike.contacts.as_mut().and_then(|h| h.body.take()) {
            (b.damping, b.ang_damping, b.twist) = (w.damping, w.ang_damping, w.twist);
        }
    }
}

impl<R: Moto, G: FnMut(Vf, &mut u8) -> (Vf, Vf), S: Sweep> Steps<R, G, S> {
    /// What [`Moto::call`] answers.
    fn answer(&mut self, c: Call, q: &mut Quad, body: &mut Body) {
        match c {
            Call::InputLimiter(dt) => self.input_limiter(q, dt),
            Call::Clutch(dt) => {
                let b = &mut self.bike;
                let s = crate::r#move::Step { dt, gas: q.gas, clutch: b.clutch, speed: F(q.contact_vel.z()), ..Default::default() };
                crate::handling::step::clutch(&b.clutch_keys, &mut b.ride, &mut b.engine, &s);
            }
            Call::Preload(dt) => {
                let (keys, mut air, mut ch, st) = self.air_in(q, body, dt);
                let posted = crate::handling::step::preload(&keys, &mut air, &mut ch, &st, &mut self.bike.preload_cue);
                self.air_out(q, body, air, ch);
                if posted {
                    self.rest.call(Call::PreloadCue, q, body);
                }
            }
            Call::Backflip(dt) => self.backflip(q, body, dt),
            Call::AirborneControl(dt) => {
                let (keys, mut air, mut ch, st) = self.air_in(q, body, dt);
                let b = &self.bike;
                let inp = crate::handling::AirIn { stunt_id: b.rider_stunt, gas: q.gas_limited, gyro: b.gyro };
                crate::handling::airborne_control(&keys, b.gas_gyro_torque, &mut air, &mut ch, &st, &inp);
                self.air_out(q, body, air, ch);
            }
            Call::Suspension(i, _) => {
                self.wheel_in(q, i);
                let mut wb = self.wheel_body(body);
                let Steps { bike, ground, .. } = self;
                crate::atv_wheel::suspension(&mut bike.wheels[i], &mut bike.more[i], &wb, ground);
                self.wheel_out(q, body, &mut wb, i);
            }
            Call::TireForces(i, dt) => {
                self.wheel_in(q, i);
                let mut wb = self.wheel_body(body);
                let Bike { wheels, more, ground, .. } = &mut self.bike;
                let under = &mut ground[i];
                crate::atv_wheel::tire_forces(&mut wheels[i], &mut wb, dt,
                    &mut |c, w, b, x| crate::atv_wheel::run(c, w, b, x, under), &mut more[i]);
                self.wheel_out(q, body, &mut wb, i);
            }
            Call::WheelIntegrate(i, dt) => {
                self.call(Call::SurfaceDrag(i, dt), q, body);
                self.call(Call::SpinIntegrate(i, dt), q, body);
                self.wheel_in(q, i);
                let calls = crate::handling::step::wheel_integrate(&mut self.bike.wheels[i], dt);
                debug_assert_eq!(calls, [crate::wheel::Call::SurfaceDrag(dt), crate::wheel::Call::SpinIntegrate(dt)]);
                let mut wb = self.wheel_body(body);
                self.wheel_out(q, body, &mut wb, i);
            }
            Call::SurfaceDrag(i, _) => {
                self.wheel_in(q, i);
                self.spin_in(q, i);
                let mut wb = self.wheel_body(body);
                let b = &mut self.bike;
                crate::atv_wheel::surface_drag(&b.wheels[i], &b.more[i], &mut b.spins[i], &mut wb);
                self.spin_out(q, i);
                self.wheel_out(q, body, &mut wb, i);
            }
            Call::SpinIntegrate(i, dt) => {
                self.wheel_in(q, i);
                self.spin_in(q, i);
                let mut wb = self.wheel_body(body);
                let b = &mut self.bike;
                let j = PARTNER[i];
                let other = crate::atv_wheel::Other {
                    contact: q.wheels[j].touching,
                    spring_force: b.wheels[j].spring_force,
                    ground: b.ground[j],
                };
                let Bike { wheels, more, spins, ground, .. } = b;
                crate::atv_wheel::spin_integrate(&mut wheels[i], &mut more[i], &mut spins[i], &mut wb, dt, &other,
                    &mut ground[i], curve());
                self.spin_out(q, i);
                self.wheel_out(q, body, &mut wb, i);
            }
            Call::AutoShift { inv_radius, .. } => {
                let b = &mut self.bike;
                crate::atv_moto::auto_shift(&b.clutch_keys, &mut b.engine, q, inv_radius);
            }
            Call::HeadTail => {
                let b = &mut self.bike;
                crate::atv_moto::head_tail(&b.clutch_keys, &mut b.parts, &mut b.cofg, &b.ride, b.stick.y, q, body);
            }
            Call::Gravity => {
                if crate::atv_moto::gravity(&mut self.bike.parts, q, body) {
                    // MD75e1: the record's resolution on the whole body, the listener's view in and out.
                    let cofg = self.bike.cofg;
                    let resolved = self.bike.contacts.as_mut().is_some_and(|h| {
                        let Some(whole) = h.body.as_mut() else { return false };
                        whole.set_view(body);
                        h.resolve(cofg)
                    });
                    match self.bike.contacts.as_ref().and_then(|h| h.body.as_ref()) {
                        Some(whole) if resolved => *body = whole.view(),
                        _ => self.rest.call(Call::GravityRecord, q, body),
                    }
                }
            }
            Call::EngineUpdate(dt) => {
                let b = &mut self.bike;
                let radius = b.wheels.map(|w| w.probe_length);
                crate::atv_moto::engine_update(&mut b.engine, &mut b.parts.drive, radius, b.parts.driven, q.gas_limited, q.clutch_in, dt);
            }
            Call::AirDrag(_) => crate::atv_moto::air_drag(self.bike.clutch_keys.drag, q, body),
            Call::WheelClear(i) => {
                q.wheels[i].touching = false;
                let Bike { wheels, more, ground, .. } = &mut self.bike;
                crate::atv_moto::wheel_clear(&mut wheels[i], &mut more[i], &mut ground[i]);
            }
            Call::Contacts => {
                // The search's vt +0xc8: the probe's node's world frame
                // (`atv_node_frame`, the node brought up to date first), the probe stood there.
                let mut scene = self.bike.parts.scene;
                scene.update(0);
                let Some((look, cs)) = self.sweep.sweep(&scene.nodes[0].world) else {
                    return self.rest.call(c, q, body);
                };
                self.bike.parts.scene = scene;
                // MD75e: the probe is the quad's trigger, stood at the frame it was swept from.
                let trigger = &mut self.bike.parts.collidables.states[crate::atv_sync::slot(Part::Trigger)];
                crate::atv_sync::stand(trigger, &scene.nodes[0].world);
                if self.sweep.two_wheels() {
                    self.bike_contacts(q, look, &cs);
                } else {
                    self.contacts(q, look, &cs);
                }
            }
            Call::RevLimit => crate::atv_moto::rev_limit(&self.bike.engine, &mut self.bike.parts.drive, self.bike.parts.driven),
            Call::PeakRates => crate::atv_moto::peak_rates(q, body),
            Call::SpinAir(dt) => {
                let (keys, ..) = self.air_in(q, body, dt);
                let b = &mut self.bike;
                let loads = crate::air::Loads { side: F(q.attitude.x()), fore_aft: F(q.attitude.z()), ..Default::default() };
                let steep = crate::handling::step::flip_wreck_steep(&keys, b.struck_class, q.airborne, &loads);
                if let Some(c) = crate::atv_moto::spin_air(b, q, body, dt, steep) {
                    self.rest.call(c, q, body);
                }
            }
            Call::VehiclePost(_) => {
                let b = &mut self.bike;
                let Some(inside) = crate::atv_moto::vehicle_post(&mut b.parts, body, b.cofg) else { return };
                if !inside {
                    self.rest.call(Call::BodyReset, q, body);
                    self.rest.call(Call::Respawn, q, body);
                }
                self.call(Call::SpatialUpdate, q, body);
                if self.bike.parts.flags & 0x02 != 0 {
                    self.call(Call::NodeBounds, q, body);
                }
                self.call(Call::CollisionSync, q, body);
                self.bike.parts.flags &= !0x04;
            }
            Call::SpatialUpdate => {
                if self.bike.parts.spatial {
                    let (centre, half) = self.node_box();
                    self.rest.call(Call::SpatialMove { node: false, centre, half, reach: F::ZERO }, q, body);
                }
            }
            Call::NodeBounds => {
                if self.bike.parts.scene.nodes[0].spatial {
                    let (centre, half) = self.node_box();
                    let (centre, half, reach) = self.bike.parts.scene.database_box(0, centre, half);
                    self.rest.call(Call::SpatialMove { node: true, centre, half, reach }, q, body);
                }
            }
            Call::CollisionSync => {
                for c in crate::atv_moto::collision_sync(&self.bike.parts.collidables) {
                    self.call(c, q, body);
                }
            }
            Call::PartSync(p) => {
                // vt +0xc8: the frame of the part's node (the quad's) in the world, the node brought up
                // to date first (`atv_node_frame`); then the stand, and the box into the database.
                let x = &mut self.bike.parts;
                x.scene.update(0);
                let frame = x.scene.nodes[0].world;
                let part = &mut x.collidables.states[crate::atv_sync::slot(p)];
                if let Some((centre, half, reach)) = crate::atv_sync::part_sync(part, &frame) {
                    self.rest.call(Call::PartMove { part: p, centre, half, reach }, q, body);
                }
            }
            Call::Gather(p) => {
                let part = &self.bike.parts.collidables.states[crate::atv_sync::slot(p)];
                if !self.sweep.gather(part.centre, part.reach) {
                    self.rest.call(c, q, body);
                }
            }
            Call::NodePosition => q.node_pos = self.bike.parts.scene.position(0),
            // MD75c3: the node's vt +0x94, the quad's node's being `atv_node_rebound`.
            Call::NodeRebound => self.bike.parts.scene.rebound(0),
            c => self.rest.call(c, q, body),
        }
    }
}

impl<R: Moto, G, S> Steps<R, G, S> {
    /// **The contacts spread onto the wheels** (MD86, [`wheel_contacts`] after the sweep): each
    /// wheel's swept contact (`+0xd4`), point and normal in, and out again with the last rear
    /// contact's collidable ([`crate::atv_moto::Parts::rear_node`]).
    fn contacts(&mut self, q: &mut Quad, look: bool, cs: &[Contact]) {
        let b = &mut self.bike;
        let mut wc: [crate::chassis::WheelContact; 4] = std::array::from_fn(|i| crate::chassis::WheelContact {
            point: b.wheels[i].contact_point,
            normal: q.wheels[i].normal,
            touching: b.wheels[i].swept_contact,
            ..Default::default()
        });
        let calls = wheel_contacts(&mut wc, &mut b.parts.rear_node, look, cs);
        debug_assert!(!calls.iter().any(|c| matches!(c, crate::chassis::ContactCall::Notify(..))), "a swept contact with an owner");
        for (i, c) in wc.iter().enumerate() {
            debug_assert_eq!((c.owner, c.vehicle), (0, 0), "a swept contact with an owner or a vehicle");
            let w = &mut b.wheels[i];
            (w.contact_point, w.contact_normal, w.swept_contact) = (c.point, c.normal, c.touching);
            q.wheels[i].normal = c.normal;
        }
    }

    /// **The contacts spread onto the bike's two wheels** (F52, [`crate::handling::bike::wheel_contacts`]
    /// after the sweep): as [`Steps::contacts`], on the front and rear alone. The bike's wheels keep
    /// no owner or vehicle here (as on the quad), nor anyone to tell.
    fn bike_contacts(&mut self, q: &mut Quad, look: bool, cs: &[Contact]) {
        use crate::handling::bike::{FRONT, REAR};
        let b = &mut self.bike;
        let mut wc = [FRONT, REAR].map(|i| crate::chassis::WheelContact {
            point: b.wheels[i].contact_point,
            normal: q.wheels[i].normal,
            touching: b.wheels[i].swept_contact,
            ..Default::default()
        });
        let [front, rear] = &mut wc;
        crate::handling::bike::wheel_contacts(front, rear, &mut b.parts.rear_node, look, cs, &mut |owner, i| {
            debug_assert!(false, "a swept contact with an owner {owner:#x} telling its listener {i}");
        });
        for (i, c) in [FRONT, REAR].into_iter().zip(&wc) {
            debug_assert_eq!((c.owner, c.vehicle), (0, 0), "a swept contact with an owner or a vehicle");
            let w = &mut b.wheels[i];
            (w.contact_point, w.contact_normal, w.swept_contact) = (c.point, c.normal, c.touching);
            q.wheels[i].normal = c.normal;
        }
    }

    /// The quad's node's box in the world (MD75c): its frame brought up to date, its bounds gathered
    /// again if they are stale (MD75c3: run here, [`crate::atv_moto::Scene::rebound`]), then
    /// [`crate::atv_moto::Scene::world_box`].
    fn node_box(&mut self) -> (Vf, Vf) {
        if self.bike.parts.scene.bounds_stale(0) {
            self.bike.parts.scene.rebound(0);
            self.bike.parts.scene.rebounded(0);
        }
        self.bike.parts.scene.world_box(0)
    }

    /// The input limiter on the quad's levers: the controls as the pad left them (the gas and brake
    /// levers, the clutch lever, the balance stunt as the gas's zero gate) in, the levers it leaves
    /// and the rate limiter's out.
    fn input_limiter(&mut self, q: &mut Quad, dt: F) {
        let b = &mut self.bike;
        let h = &mut b.held;
        (h.gas, h.front_brake, h.rear_brake, h.clutch) = (q.gas_limited, q.front_brake, q.rear_brake, q.clutch_in);
        (h.lean, h.reverse) = ((q.lean, q.lean_fore_aft), q.push_gate);
        let c = crate::ride::Controls {
            gas: q.gas,
            brake: q.brake,
            clutch: b.clutch,
            hands_off: q.balancing,
            gas_cut: b.gate_cut,
            ..Default::default()
        };
        let inp = crate::handling::step::LimiterIn { airborne: q.airborne, cut_time: b.cut_time };
        let c = crate::handling::step::input_limiter(h, &mut b.gas_cut, &c, b.stick, &b.joystick, dt, &inp);
        (q.gas, q.brake, b.clutch) = (c.gas, c.brake, c.clutch);
        (q.gas_limited, q.front_brake, q.rear_brake, q.clutch_in) = (h.gas, h.front_brake, h.rear_brake, h.clutch);
        (q.lean, q.lean_fore_aft, q.push_gate) = (h.lean.0, h.lean.1, h.reverse);
    }

    /// The air steps' keys, state, chassis and step off the quad.
    fn air_in(&self, q: &Quad, body: &Body, dt: F) -> (crate::air::Params, crate::air::Air, crate::air::Chassis, crate::air::Step) {
        let b = &self.bike;
        let mut keys = b.air_keys;
        keys.control_authority_time = q.authority_time;
        let mut air = b.air;
        (air.on_ground, air.contact_vel, air.crossup, air.authority, air.flipping) =
            (q.on_ground, q.contact_vel, q.crossup, q.authority, q.flipping);
        let ch = crate::air::Chassis {
            speed: q.speed_len,
            pitch: F(q.euler.x()),
            roll: F(q.euler.z()),
            force: body.force,
            moment: body.moment,
            vel: body.velocity,
            local_vel: q.local_vel,
            ang_vel: body.ang_vel,
        };
        let st = crate::air::Step {
            dt,
            heading: q.heading,
            up: q.up,
            stick: (b.stick.x, b.stick.y),
            stick_hold: b.stick_hold,
            dpad_y: b.stick.digital[1],
            brake: q.brake,
            front_brake: q.front_brake,
            rear_brake: q.rear_brake,
            lean: q.lean,
            touching: b.touching,
            spinning: b.spin_applied,
            spin_active: b.spin_active,
            spin_ramp: q.spinning,
            ..Default::default()
        };
        (keys, air, ch, st)
    }

    fn air_out(&mut self, q: &mut Quad, body: &mut Body, air: crate::air::Air, ch: crate::air::Chassis) {
        (q.on_ground, q.contact_vel, q.crossup, q.authority, q.flipping) =
            (air.on_ground, air.contact_vel, air.crossup, air.authority, air.flipping);
        self.bike.air = air;
        (body.force, body.moment) = (ch.force, ch.moment);
    }

    /// The backflip, its mid-clip wreck's reason asked of the ported test on the peak-rates vector
    /// ([`Quad::attitude`], the g loads it reads: `x` sideways, `z` fore and aft).
    fn backflip(&mut self, q: &mut Quad, body: &mut Body, dt: F) {
        let (keys, mut air, mut ch, st) = self.air_in(q, body, dt);
        let b = &mut self.bike;
        let inp = crate::handling::FlipIn { buttons: b.buttons, right_y: b.right_y, up_y: b.up_y, stunt_id: b.stunt_id };
        let loads = crate::air::Loads { side: F(q.attitude.x()), fore_aft: F(q.attitude.z()), ..Default::default() };
        let steep = crate::handling::step::flip_wreck_steep(&keys, b.struck_class, q.airborne, &loads);
        let mut started = false;
        let out = crate::handling::backflip(&keys, &mut air, &mut ch, &st, &inp, &mut b.flip_clip, || {
            started = true;
            true
        }, || steep);
        if out.stunt_cleared {
            b.stunt_id = 255;
        }
        let stunt = b.flip_stunt;
        self.air_out(q, body, air, ch);
        if started {
            self.rest.call(Call::StartStunt(stunt), q, body);
        }
        if out.stunt_ended {
            self.rest.call(Call::EndStunt, q, body);
        }
        if let Some(reason) = out.wreck {
            self.rest.call(Call::Wreck(reason), q, body);
        }
    }

    /// Wheel `i`'s fields the quad keeps, into the wheel step's wheel.
    fn wheel_in(&mut self, q: &Quad, i: usize) {
        let (w, x, qw) = (&mut self.bike.wheels[i], &mut self.bike.more[i], &q.wheels[i]);
        (w.steered, w.balancing, w.contact, w.contact_normal) = (qw.steered, qw.balancing, qw.touching, qw.normal);
        (w.max_steer_angle, w.steer_angle, w.spring_rate) = (qw.max_steer_angle, qw.steer_command, qw.spring);
        x.chassis_velocity_local = qw.contact_vel;
    }

    /// The wheel integrate's words [`Bike::spins`] shares with the drive and the quad's wheel, and
    /// its brake lever.
    fn spin_in(&mut self, q: &Quad, i: usize) {
        let (s, d, qw) = (&mut self.bike.spins[i], &self.bike.parts.drive[i], &q.wheels[i]);
        (s.spin, s.torque, s.slip.spinning) = (d.spin, d.torque, d.from_torque);
        s.local_force.0[2] = d.forward.0;
        (s.rear_pair, s.donut, s.inv_radius) = (i32::from(qw.rear_pair), qw.donut, qw.inv_radius);
        s.brake = Some(if i < 2 { q.front_brake } else { q.rear_brake });
    }

    fn spin_out(&mut self, q: &mut Quad, i: usize) {
        let (s, d, qw) = (&self.bike.spins[i], &mut self.bike.parts.drive[i], &mut q.wheels[i]);
        (d.spin, d.torque, d.from_torque, d.forward) = (s.spin, s.torque, s.slip.spinning, F(s.local_force.z()));
        (qw.rear_pair, qw.donut, qw.inv_radius) = (s.rear_pair != 0, s.donut, s.inv_radius);
    }

    fn wheel_body(&self, body: &Body) -> crate::atv_wheel::Body {
        crate::atv_wheel::Body {
            position: body.position,
            velocity: body.velocity,
            force: body.force,
            moment: body.moment,
            correction: body.correction,
            orientation: body.orientation,
            cofg: self.bike.cofg,
            spin: body.ang_vel,
        }
    }

    fn wheel_out(&mut self, q: &mut Quad, body: &mut Body, wb: &mut crate::atv_wheel::Body, i: usize) {
        let (w, x, qw) = (&self.bike.wheels[i], &self.bike.more[i], &mut q.wheels[i]);
        (qw.steered, qw.balancing, qw.touching, qw.normal) = (w.steered, w.balancing, w.contact, w.contact_normal);
        (qw.max_steer_angle, qw.steer_command, qw.spring) = (w.max_steer_angle, w.steer_angle, w.spring_rate);
        qw.contact_vel = x.chassis_velocity_local;
        (body.position, body.force, body.moment, body.correction) = (wb.position, wb.force, wb.moment, wb.correction);
    }
}

// --- MD65a: the wreck body ----------------------------------------------------------------------------

/// The file the quad's wreck rays load from, on its scene node.
pub const WRECK_FILE: &str = "WreckATVm.col";
/// The category the wreck rays are in, and the one the trigger probe's category list holds.
pub const PHYSICS_BODY: &str = "PHYSICSBODY";
pub const TRIGGER: &str = "TRIGGER";

/// The quad's collidables the wreck body is about: the three the vehicle's own init made (the riding
/// rays, a ray probe off the bike's `m.col`; the frame mesh; the sweep) and the two the quad's init
/// adds (a ray probe with no file in the trigger category, and the wreck rays).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Riding,
    Frame,
    Sweep,
    Trigger,
    Wreck,
}

/// A collidable's list as the original keeps it: a mode word and its entries, a 0 entry an empty slot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct List {
    pub mode: u32,
    pub list: Vec<u32>,
}

impl List {
    /// Another collidable added to an object list: nothing for none (0); nothing if it is listed;
    /// else into the first empty slot, else on the end.
    pub fn add_object(&mut self, handle: u32) {
        if handle != 0 {
            self.add(handle);
        }
    }

    /// A category word added to a category list: as [`List::add_object`], but a 0 is added too (it
    /// fills no slot, so it is listed once, on the end, if no slot is empty).
    pub fn add_category(&mut self, word: u32) {
        self.add(word);
    }

    fn add(&mut self, x: u32) {
        if self.list.contains(&x) {
            return;
        }
        match self.list.iter_mut().find(|e| **e == 0) {
            Some(slot) => *slot = x,
            None => self.list.push(x),
        }
    }
}

/// One of the quad's collidables: its handle (what the others' lists name it by, never 0) and its
/// object and category lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Col {
    pub handle: u32,
    pub objects: List,
    pub categories: List,
}

impl Col {
    /// A part as its constructor leaves it: every list empty, and in mode 1 (ignore what is listed).
    pub fn made(handle: u32) -> Col {
        let empty = List { mode: 1, list: Vec::new() };
        Col { handle, objects: empty.clone(), categories: empty }
    }
}

/// The quad's collidables as the wreck body's block reads and leaves them. The field map is
/// research/oracle.txt's `# MD65a`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WreckBody {
    pub riding: Col,
    pub frame: Col,
    pub sweep: Col,
    pub trigger: Col,
    pub wreck: Col,
    /// The wreck rays' own category word (looked up as [`PHYSICS_BODY`]), and whether they were given
    /// the quad as their owner and the quad's collision record as theirs: the wreck rays' hits go into
    /// the record the riding rays' do.
    pub wreck_category: u32,
    pub wreck_owned: bool,
    pub wreck_record: bool,
    /// Whether the trigger probe was given the quad as its owner.
    pub trigger_owned: bool,
    /// The riding rays' handle, kept for the swap back ([`swap_rays`]).
    pub riding_kept: u32,
}

/// The flags words the two parts are made with (both leave the flag byte's bit 1 set and the rest
/// as they were).
const TRIGGER_FLAGS: u32 = 2;
const WRECK_FLAGS: u32 = 3;

/// **The quad's init, its last collision block**: the two ray-probe parts made, and every collidable
/// of the quad told to ignore them, and them the others.
///
/// The trigger probe (`trigger`, the handle its allocation gave) is made, flagged, given the quad and
/// hung in the scene tree; the riding rays are kept. The wreck rays (`wreck`) are made, flagged,
/// loaded from [`WRECK_FILE`] on the scene node and readied, put in [`PHYSICS_BODY`]'s category,
/// given the quad and its collision record, and hung. Then the riding rays and the sweep ignore
/// both; the wreck rays (object mode 1) ignore the trigger probe, the riding rays, the frame and the
/// sweep; the trigger probe (object mode 1) the riding rays, the frame, the sweep and the wreck rays,
/// and (category mode 1) [`TRIGGER`]'s category and the quad's own word `actor`. The frame mesh's
/// list is left alone. `category` answers a name with its category word, in the order asked.
#[allow(clippy::too_many_arguments)] // the original's inputs, each one a word it reads
pub fn build_wreck_body(
    w: &mut WreckBody,
    trigger: u32,
    wreck: u32,
    actor: u32,
    mut category: impl FnMut(&str) -> u32,
    q: &mut Quad,
    body: &mut Body,
    m: &mut impl Moto,
) {
    m.call(Call::PartMade(Part::Trigger), q, body);
    w.trigger = Col::made(trigger);
    m.call(Call::PartFlags(Part::Trigger, TRIGGER_FLAGS), q, body);
    w.trigger_owned = true;
    m.call(Call::PartHung(Part::Trigger), q, body);
    w.riding_kept = w.riding.handle;
    m.call(Call::PartMade(Part::Wreck), q, body);
    w.wreck = Col::made(wreck);
    m.call(Call::PartFlags(Part::Wreck, WRECK_FLAGS), q, body);
    m.call(Call::PartLoad(Part::Wreck), q, body);
    m.call(Call::PartReady(Part::Wreck), q, body);
    w.wreck_category = category(PHYSICS_BODY);
    w.wreck_owned = true;
    w.wreck_record = true;
    m.call(Call::PartHung(Part::Wreck), q, body);

    let (t, k) = (w.trigger.handle, w.wreck.handle);
    let (r, f, s) = (w.riding.handle, w.frame.handle, w.sweep.handle);
    w.riding.objects.add_object(t);
    w.riding.objects.add_object(k);
    w.sweep.objects.add_object(t);
    w.sweep.objects.add_object(k);
    w.wreck.objects.mode = 1;
    for h in [t, r, f, s] {
        w.wreck.objects.add_object(h);
    }
    w.trigger.objects.mode = 1;
    for h in [r, f, s, k] {
        w.trigger.objects.add_object(h);
    }
    w.trigger.categories.mode = 1;
    let trigger_word = category(TRIGGER);
    w.trigger.categories.add_category(trigger_word);
    w.trigger.categories.add_category(actor);
}

/// Which rays the quad's collision runs on, as the swap leaves it: the handle in the quad's two
/// active-rays words and in its collision record's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rays {
    pub active: u32,
    pub active_too: u32,
    pub record: u32,
    /// The two candidates: the wreck rays and the riding rays kept ([`WreckBody::riding_kept`]).
    pub wreck: u32,
    pub riding: u32,
}

/// **The swap between the riding and the wreck rays**: the wreck entry's (`wrecked`) and the unwreck's
/// (not), the bike's two shared virtuals, on the quad's two parts.
///
/// The one swapped to goes into the quad's two active-rays words and the collision record's; the
/// other is hidden from the scene and taken off its collision world. The one swapped to is then
/// shown and put in the quad's world, unless it is in a world already (`in_world`).
pub fn swap_rays(r: &mut Rays, wrecked: bool, in_world: bool, q: &mut Quad, body: &mut Body, m: &mut impl Moto) {
    let (to, to_part, from_part) =
        if wrecked { (r.wreck, Part::Wreck, Part::Riding) } else { (r.riding, Part::Riding, Part::Wreck) };
    r.active = to;
    r.active_too = to;
    r.record = to;
    m.call(Call::PartHide(from_part), q, body);
    m.call(Call::PartOffWorld(from_part), q, body);
    if !in_world {
        m.call(Call::PartShow(to_part), q, body);
        m.call(Call::PartInWorld(to_part), q, body);
    }
}

// --- MD65b: the quad's small slots --------------------------------------------------------------------

/// **The quad's level setup, its surface remap**: the bike's shared level setup ([`Call::LevelSetup`]),
/// then each of the sixteen surfaces the level maps (`map[i]` not negative) takes the tire tables of
/// surface `map[i]`, on all four wheels, in order: surface 0 first, and per surface the left front,
/// right front, left rear, right rear. A copy is made only with both surfaces under sixteen, and
/// it copies every table but the last side one (`DonutsSideForceSpeedScale`), which each surface
/// keeps. Being in place, a later surface mapped to one already remapped takes the remapped tables.
pub fn remap_surfaces(
    t: &mut TireSetup,
    map: &[i32; crate::phy::MAX_SURFACES],
    q: &mut Quad,
    body: &mut Body,
    m: &mut impl Moto,
) {
    m.call(Call::LevelSetup, q, body);
    for (i, &src) in map.iter().enumerate() {
        if src < 0 {
            continue;
        }
        for w in &mut t.wheels {
            copy_surface(w, i, src);
        }
    }
}

/// The wheel's surface copy: `dst` takes `src`'s tables but the donut scale, both under sixteen.
fn copy_surface(w: &mut WheelTire, dst: usize, src: i32) {
    let n = crate::phy::MAX_SURFACES;
    if src < 0 || src as usize >= n || dst >= n {
        return;
    }
    let from = w.surfaces[src as usize];
    let to = &mut w.surfaces[dst];
    to.drive = from.drive;
    to.side[..12].copy_from_slice(&from.side[..12]);
    to.drag = from.drag;
}

/// What the quad answers for the surface it is on: the left rear wheel's, as the ground query left it
/// ([`WheelGround::surface`]).
pub fn surface_under(wheels: &[WheelGround; 4]) -> u8 {
    wheels[2].surface
}

/// What the quad answers for its strut travel: the left front wheel's.
pub fn strut_travel(t: &TireSetup) -> F {
    t.wheels[0].strut_compression
}

/// **The trigger probe's segments**: with a trigger probe made ([`build_wreck_body`]), one segment per
/// wheel (left front, right front, left rear, right rear), from the mount raised by the strut travel
/// down to the mount lowered by the radius (`y` only; `x`, `z` and `w` are the mount's). The original
/// hands the eight points to the probe's shape on the quad's scene node; with none it does nothing
/// (`None`).
pub fn trigger_segments(t: &TireSetup, has_trigger: bool) -> Option<[[Vf; 2]; 4]> {
    if !has_trigger {
        return None;
    }
    Some(t.wheels.map(|w| {
        let top = F(fpu::add(w.mount.y(), w.strut_compression.0));
        let bottom = F(fpu::sub(w.mount.y(), w.radius.0));
        [w.mount.merge(Dest::Y, Vf::splat(top.0)), w.mount.merge(Dest::Y, Vf::splat(bottom.0))]
    }))
}

/// One wheel as the quad's ground settle reads and leaves it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelGround {
    /// Where the wheel is mounted, and the offset from it the settle probes at (body frame).
    pub mount: Vf,
    pub probe: Vf,
    /// The probe point in the world, as the ground query leaves it, the ground's normal there and
    /// the surface it is; and the point kept once all four are settled.
    pub point: Vf,
    pub normal: Vf,
    pub surface: u8,
    pub kept: Vf,
}

/// **The quad's ground settle**: per wheel (left front, right front, left rear, right rear), the mount
/// plus the probe (`xyz`; `w` the mount's) turned into the world by the body's orientation and moved
/// to the quad's scene node (`node_pos`, `xyz`), and the ground queried there: `ground` answers the
/// point (in place), the normal and the surface, one wheel at a time, in order. Then each wheel keeps
/// its point.
pub fn settle_wheels(wheels: &mut [WheelGround; 4], body: &Body, node_pos: Vf, mut ground: impl FnMut(&mut WheelGround)) {
    for w in wheels.iter_mut() {
        let p = add_xyz(w.mount, w.probe);
        w.point = add_xyz(body.to_world(p), node_pos);
        ground(w);
    }
    for w in wheels.iter_mut() {
        w.kept = w.point;
    }
}

// --- MD65c: the animation driver ------------------------------------------------------------------

/// The stunt id the riding pose is played for: ids below it are tricks with a clip of their own each;
/// this one blends the riding pose's three clips ([`POSE_CLIPS`]); any other plays the middle one.
pub const RIDING_POSE: i32 = 46;
/// The riding pose's three clips, in [`Anim`]'s clip table after the 46 stunts': the blend's weight
/// is the crossup's, and the stick fore and aft mixes the first two (its forward half) or the last two.
pub const POSE_CLIPS: [i32; 3] = [46, 47, 48];

/// What the animation driver reads and writes beyond [`Quad`] and [`PutBack`]. The field map is
/// research/oracle.txt's `# MD65b`/`# MD65c`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anim {
    /// The vehicle state's word: in 3 and 4 the two pose blends are left as they were.
    pub vehicle_state: u32,
    /// The clips' fade-in, run down by `fade_rate` a second and held in `0..1`; one when the rider's
    /// animation is stopped.
    pub fade: F,
    pub fade_rate: F,
    /// The rider's stunt id as this frame plays it ([`Quad::balancing`]'s word) and as the stunt
    /// steps last set it; and the balance stunt's id.
    pub stunt: i32,
    pub next_stunt: i32,
    pub balance_stunt: i32,
    /// A word the unwreck clears; set, the rider's animation is stopped (what sets it was not traced).
    pub restart: u32,
    /// How far into its clip the stunt is (the stunt clip's weight is this over the clip's length).
    pub stunt_time: F,
    /// The crossup either way that the riding pose's blend spans, and the two blends the pose is
    /// played at: the crossup's and the stick fore and aft's.
    pub crossup_range: F,
    pub crossup_blend: F,
    pub fore_aft_blend: F,
}

/// What the base driver's net interpolation (vt `+0x3e8`) reads: how many snapshots have come
/// (`+0xd04`), the network clock's time (its `+0x4cc`), the quad's lag behind it (`+0xd10`), and the
/// times (word 0) of the five snapshots the slots `+0xcd4`, `+0xcdc` .. `+0xcf4` (8 apart) point at,
/// newest first.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NetClock {
    pub snapshots: u32,
    pub now: F,
    pub delay: F,
    pub times: [F; 5],
}

/// The interpolation's own lag, on top of [`NetClock::delay`], in seconds.
const NET_LAG: f32 = 0.1;
/// How far the interpolation may run before the older snapshot or past the newer one.
const NET_T_MIN: f32 = -0.9;
const NET_T_MAX: f32 = 1.9;

/// **The biplane's base animation driver** (`atv_biplane_animate`), as the quad's [`animate`] runs it
/// first: only in vehicle states 3 and 4, the world angular velocity is zeroed (`(0,0,0,1)`), then the
/// net interpolation (vt `+0x3e8`) runs and, as it always answers yes, the net unpack
/// ([`Call::NetUnpack`]).
///
/// The interpolation does nothing with fewer than five snapshots. Otherwise it wants the time
/// `now − delay − 0.1` and walks the snapshots from the newest while that is before the older one's
/// time, at most four steps; the original's walk reads the second slot twice, so a stop on its second
/// step never happens and the pair is (1, 0), (2, 1), (3, 2) or (4, 3). It hands the pair and `t` =
/// `(want − older) / (newer − older)`, clamped to `-0.9..1.9`, to [`Call::NetInterpolate`]
/// ([`net_interpolate`] is its port).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn base_animate(vehicle_state: u32, net: &NetClock, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    if vehicle_state.wrapping_sub(3) >= 2 {
        return;
    }
    q.world_ang_vel = NOTHING;
    if net.snapshots >= 5 {
        let want = net.now - net.delay - F(NET_LAG);
        let (mut older, mut newer) = (1, 0);
        for i in 0..4 {
            if !(want < net.times[older]) {
                break;
            }
            newer = older;
            older = i + 1;
        }
        let t = (want - net.times[older]) / (net.times[newer] - net.times[older]);
        m.call(Call::NetInterpolate { older, newer, t: t.max(F(NET_T_MIN)).min(F(NET_T_MAX)) }, q, body);
    }
    m.call(Call::NetUnpack(dt), q, body);
}

/// A network snapshot as [`net_interpolate`] reads one: its time (word 0), where the quad was, its
/// velocity and orientation (`+0x10`, `+0x20`, `+0x30`), the two pose blends (`+0x40` crossup,
/// `+0x44` fore and aft), the wreck word (`+0x48`), the stunt id (`+0x4c`) and a word that goes to the
/// quad's `+0x940` (`+0x50`, not traced).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub time: F,
    pub position: Vf,
    pub velocity: Vf,
    pub orientation: Vf,
    pub crossup_blend: F,
    pub fore_aft_blend: F,
    pub wreck: u32,
    pub stunt: i32,
    pub word: u32,
}

/// A lag filter as the net code keeps them: its value, its time constant and the fraction it last
/// moved by.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lag {
    pub value: F,
    pub time: F,
    pub fraction: F,
}

impl Lag {
    /// The value moved `min(dt, time) / time` of the way to `target` (`adda(0, value)`, `madd`).
    pub fn run(&mut self, dt: F, target: F) -> F {
        self.fraction = dt.min(self.time) / self.time;
        self.value = Acc::adda(F::ZERO, self.value).madd(self.fraction, target - self.value);
        self.value
    }
}

/// What the net interpolation keeps beyond [`Remote`] and [`Anim`]. The field map is
/// research/oracle.txt's `# MD65c2`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NetSmooth {
    /// The position's three lags (`*(+0xd14)`) and the velocity's (`*(+0xd18)`, only ever set, never
    /// run).
    pub position: [Lag; 3],
    pub velocity: [Lag; 3],
    /// The two pose blends' lags (`*(+0x1790)` crossup, `*(+0x178c)` fore and aft).
    pub crossup: Lag,
    pub fore_aft: Lag,
    /// Where a far snapshot puts the quad back (`+0xd40`).
    pub put_back_at: Vf,
    /// What the snapshot's velocity changed by (its fourth quadword; `w` the newer's velocity's).
    pub velocity_change: Vf,
    /// The newer snapshot's wreck word and its untraced word (snapshot pointers 6 and 8: `+0xc80` and
    /// `+0x940` on the spawned quad).
    pub wreck: u32,
    pub word: u32,
}

/// How far inside the world's area the interpolation holds the quad.
const NET_EDGE: f32 = 25.0;
/// Past this the lags are set to the snapshot, past [`NET_FAR`] the quad is put back there too.
const NET_NEAR: f32 = 8.0;
const NET_FAR: f32 = 16.0;

/// The quad's lerp: the older's at or before `t` 0, the newer's from 1, else `adda(0, a) madd(t, b − a)`.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn net_lerp(a: F, b: F, t: F) -> F {
    if t <= F::ZERO {
        a
    } else if !(t < F::ONE) {
        b
    } else {
        Acc::adda(F::ZERO, a).madd(t, b - a)
    }
}

/// `a + t (b − a)` over `xyz` on VU0 (`vsub`, `vmulx`, `vadd`), `w` the newer's.
fn net_lerp_xyz(a: Vf, b: Vf, t: F) -> Vf {
    let d = sub_xyz(b, a);
    let d = d.merge(Dest::XYZ, d.mul(Vf::splat(t.0)));
    d.merge(Dest::XYZ, d.add(a))
}

/// **The quad's net interpolation** (vt `+0x3ec`, `atv_quad_net_interpolate`): a remote quad put
/// between two network snapshots, `t` of the way from the older to the newer (`t` in `-0.9..1.9`, as
/// [`base_animate`] hands it; `dt` the frame's, which the original carries through in `f12`).
///
/// The quad's own part: the two pose blends, each the older's at `t ≤ 0`, the newer's at `t ≥ 1`,
/// else lerped, run through their lags into [`Anim::crossup_blend`] and [`Anim::fore_aft_blend`]; the
/// newer's stunt id into [`Anim::next_stunt`], its wreck word and untraced word into [`NetSmooth`].
///
/// Then the biplane's (`atv_biplane_net_interpolate`, which the quad's calls last): position and
/// velocity lerped unclamped in `t`, the orientation slerped the short way ([`crate::math::slerp`]),
/// and the position's `x` and `z` held 25 ft inside the world's `area` (`[x min, x max, z min, z
/// max]`, the world's `+0x550..+0x55c`). More than 8 ft from where the snapshot last put the quad
/// ([`Remote::position`]), the position and velocity lags are set to it (each lane but `FLT_MAX`);
/// more than 16 ft, it is also kept in [`NetSmooth::put_back_at`] and the quad put back
/// ([`Call::NetPutBack`]). The position then goes through its lags into [`Remote::position`] (`w`
/// one); the velocity's change is kept, the velocity (`w` zero) goes to [`Remote::velocity`] unlagged
/// and its length to [`Quad::speed_len`]; the orientation to [`Remote::orientation`].
#[allow(clippy::too_many_arguments)] // the original's inputs, each one a word it reads
pub fn net_interpolate(
    older: &Snapshot,
    newer: &Snapshot,
    t: F,
    dt: F,
    area: [F; 4],
    s: &mut NetSmooth,
    r: &mut Remote,
    a: &mut Anim,
    q: &mut Quad,
    body: &mut Body,
    table: &crate::air::AcosTable,
    m: &mut impl Moto,
) {
    let l = net_interpolate_head(older, newer, t, dt, area, s, r, a, table);
    if l.put_back {
        m.call(Call::NetPutBack, q, body);
    }
    net_interpolate_tail(&l, dt, s, r, q);
}

/// What [`net_interpolate_head`] hands its tail: the lerped position (held inside the area), the
/// lerped velocity and the slerped orientation, and whether the quad is to be put back first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetLerp {
    pub position: Vf,
    pub velocity: Vf,
    pub orientation: Vf,
    pub put_back: bool,
}

/// [`net_interpolate`] up to its put-back: the blends, the stunt id, the wreck and untraced words, the
/// lerps and slerp, the area hold, and the lags set (and [`NetSmooth::put_back_at`] kept) when far.
/// A caller with the quad in RAM writes these words, runs the put-back when [`NetLerp::put_back`]
/// asks for it, reads the words back and finishes with [`net_interpolate_tail`].
#[allow(clippy::too_many_arguments)] // the original's inputs, each one a word it reads
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn net_interpolate_head(
    older: &Snapshot,
    newer: &Snapshot,
    t: F,
    dt: F,
    area: [F; 4],
    s: &mut NetSmooth,
    r: &Remote,
    a: &mut Anim,
    table: &crate::air::AcosTable,
) -> NetLerp {
    a.crossup_blend = s.crossup.run(dt, net_lerp(older.crossup_blend, newer.crossup_blend, t));
    a.fore_aft_blend = s.fore_aft.run(dt, net_lerp(older.fore_aft_blend, newer.fore_aft_blend, t));
    a.next_stunt = newer.stunt;
    s.word = newer.word;
    s.wreck = newer.wreck;

    let mut p = net_lerp_xyz(older.position, newer.position, t);
    let v = net_lerp_xyz(older.velocity, newer.velocity, t);
    let o = crate::math::slerp(older.orientation, newer.orientation, t, true, table);
    let edge = F(NET_EDGE);
    for (lane, lo, hi) in [(0, area[0], area[1]), (2, area[2], area[3])] {
        let x = F(p.0[lane]);
        if x < edge + lo || !(x <= hi - edge) {
            p.0[lane] = x.max(edge + lo).min(hi - edge).0;
        }
    }
    let d = sub_xyz(r.position, p);
    let far = F(fpu::sqrt(dot3(d, d).0));
    let mut put_back = false;
    if !(far <= F(NET_NEAR)) {
        for k in 0..3 {
            if F(p.0[k]) != F(f32::MAX) {
                s.position[k].value = F(p.0[k]);
            }
        }
        for k in 0..3 {
            if F(v.0[k]) != F(f32::MAX) {
                s.velocity[k].value = F(v.0[k]);
            }
        }
        if !(far <= F(NET_FAR)) {
            s.put_back_at = p;
            put_back = true;
        }
    }
    NetLerp { position: p, velocity: v, orientation: o, put_back }
}

/// [`net_interpolate`] after its put-back: the position through its lags into [`Remote::position`],
/// the velocity's change, the velocity and its length, and the orientation.
pub fn net_interpolate_tail(l: &NetLerp, dt: F, s: &mut NetSmooth, r: &mut Remote, q: &mut Quad) {
    let (p, v) = (l.position, l.velocity);
    let lagged = [0, 1, 2].map(|k| s.position[k].run(dt, F(p.0[k])).0);
    r.position = Vf::new(lagged[0], lagged[1], lagged[2], 1.0);
    s.velocity_change = sub_xyz(v, r.velocity);
    r.velocity = Vf::new(v.x(), v.y(), v.z(), 0.0);
    q.speed_len = F(fpu::sqrt(dot3(v, v).0));
    r.orientation = l.orientation;
}

/// What the animation driver asks of the rider's stunt clips: whether the rider's animation set has a
/// stunt's clip ready (asked for ids below [`RIDING_POSE`]), and that clip's length (asked only when
/// it is ready and the animation was not stopped this frame).
pub trait StuntClips {
    fn ready(&mut self, stunt: i32) -> bool;
    fn length(&mut self, stunt: i32) -> F;
}

fn unit(x: F) -> F {
    x.max(F::ZERO).min(F::ONE)
}

/// **The quad's animation driver**, once a frame: the biplane's own driver first ([`base_animate`]),
/// then the rider's clip, the model's nodes and the hung nodes.
///
/// What the simulation reads of it:
///
/// - In vehicle states 3 and 4, [`base_animate`]'s: the world angular velocity zeroed, the quad put
///   between its network snapshots and unpacked.
/// - [`Anim::fade`] run down by `dt` (`0 + fade − rate·dt` through the accumulator) into `0..1`.
/// - With no put-back this frame ([`PutBack::request`] zero) and the stunt changed or
///   [`Anim::restart`] set, the rider's animation is stopped and the fade put back to one.
/// - The stunt played this frame is the stunt steps' ([`Anim::stunt`]), so [`Quad::balancing`] is
///   whether it is the balance stunt.
/// - A stunt below [`RIDING_POSE`] with its clip ready ([`StuntClips`]) plays it, at its time over the
///   clip's length (none when the animation was stopped).
/// - Otherwise, outside vehicle states 3 and 4, the fore-and-aft blend is `(1 + stick) · ½`; for the
///   riding pose the crossup blend is the crossup's place in `±range` (none when the two ends are
///   equal), mapped through `0 + t·1` and clamped, and [`PutBack::settle`] is it; for any other stunt
///   it is `½ + ½ · lean` and the middle pose is played at a half.
///
/// The clips, the node pose, the frame reads and the attachment are [`Call`]s.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn animate(
    a: &mut Anim,
    pb: &mut PutBack,
    q: &mut Quad,
    body: &mut Body,
    net: &NetClock,
    dt: F,
    clips: &mut impl StuntClips,
    m: &mut impl Moto,
) {
    base_animate(a.vehicle_state, net, q, body, dt, m);
    a.fade = unit(Acc::adda(F::ZERO, a.fade).msub(a.fade_rate, dt));
    let stopped = pb.request == 0 && (a.next_stunt != a.stunt || a.restart != 0);
    if stopped {
        m.call(Call::AnimStop, q, body);
        a.fade = F::ONE;
    }
    a.stunt = a.next_stunt;
    q.balancing = a.stunt == a.balance_stunt;
    let held = a.vehicle_state == 3 || a.vehicle_state == 4;
    let call = if a.stunt < RIDING_POSE && clips.ready(a.stunt) {
        let weight = if stopped { F::ZERO } else { a.stunt_time / clips.length(a.stunt) };
        Call::PlayClip { clip: a.stunt, weight: unit(weight), fade: a.fade }
    } else if a.stunt == RIDING_POSE {
        if !held {
            a.fore_aft_blend = F(0.5) * (F::ONE + q.lean_fore_aft);
            let lo = F(fpu::neg(a.crossup_range.0));
            a.crossup_blend = if lo == a.crossup_range {
                F::ZERO
            } else {
                let t = (q.crossup - lo) / (a.crossup_range - lo);
                unit(Acc::adda(F::ZERO, F::ZERO).madd(t, F::ONE - F::ZERO))
            };
        }
        pb.settle = a.crossup_blend;
        let (clips, mix) = if a.fore_aft_blend < F(0.5) {
            ([POSE_CLIPS[1], POSE_CLIPS[0]], F(2.0) * a.fore_aft_blend)
        } else {
            ([POSE_CLIPS[2], POSE_CLIPS[1]], F(2.0) * (a.fore_aft_blend - F(0.5)))
        };
        Call::BlendClips { clips, weight: unit(a.crossup_blend), mix: unit(mix), fade: a.fade }
    } else {
        if !held {
            a.fore_aft_blend = F(0.5) * (F::ONE + q.lean_fore_aft);
            a.crossup_blend = Acc::adda(F::ZERO, F(0.5)).madd(F(0.5), q.lean);
        }
        Call::PlayClip { clip: POSE_CLIPS[1], weight: F(0.5), fade: a.fade }
    };
    m.call(call, q, body);
    m.call(Call::PoseNodes, q, body);
    m.call(Call::ReadFrame(0), q, body);
    m.call(Call::ReadFrame(1), q, body);
    for (i, &kind) in pb.hung.iter().enumerate() {
        if kind == 0 || kind == 1 {
            m.call(Call::SetFrame { hung: i, frame: kind as usize }, q, body);
        }
    }
    m.call(Call::AttachmentMoved, q, body);
}

/// The constructor's steering lock, either way: a quarter turn's half (`0x3f490fd9`), which the reader
/// replaces on the fronts only.
const CONSTRUCTED_LOCK: f32 = 0.78539807;

impl WheelTire {
    /// What the wheel's constructor leaves in the words [`tire_phy`] writes or keeps: all zero but the
    /// three spin inertias (1), the steering lock (`±`[`CONSTRUCTED_LOCK`]) and
    /// `OtherTireNormalLoadScale` (1). `mount` and `sweep` are the wheel setup's, off the model: the
    /// reader reads them and is handed them here.
    pub fn constructed(mount: Vf, sweep: bool) -> WheelTire {
        WheelTire {
            spin_inertia: F::ONE,
            inv_spin_inertia: F::ONE,
            tire_inertia: F::ONE,
            max_steer_angle: F(CONSTRUCTED_LOCK),
            min_steer_angle: -F(CONSTRUCTED_LOCK),
            other_load_scale: F::ONE,
            mount,
            sweep,
            ..WheelTire::default()
        }
    }
}

/// `(0, 4.1, -0.6, 1)`: the quad's `FPCameraOffset` before `[Camera]` is read (the cars' is 5.1 up).
const QUAD_FP_CAMERA_OFFSET: Vf = Vf::new(0.0, 4.1, -0.6, 1.0);
/// The quad's follow camera's presets file before `[Camera]` is read.
pub const QUAD_CAMERA_FILE: &str = "FC_ATVDefault.pb";
/// `(1, 1, 1, 1)`: `WreckDamping`'s default.
const ONES: Vf = Vf::new(1.0, 1.0, 1.0, 1.0);
/// The string getter's buffer for `FCamFileName`, terminator included.
const CAMERA_FILE_LEN: usize = 0x20;

/// **The quad's loader** (MD122): an `atv_*.phy` as SLUS-21104 loads it into a quad, in its order and
/// with its defaults, with no RAM image.
///
/// - MXVehicle's base loader: `[Mass Properties]` through the rigid-body part reader
///   ([`crate::handling::mass::Part`]), then `[Wrecks]`' `MaxWreckTime` (default 5), `WreckGravity`
///   (3), `WreckDamping` (ones) and `WreckInertia` (`(0, 0, 0, 1)`).
/// - The bike's shared loader: `[Mass Properties]` again ([`crate::handling::mass::Bike`]), the tire
///   reader ([`tire_phy`], against the part's mass) and the bike's keys — `[Engine]`, `[Roll Control]`,
///   `[Pitch Control]`, `[Aerodynamics]`, `[Airborne Control]`, `[Joystick]`, `[Preload]`, `[Backflip]`,
///   `[Three Sixty]`, `[Donuts]`, `[No Hander Lander]`, `[Wrecks]` — which are MX Unleashed's keys with
///   MX Unleashed's defaults and units, so [`crate::phy::Phy::read`] reads them; then the machine's
///   `[Camera]` and the bike's `[Animation]` boxes.
///
/// What the loader takes from the model rather than the file is handed in: the model's half box (the
/// part reader's inertia) and each wheel's mount and sweep shape (the wheel setup's).
#[derive(Clone, Debug)]
pub struct Load {
    pub part: crate::handling::mass::Part,
    pub max_wreck_time: F,
    pub wrecks: crate::car::MxvatvWrecks,
    pub bike: crate::handling::mass::Bike,
    /// The four wheels from their constructed state, and the segment each sweep shape was rebuilt to.
    pub tires: TireSetup,
    pub sweeps: [Option<[Vf; 2]>; 4],
    /// The engine as its three setters leave it (in first), its `EngineInertia` (kept beside the
    /// inverse) and its peak torque and the revs of it.
    pub engine: crate::r#move::Engine,
    pub engine_inertia: F,
    pub peak: crate::phy::Peak,
    /// The ground step's keys: the clutch, shift points, reverse push, steering fade, donut and drag,
    /// with the centres of gravity and travels the bike's `[Mass Properties]` read and the springs the
    /// tire reader set.
    pub mv: crate::r#move::Params,
    pub roll: crate::phy::Roll,
    pub pitch: crate::phy::Pitch,
    pub joystick: crate::phy::Joystick,
    pub steer_ramp: crate::handling::step::SteerRamp,
    pub donuts: crate::phy::Donuts,
    pub balance: crate::phy::Balance,
    pub air: crate::air::Params,
    pub gas_gyro_torque: F,
    pub max_crossup: F,
    pub three_sixty: crate::air::SpinTuning,
    pub camera: crate::car::MxvatvCamera,
    pub pelvis_box_position: Vf,
    pub pelvis_box_size: Vf,
}

/// Where [`Load::words`] puts what it loaded: the quad and what it points at — its rigid body,
/// collision record, physics world, engine and four wheels — and the three tables the engine's setters
/// keep behind pointers (the RPM points, the torques, the gear ratios).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadAt {
    pub quad: u32,
    pub body: u32,
    pub record: u32,
    pub world: u32,
    pub engine: u32,
    pub wheels: [u32; 4],
    pub curve_rpm: u32,
    pub curve_torque: u32,
    pub ratios: u32,
}

impl Load {
    /// Read the file's bytes.
    pub fn read_bytes(bytes: &[u8], half_box: Vf, mounts: [(Vf, bool); 4], curve: &crate::camera::CurveTable) -> Load {
        Load::read(&crate::phy::Ini::parse_bytes(bytes), half_box, mounts, curve)
    }

    /// Load a parsed file. `mounts` is each wheel's mount and whether it has a sweep shape, in
    /// [`LEFT_FRONT`]..[`RIGHT_REAR`] order.
    pub fn read(ini: &crate::phy::Ini, half_box: Vf, mounts: [(Vf, bool); 4], curve: &crate::camera::CurveTable) -> Load {
        use crate::handling::mass;
        let part = mass::Part::read(&ini.section("Mass Properties"), half_box, curve);
        let s = ini.section("Wrecks");
        let max_wreck_time = s.float_or("MaxWreckTime", F(5.0));
        let wrecks = crate::car::MxvatvWrecks {
            wreck_gravity: s.float_or("WreckGravity", F(3.0)),
            wreck_damping: s.vector("WreckDamping", ONES),
            wreck_inertia: s.vector("WreckInertia", NOTHING),
        };
        let bike = mass::Bike::read(&ini.section("Mass Properties"));
        let mut tires = TireSetup { wheels: mounts.map(|(m, s)| WheelTire::constructed(m, s)), ..TireSetup::default() };
        let sweeps = tire_phy(&mut tires, ini, part.body.mass);

        let phy = crate::phy::Phy::read(ini, curve);
        let mut mv = phy.mv;
        (mv.cofg, mv.balance_cofg) = (bike.cofgs[0], bike.cofgs[2]);
        (mv.cofg_travel_forward, mv.cofg_travel_back, mv.brake_cofg_travel_forward) =
            (bike.travel_forward, bike.travel_back, bike.brake_travel_forward);
        (mv.front_spring_when_stopped, mv.front_spring) = tires.front_spring;
        (mv.rear_spring_when_stopped, mv.rear_spring) = tires.rear_spring;
        let engine_inertia = ini.section("Engine").float("EngineInertia").unwrap_or(F::ZERO);

        let s = ini.section("Three Sixty");
        let key = |k: &str| s.float(k).unwrap_or(F::ZERO);
        let three_sixty = crate::air::SpinTuning {
            min_time: key("MinThreeSixtyTime"),
            max_time: key("MaxThreeSixtyTime"),
            min_torque_x: key("MinThreeSixtyTorqueX"),
            max_torque_x: key("MaxThreeSixtyTorqueX"),
            min_torque_y: key("MinThreeSixtyTorqueY"),
            max_torque_y: key("MaxThreeSixtyTorqueY"),
            rate_increase: key("ThreeSixtyTorqueRateIncrease"),
            rate_decrease: key("ThreeSixtyTorqueRateDecrease"),
        };

        let s = ini.section("Camera");
        let camera_file = match s.text("FCamFileName") {
            Some(name) => {
                let mut end = name.len().min(CAMERA_FILE_LEN - 1);
                while !name.is_char_boundary(end) {
                    end -= 1;
                }
                name[..end].to_string()
            }
            None => QUAD_CAMERA_FILE.to_string(),
        };
        let camera = crate::car::MxvatvCamera {
            left_shoulder_offset: s.vector("Left1stPerShoulderOffset", NOTHING),
            right_shoulder_offset: s.vector("Right1stPerShoulderOffset", NOTHING),
            fp_camera_offset: s.vector("FPCameraOffset", QUAD_FP_CAMERA_OFFSET),
            camera_file,
        };

        Load {
            part,
            max_wreck_time,
            wrecks,
            bike,
            tires,
            sweeps,
            engine: phy.engine,
            engine_inertia,
            peak: phy.peak,
            mv,
            roll: phy.roll,
            pitch: phy.pitch,
            joystick: phy.joystick,
            steer_ramp: phy.steer_ramp,
            donuts: phy.donuts,
            balance: phy.balance,
            air: phy.air,
            gas_gyro_torque: phy.gas_gyro_torque,
            max_crossup: phy.max_crossup,
            three_sixty,
            camera,
            pelvis_box_position: phy.animation.pelvis_box_position,
            pelvis_box_size: phy.animation.pelvis_box_size,
        }
    }

    /// The quad's wheelie, stoppie, steering, full-lock, donut and spring keys. The full-lock option is
    /// off, as the quad is made.
    pub fn params(&self) -> Params {
        let (p, m) = (&self.pitch, &self.mv);
        Params {
            max_pitch_command: p.max_pitch_command,
            pitch_gain: p.pitch_gain,
            pitch_rate_gain: p.pitch_rate_gain,
            inv_wheelie_top_speed: p.inv_wheelie_top_speed,
            max_stoppie_pitch_command: p.max_stoppie_pitch_command,
            stoppie_pitch_gain: p.stoppie_pitch_gain,
            stoppie_yaw_rate_scale: p.stoppie_yaw_rate_scale,
            stoppie_top_speed: p.stoppie_top_speed,
            steer_speed_1: m.steer_speed_1,
            steer_scale_1: m.steer_scale_1,
            steer_speed_2: m.steer_speed_2,
            steer_scale_2: m.steer_scale_2,
            full_lock: false,
            full_lock_angle: self.steer_ramp.amount,
            full_lock_time: self.steer_ramp.time,
            max_donut_speed: m.max_donut_speed,
            donut_angle: m.donut_angle,
            donut_angle_back: m.donut_angle_back,
            front_spring: self.tires.front_spring,
            rear_spring: self.tires.rear_spring,
        }
    }

    /// [`Bike::clutch_keys`]: the ground step's keys the quad's bike steps read.
    pub fn clutch_keys(&self) -> crate::r#move::Params {
        let m = &self.mv;
        crate::r#move::Params {
            clutch_torque_scale_time: m.clutch_torque_scale_time,
            max_clutch_torque_scale_speed: m.max_clutch_torque_scale_speed,
            max_clutch_torque_scale: m.max_clutch_torque_scale,
            up_shift_rpm: m.up_shift_rpm,
            down_shift_rpm: m.down_shift_rpm,
            cofg: m.cofg,
            balance_cofg: m.balance_cofg,
            cofg_travel_forward: m.cofg_travel_forward,
            cofg_travel_back: m.cofg_travel_back,
            clutch_cofg_travel_back: m.clutch_cofg_travel_back,
            brake_cofg_travel_forward: m.brake_cofg_travel_forward,
            drag: m.drag,
            ..crate::r#move::Params::default()
        }
    }

    /// [`Bike::air_keys`]: the keys the quad's air steps read — the air control, gyro and unroll, the
    /// preload and backflip, and the speed and airborne-pitch wreck limits; the rest are left at 0.
    /// The control authority time is among them: the quad holds its own ([`Quad::authority_time`]),
    /// which the air steps put over it.
    pub fn air_keys(&self) -> crate::air::Params {
        let a = &self.air;
        crate::air::Params {
            airborne_control_moment: a.airborne_control_moment,
            airborne_damping: a.airborne_damping,
            airborne_control_force: a.airborne_control_force,
            brake_gyro_torque: a.brake_gyro_torque,
            unroll_gain: a.unroll_gain,
            unroll_torque: a.unroll_torque,
            unyaw_gain: a.unyaw_gain,
            unyaw_torque: a.unyaw_torque,
            preload_time: a.preload_time,
            max_preload: a.max_preload,
            min_preload_time: a.min_preload_time,
            max_preload_time: a.max_preload_time,
            min_dpad_preload_time: a.min_dpad_preload_time,
            max_dpad_preload_time: a.max_dpad_preload_time,
            min_backflip_time: a.min_backflip_time,
            max_backflip_time: a.max_backflip_time,
            min_backflip_torque: a.min_backflip_torque,
            max_backflip_torque: a.max_backflip_torque,
            backflip_torque_rate_increase: a.backflip_torque_rate_increase,
            backflip_torque_rate_decrease: a.backflip_torque_rate_decrease,
            front_wreck_speed_limit: a.front_wreck_speed_limit,
            back_wreck_speed_limit: a.back_wreck_speed_limit,
            side_wreck_speed_limit: a.side_wreck_speed_limit,
            airborne_pitch_down_wreck_angle: a.airborne_pitch_down_wreck_angle,
            airborne_pitch_up_wreck_angle: a.airborne_pitch_up_wreck_angle,
            control_authority_time: F::ZERO,
            racing_down_wreck_speed_limit: F::ZERO,
            freestyle_down_wreck_speed_limit: F::ZERO,
            ground_pitch_down_wreck_angle: F::ZERO,
            ground_pitch_up_wreck_angle: F::ZERO,
            airborne_roll_wreck_angle: F::ZERO,
            no_hander_wreck_speed_limit: F::ZERO,
            no_hander_max_roll_angle: F::ZERO,
            joust_wreck_height: F::ZERO,
            crossup_wreck: F::ZERO,
            crossup_wreck_time: F::ZERO,
            rotation_speed: None,
        }
    }

    /// The loaded tuning over a bike: the input limiter, the clutch keys, the engine (in first, the
    /// curve's cursor at its foot), the air keys, the gas gyro, and the four wheels' tire words (their
    /// suspension, steering and spin keys, and each tire's surfaces). Its state is left as it was.
    pub fn tune(&self, b: &mut Bike) {
        b.joystick = self.joystick;
        b.clutch_keys = self.clutch_keys();
        let e = &self.engine;
        let be = &mut b.engine;
        (be.gear, be.gears, be.final_drive, be.gear_ratios, be.ratio, be.inv_ratio) =
            (e.gear, e.gears, e.final_drive, e.gear_ratios, e.ratio, e.inv_ratio);
        (be.rev_limit, be.idle_throttle, be.engine_braking, be.inv_inertia) = (e.rev_limit, e.idle_throttle, e.engine_braking, e.inv_inertia);
        (be.clutch_static, be.clutch_kinetic, be.min_rpm) = (e.clutch_static, e.clutch_kinetic, e.min_rpm);
        (be.curve_rpm, be.curve_torque, be.curve) = (e.curve_rpm, e.curve_torque, e.curve);
        b.air_keys = self.air_keys();
        b.gas_gyro_torque = self.gas_gyro_torque;
        for k in 0..4 {
            let t = &self.tires.wheels[k];
            let w = &mut b.wheels[k];
            (w.max_compression, w.max_damper_speed, w.extension_rate) = (t.strut_compression, t.max_compression_rate, t.max_decompression_rate);
            (w.damping_out, w.damping_in) = (t.damping_in, t.damping_out);
            (w.max_steer_angle, w.min_steer_angle) = (t.max_steer_angle, t.min_steer_angle);
            // The steering torque the setter leaves is the step's input, cleared every tick: not tuning.
            (w.steer_damping, w.steer_inv_inertia) = (t.steer_damping, t.inv_steer_inertia);
            (w.mount, w.probe_length) = (t.mount, t.radius);
            let s = &mut b.spins[k];
            (s.parametric, s.inv_inertia, s.tire_inertia) = (t.parametric, t.inv_spin_inertia, t.tire_inertia);
            (s.arm, s.other_load_scale, s.inv_radius) = (t.force_position, t.other_load_scale, t.inv_radius);
            s.surfaces = t.surfaces;
        }
    }

    /// The loaded words over the quad's own state: the reverse push's speed and gain, the gravity force
    /// the part reader put in the world, and each wheel's lock, `1/r` and whether the init made it a
    /// steered one (the fronts) — with the balancing flag the init clears on all four.
    pub fn tune_quad(&self, q: &mut Quad) {
        (q.push_target, q.push_gain) = (self.mv.reverse_speed, self.mv.reverse_gain);
        q.gravity = self.part.gravity.force;
        for (k, w) in q.wheels.iter_mut().enumerate() {
            let t = &self.tires.wheels[k];
            (w.max_steer_angle, w.inv_radius) = (t.max_steer_angle, t.inv_radius);
            (w.steered, w.balancing) = (k < LEFT_REAR, false);
        }
    }

    /// The loaded mass, damping, inertia and rate limits over the quad's rigid body.
    pub fn tune_body(&self, b: &mut crate::atv_body::Body) {
        let (pb, bk) = (&self.part.body, &self.bike);
        (b.inv_mass, b.damping, b.ang_damping) = (pb.inv_mass, pb.damping, pb.angular_damping);
        b.inv_inertia = Vf::new(bk.inv_inertia[0].0, bk.inv_inertia[1].0, bk.inv_inertia[2].0, b.inv_inertia.w());
        if let Some(y) = bk.yaw_rate_limit {
            b.max_rates = [F(QUAD_MAX_RATE), y, F(QUAD_MAX_RATE)];
        }
    }

    /// Every word the loaders write, as [`crate::handling::mass::Part::words`] gives them: `(address,
    /// mask, bits)`, in the order they are written (a later word over an earlier one wins). A byte of
    /// the camera's file name is the low byte of its word's mask.
    pub fn words(&self, at: &LoadAt) -> Vec<(u32, u32, u32)> {
        use crate::handling::mass::Layout;
        let mut w = self.part.words(Layout { object: at.quad, body: at.body, record: at.record, gravity: at.world });
        let o = at.quad;
        let f = |w: &mut Vec<(u32, u32, u32)>, a: u32, v: F| w.push((a, !0, v.to_bits()));
        let i = |w: &mut Vec<(u32, u32, u32)>, a: u32, v: u32| w.push((a, !0, v));
        let v = |w: &mut Vec<(u32, u32, u32)>, a: u32, q: Vf| {
            for (k, b) in q.to_bits().into_iter().enumerate() {
                w.push((a + 4 * k as u32, !0, b));
            }
        };
        f(&mut w, o + 0xc88, self.max_wreck_time);
        f(&mut w, o + 0xc90, self.wrecks.wreck_gravity);
        v(&mut w, o + 0xca0, self.wrecks.wreck_damping);
        v(&mut w, o + 0xcb0, self.wrecks.wreck_inertia);
        w.extend(self.bike.words(o, at.body));

        // The tire reader.
        for (k, t) in self.tires.wheels.iter().enumerate() {
            let a = at.wheels[k];
            i(&mut w, a + 0x34, t.parametric as u32);
            for (off, x) in [
                (0x3c, t.strut_compression),
                (0x40, t.max_compression_rate),
                (0x44, t.max_decompression_rate),
                (0x48, t.spring),
                (0x4c, t.damping_in),
                (0x50, t.damping_out),
                (0x54, t.spin_inertia),
                (0x58, t.inv_spin_inertia),
                (0x5c, t.tire_inertia),
                (0x60, t.steer_torque),
                (0x64, t.steer_damping),
                (0x68, t.steer_inertia),
                (0x6c, t.inv_steer_inertia),
                (0x70, t.max_steer_angle),
                (0x74, t.min_steer_angle),
                (0xb8, t.other_load_scale),
                (0x214, t.radius),
                (0x218, t.ply),
                (0x21c, t.width),
                (0x220, t.tire_mass),
                (0x838, t.inv_radius),
                (0x8a8, t.ply_scale),
                (0x8b4, t.steer_torque),
            ] {
                f(&mut w, a + off, x);
            }
            // The init's: the fronts steer; no wheel is balancing.
            i(&mut w, a + 0x2c, u32::from(k < LEFT_REAR));
            i(&mut w, a + 0x30, 0);
            v(&mut w, a + 0xa0, t.force_position);
            v(&mut w, a + 0x100, t.rest);
            for (j, s) in t.surfaces.iter().enumerate() {
                for (n, x) in s.drive.iter().chain(&s.side).chain(&s.drag).enumerate() {
                    f(&mut w, a + 0x238 + 0x40 * n as u32 + 4 * j as u32, *x);
                }
            }
        }
        for (off, x) in [
            (0x137c, self.tires.front_spring.0),
            (0x1380, self.tires.front_spring.1),
            (0x1384, self.tires.rear_spring.0),
            (0x1388, self.tires.rear_spring.1),
        ] {
            f(&mut w, o + off, x);
        }
        for j in 0..crate::phy::MAX_SURFACES {
            f(&mut w, o + 0x1654 + 4 * j as u32, self.tires.mud_time[j]);
            f(&mut w, o + 0x1694 + 4 * j as u32, self.tires.mud_amount[j]);
        }

        // [Engine]: the engine's words, its tables, and the quad's.
        let (e, en) = (&self.engine, at.engine);
        for (off, x) in [
            (0x2c, e.idle_throttle),
            (0x30, e.engine_braking),
            (0x34, e.final_drive),
            (0x38, self.engine_inertia),
            (0x44, e.clutch_static),
            (0x48, e.clutch_kinetic),
            (0x54, e.min_rpm),
            (0x74, e.ratio),
            (0x78, e.inv_ratio),
            (0xa8, self.peak.rpm),
            (0xac, e.rev_limit),
            (0xb0, self.peak.torque),
        ] {
            f(&mut w, en + off, x);
        }
        if F::ZERO < self.engine_inertia {
            f(&mut w, en + 0x3c, e.inv_inertia);
        }
        i(&mut w, en + 0x5c, e.gear as u32);
        i(&mut w, en + 0x60, e.curve.points);
        i(&mut w, en + 0x6c, e.gears as u32);
        i(&mut w, en + 0x80, e.curve.points);
        i(&mut w, en + 0x84, e.curve.index);
        f(&mut w, en + 0x88, e.curve.frac());
        for k in 0..e.curve.points {
            f(&mut w, at.curve_rpm + 4 * k, e.curve_rpm[k as usize]);
            f(&mut w, at.curve_torque + 4 * k, e.curve_torque[k as usize]);
        }
        for k in 0..e.gears as u32 {
            f(&mut w, at.ratios + 4 * k, e.gear_ratios[k as usize]);
        }
        let m = &self.mv;
        for (off, x) in [
            (0x1350, m.clutch_torque_scale_time),
            (0x1358, m.max_clutch_torque_scale_speed),
            (0x135c, m.max_clutch_torque_scale),
            (0x1328, m.clutch_cofg_travel_back),
            (0x13dc, m.up_shift_rpm),
            (0x13e0, m.down_shift_rpm),
            (0x123c, m.reverse_speed),
            (0x1240, m.reverse_gain),
        ] {
            f(&mut w, o + off, x);
        }

        // [Roll Control], [Pitch Control], [Aerodynamics].
        let (r, p) = (&self.roll, &self.pitch);
        for (off, x) in [
            (0x1360, r.max_roll_command),
            (0x1364, r.max_roll_rate_command),
            (0x1368, r.max_roll_speed),
            (0x136c, r.max_roll_yaw_rate),
            (0x1370, r.roll_gain),
            (0x1374, r.min_stand_upright_speed),
            (0x1378, r.max_stand_upright_speed),
            (0x138c, r.roll_control_gain),
            (0x1390, r.roll_control_rate_gain),
            (0x1394, r.roll_control_max_cg),
            (0x1398, r.max_roll_rate_disturbance),
            (0x139c, p.max_pitch_command),
            (0x13a0, p.pitch_gain),
            (0x13a4, p.pitch_rate_gain),
            (0x13a8, p.inv_wheelie_top_speed),
            (0x13cc, p.max_stoppie_pitch_command),
            (0x13d0, p.stoppie_pitch_gain),
            (0x13d4, p.stoppie_yaw_rate_scale),
            (0x13d8, p.stoppie_top_speed),
            (0x13b4, p.stoppie_brake),
        ] {
            f(&mut w, o + off, x);
        }
        f(&mut w, at.world + 4, m.drag);

        // [Airborne Control], [Joystick], [Preload], [Backflip], [Three Sixty], [Donuts].
        let (a, j, t, d) = (&self.air, &self.joystick, &self.three_sixty, &self.donuts);
        f(&mut w, o + 0x1180, a.airborne_damping);
        f(&mut w, o + 0x1184, a.airborne_control_force);
        v(&mut w, o + 0x1170, a.airborne_control_moment);
        for (off, x) in [
            (0x1330, a.brake_gyro_torque),
            (0x1334, self.gas_gyro_torque),
            (0x1338, a.unroll_gain),
            (0x133c, a.unroll_torque),
            (0x1340, a.unyaw_gain),
            (0x1344, a.unyaw_torque),
            (0x14b0, self.max_crossup),
            (0x12d8, j.x_axis_rate_limit),
            (0x12dc, j.y_axis_rate_limit),
            (0x12e0, j.x_dpad_rate_limit),
            (0x12e4, j.y_dpad_rate_limit),
            (0x12f4, self.steer_ramp.amount),
            (0x12f8, self.steer_ramp.time),
            (0x1300, m.steer_speed_1),
            (0x1304, m.steer_scale_1),
            (0x1308, m.steer_speed_2),
            (0x130c, m.steer_scale_2),
            (0x1310, j.gas_rate_limit),
            (0x1314, j.brake_rate_limit),
            (0x1318, j.clutch_in_rate_limit),
            (0x131c, j.clutch_out_rate_limit),
            (0xc5c, a.preload_time),
            (0xc6c, a.max_preload),
            (0xc64, a.min_preload_time),
            (0xc68, a.max_preload_time),
            (0x1160, a.min_dpad_preload_time),
            (0x1164, a.max_dpad_preload_time),
            (0x11b4, a.max_backflip_torque),
            (0x11b0, a.min_backflip_torque),
            (0x11b8, a.backflip_torque_rate_increase),
            (0x11bc, a.backflip_torque_rate_decrease),
            (0x11a4, a.min_backflip_time),
            (0x11a8, a.max_backflip_time),
            (0x11f0, t.max_torque_x),
            (0x11ec, t.min_torque_x),
            (0x11f8, t.max_torque_y),
            (0x11f4, t.min_torque_y),
            (0x11fc, t.rate_increase),
            (0x1200, t.rate_decrease),
            (0x11e0, t.min_time),
            (0x11e4, t.max_time),
            (0x1228, d.min_donut_speed),
            (0x122c, m.max_donut_speed),
            (0x1230, m.donut_angle),
            (0x1234, m.donut_angle_back),
            (0x1238, d.roll_angle_scale),
        ] {
            f(&mut w, o + off, x);
        }

        // [No Hander Lander], [Wrecks].
        let b = &self.balance;
        for (off, x) in [
            (0x1500, b.min_balance_inertia),
            (0x1504, b.max_balance_inertia),
            (0x14fc, b.balance_inertia_rate),
            (0x1508, b.input_inertia),
            (0x1520, b.max_balance_point),
            (0x1514, b.balance_noise_amplitude),
            (0x151c, b.balance_noise_time_scale),
            (0x13e8, a.front_wreck_speed_limit),
            (0x13ec, a.back_wreck_speed_limit),
            (0x13f0, a.side_wreck_speed_limit),
            (0x13f4, a.racing_down_wreck_speed_limit),
            (0x13f8, a.freestyle_down_wreck_speed_limit),
            (0x13fc, a.ground_pitch_down_wreck_angle),
            (0x1400, a.ground_pitch_up_wreck_angle),
            (0x1404, a.airborne_pitch_down_wreck_angle),
            (0x1408, a.airborne_pitch_up_wreck_angle),
            (0x140c, a.airborne_roll_wreck_angle),
            (0x1420, a.crossup_wreck),
            (0x1424, a.crossup_wreck_time),
            (0x1414, a.no_hander_max_roll_angle),
            (0x1410, a.no_hander_wreck_speed_limit),
            (0x1418, a.joust_wreck_height),
        ] {
            f(&mut w, o + off, x);
        }

        // [Camera], [Animation].
        let c = &self.camera;
        v(&mut w, o + 0x980, c.left_shoulder_offset);
        v(&mut w, o + 0x990, c.right_shoulder_offset);
        v(&mut w, o + 0xda0, c.fp_camera_offset);
        let name = c.camera_file.as_bytes();
        for k in 0..=name.len() {
            let a = o + 0xdb0 + k as u32;
            let shift = 8 * (a & 3);
            w.push((a & !3, 0xff << shift, u32::from(name.get(k).copied().unwrap_or(0)) << shift));
        }
        v(&mut w, o + 0x1490, self.pelvis_box_position);
        v(&mut w, o + 0x14a0, self.pelvis_box_size);
        w
    }

    /// Load a parsed file with the model's inputs read off the model at the quad's spawn point `at`
    /// and `heading`: the part reader's box in the heading's own frame ([`heading_frame`]), the
    /// mounts in the unit one ([`spawn_frame`]).
    pub fn read_model(ini: &crate::phy::Ini, model: &Model, at: Vf, heading: Vf, curve: &crate::camera::CurveTable) -> Result<Load, String> {
        Ok(Load::read(ini, model.half_box(&heading_frame(at, heading)), model.mounts(&spawn_frame(at, heading))?, curve))
    }
}

// --- MD129: the loader's model inputs ----------------------------------------------------------------

/// The wheel nodes the wheel setup looks up by name, in [`LEFT_FRONT`]..[`RIGHT_REAR`] order.
pub const WHEEL_NODES: [&str; 4] = ["WheelLeftFront", "WheelRightFront", "WheelLeftRear", "WheelRightRear"];

/// One node of the quad's chassis model (`.psg`) as it loads: its name, its local rows (the bind pose,
/// which the animation controller saves untouched at load), its own box, and its parent.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelNode {
    pub name: String,
    pub local: crate::math::Mat4,
    pub own: crate::props::NodeBox,
    pub parent: Option<usize>,
}

/// **The quad's chassis model** (MD129), the `.psg` the machine's `.mcf` names (`SLTFile=`): its nodes
/// in the file's order, each after its parent. The model hangs under the quad's scene node, a node with
/// no box of its own whose world rows are the quad's placement, so what the loader reads off it — the
/// part reader's box and the wheel setup's mounts — is worked in that placement's frame and carries its
/// rounding: at a spawn some 3,000 ft out the bind pose's box and mounts move by up to a few 1e-4.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub nodes: Vec<ModelNode>,
}

impl Model {
    /// Read the `.psg`: `psg\0`, a version and the node count, then 224-byte records — the name
    /// (128 bytes), the local rows (16 floats, the translation last), the box's centre and half
    /// extent (three floats each, `w` 1 as the scene node keeps them), a radius and the parent (`-1`
    /// for the root).
    pub fn parse(psg: &[u8]) -> Result<Model, String> {
        const NODES: usize = 12;
        const RECORD: usize = 224;
        const NAME: usize = 128;
        const CENTRE: usize = 192;
        const HALF: usize = 204;
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
        let mut nodes = Vec::with_capacity(count.min(4096));
        for i in 0..count {
            let at = NODES + i * RECORD;
            let name = psg.get(at..at + NAME).ok_or_else(|| format!(".psg ends before node {i}"))?;
            let name = String::from_utf8_lossy(name.split(|&b| b == 0).next().unwrap_or_default()).into_owned();
            let row = |r: usize| -> Result<Vf, String> {
                let b = at + NAME + r * 16;
                Ok(Vf::from_bits([word(b)?, word(b + 4)?, word(b + 8)?, word(b + 12)?]))
            };
            let local = crate::math::Mat4([row(0)?, row(1)?, row(2)?, row(3)?]);
            let three = |b: usize| -> Result<Vf, String> { Ok(Vf::from_bits([word(b)?, word(b + 4)?, word(b + 8)?, 1f32.to_bits()])) };
            let own = crate::props::NodeBox { centre: three(at + CENTRE)?, half_extent: three(at + HALF)? };
            let parent = match usize::try_from(word(at + PARENT)? as i32) {
                Ok(p) if p < i => Some(p),
                Ok(p) => return Err(format!("node {i}'s parent {p} comes after it")),
                Err(_) => None,
            };
            nodes.push(ModelNode { name, local, own, parent });
        }
        Ok(Model { nodes })
    }

    /// Each node's world rows with the model's roots under `frame`, as the scene's world update
    /// composes them ([`crate::toys::compose`]).
    pub fn worlds(&self, frame: &crate::math::Mat4) -> Vec<crate::math::Mat4> {
        let mut worlds: Vec<crate::math::Mat4> = Vec::with_capacity(self.nodes.len());
        for n in &self.nodes {
            let parent = n.parent.map_or(*frame, |p| worlds[p]);
            worlds.push(crate::toys::compose(&n.local, &parent));
        }
        worlds
    }

    /// **The part reader's box**: the half extent of the quad's scene node's bounds
    /// ([`crate::prop_world::node_bounds`]) — the node has no box, its world rows are `frame`, and
    /// under it hangs the model, every node with its own box.
    pub fn half_box(&self, frame: &crate::math::Mat4) -> Vf {
        use crate::prop_world::BoundsNode;
        let worlds = self.worlds(frame);
        fn tree(m: &Model, worlds: &[crate::math::Mat4], i: usize) -> BoundsNode {
            let children = (0..m.nodes.len()).filter(|&k| m.nodes[k].parent == Some(i)).map(|k| tree(m, worlds, k)).collect();
            BoundsNode { own: Some(m.nodes[i].own), world: worlds[i], children }
        }
        let roots = (0..self.nodes.len()).filter(|&k| self.nodes[k].parent.is_none()).map(|k| tree(self, &worlds, k)).collect();
        crate::prop_world::node_bounds(&BoundsNode { own: None, world: *frame, children: roots }).half_extent
    }

    /// **The wheel setup's mounts**: each [`WHEEL_NODES`] node's world origin carried back into
    /// `frame` (less `frame`'s origin, then the rows' dots: `vmulax`, `vmadday`, `vmaddz` on the
    /// transposed rows), `w` 1; no wheel has a sweep shape, as the quad's init asks for none.
    pub fn mounts(&self, frame: &crate::math::Mat4) -> Result<[(Vf, bool); 4], String> {
        let worlds = self.worlds(frame);
        let [r0, r1, r2, r3] = frame.0;
        let column = |k: usize| Vf::new(r0[k], r1[k], r2[k], 0.0);
        let mut out = [(Vf::ZERO, false); 4];
        for (slot, name) in out.iter_mut().zip(WHEEL_NODES) {
            let i = self.nodes.iter().position(|n| n.name == name).ok_or_else(|| format!("the model has no {name}"))?;
            let d = worlds[i].0[3].sub(r3);
            let acc = column(0).mul(d.bc(Field::X));
            let acc = Vf::madd(acc, column(1), d.bc(Field::Y));
            let p = Vf::madd(acc, column(2), d.bc(Field::Z));
            *slot = (Vf::new(p.x(), p.y(), p.z(), 1.0), false);
        }
        Ok(out)
    }
}

/// **The quad's placement as the wheel setup reads the model in it**: at `at`, facing `heading` made
/// unit length ([`crate::contact::unit`]) as the third row, upright (`(0, 1, 0)`) and the first row
/// their cross, `(z.z, 0, -z.x)`. The quad keeps both as its spawn point and heading.
pub fn spawn_frame(at: Vf, heading: Vf) -> crate::math::Mat4 {
    rows_facing(at, crate::contact::unit(heading))
}

/// **The quad's placement as the part reader reads the model in it**: [`spawn_frame`] with the
/// heading as kept, not made unit length.
pub fn heading_frame(at: Vf, heading: Vf) -> crate::math::Mat4 {
    rows_facing(at, heading)
}

fn rows_facing(at: Vf, z: Vf) -> crate::math::Mat4 {
    crate::math::Mat4([
        Vf::new(z.z(), 0.0, (-F(z.x())).0, 0.0),
        Vf::new(0.0, 1.0, 0.0, 0.0),
        Vf::new(z.x(), z.y(), z.z(), 0.0),
        Vf::new(at.x(), at.y(), at.z(), 1.0),
    ])
}

/// The body's largest roll and pitch rates, either side of `YawRateLimit`.
const QUAD_MAX_RATE: f32 = 20.0;

// --- MD123: the quad on MX Unleashed's worlds ------------------------------------------------------

/// **The quad's ground query on an MX Unleashed world** (MD123): what [`Steps`]' `ground` answers on a
/// track the shell rides, in place of Desert Oasis's [`crate::atv_ground::Terrain::ground`]. The
/// probe's end `at` is moved onto the world's ground ([`crate::world::World::ground`], the query
/// [`crate::ride::Ride`]'s wheels ask), and the surface under it written into the wheel's record where
/// the world's own query writes one ([`crate::world::World::wheel_surface`]), else left as it was.
pub fn world_ground(world: &mut dyn crate::world::World, at: Vf, record: &mut u8) -> (Vf, Vf) {
    let (point, normal) = world.ground(at);
    if let Some(s) = world.wheel_surface(at) {
        *record = s;
    }
    (point, normal)
}

/// The world box the vehicle's in-world test reads ([`crate::atv_moto::in_world`]) for an MX Unleashed
/// world: its map edge ([`crate::world::World::area`]) from the origin, or none (every spot inside)
/// for a world with no edge. The floor is kept.
pub fn world_bounds(world: &dyn crate::world::World, floor: F) -> crate::atv_moto::Bounds {
    let area = world.area().map(|a| (Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(a.width.0, 0.0, a.depth.0, 1.0)));
    crate::atv_moto::Bounds { floor, area }
}

/// **The quad's wheel rays swept over an MX Unleashed world** (MD123), the [`Sweep`] for
/// [`Call::Contacts`] on a track: the trigger probe's four segments (one a wheel, in
/// [`LEFT_FRONT`]..[`RIGHT_REAR`] order, in the quad's frame; [`trigger_segments`]) stood at the scene
/// node's frame and swept as one bundle against everything solid ([`crate::world::World::solid`],
/// what [`crate::ride::Ride`]'s probes are swept against) and then the placed groups
/// ([`crate::world::World::groups`]) into the same record. Each segment that hit is a contact at its
/// own start (so [`wheel_contacts`] gives it to its wheel), the hit's normal and point, the thing hit's
/// owner number as its node, and no owner or vehicle (nothing on an MX Unleashed world listens).
///
/// The gather ([`Sweep::gather`]) is answered and does nothing: the world's sweep looks at all it has,
/// with no nearby list to fill. The world is shared with the ground query ([`Ride::frame_on`]).
pub struct OnWorld<'a, 'w> {
    pub world: &'a std::cell::RefCell<&'w mut dyn crate::world::World>,
    pub segments: [[Vf; 2]; 4],
}

impl Sweep for OnWorld<'_, '_> {
    fn sweep(&mut self, frame: &crate::ray_probe::Matrix) -> Option<(bool, Vec<Contact>)> {
        use crate::collide::{Bundle, Probe, Record};
        let m = crate::math::Mat4(*frame);
        let ends = self.segments.map(|[a, b]| [crate::math::transform_point(&m, a), crate::math::transform_point(&m, b)]);
        let centre = frame[3].merge(Dest::W, Vf::VF0);
        let mut radius = F::ZERO;
        let mut bundle = Bundle { count: 4, centre, ..Bundle::default() };
        for (k, [a, b]) in ends.iter().enumerate() {
            bundle.probes[k] = Probe::between(*a, *b);
            for p in [a, b] {
                let d = p.sub(centre);
                let r = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(d.x(), d.x()), d.y(), d.y()), d.z(), d.z())));
                if r > radius {
                    radius = r;
                }
            }
        }
        bundle.radius = radius;
        let mut record = Record::new();
        let mut world = self.world.borrow_mut();
        let solid = world.solid(&bundle, &mut record);
        let groups = world.groups(&bundle, &mut record);
        let contacts = (0..4)
            .filter(|&k| record.hit[k])
            .map(|k| Contact {
                at: self.segments[k][0],
                normal: record.normal[k],
                point: record.point[k],
                node: record.owner[k] as u32,
                ..Contact::default()
            })
            .collect();
        Some((solid || groups, contacts))
    }

    fn gather(&mut self, _centre: Vf, _reach: F) -> bool {
        true
    }
}

/// **The quad as a shell steps it** (MD123): its keys ([`Params`]), its words ([`Quad`]), its rigid
/// body, the bike steps' own words ([`Bike`]) and whoever answers the calls those leave (`rest`), one
/// [`frame`] a tick. The same type rides Desert Oasis in the tests ([`Ride::frame`] handed
/// [`crate::atv_ground::Terrain`]'s query and [`crate::atv_search::Oasis`]) and an MX Unleashed world in
/// the shell ([`Ride::frame_on`]).
///
/// What the world step hands `rest` (a `Vec<Call>` lists them, until its owner clears it) and nobody
/// here answers:
///
/// - [`Call::GravityRecord`] with no record held ([`Bike::contacts`] `None`): the collision record's
///   virtual, whose contacts an MX Unleashed world never files.
/// - [`Call::SpatialMove`] and [`Call::PartMove`]: SLUS-21104's spatial database, which an MX Unleashed
///   world does not have; its sweep reads the world directly ([`OnWorld`]).
/// - [`Call::BodyReset`] and [`Call::Respawn`], the quad's origin off the world's box
///   ([`world_bounds`]).
/// - [`Call::PreloadCue`] (a sound cue), [`Call::StartStunt`]/[`Call::EndStunt`] (the rider's
///   animation) and [`Call::Wreck`] (the reason; [`Ride::wreck`] reads it).
pub struct Ride<R = Vec<Call>> {
    pub p: Params,
    pub q: Quad,
    pub body: crate::atv_body::Body,
    pub bike: Bike,
    pub rest: R,
    pub table: crate::air::AcosTable,
    /// MD123a's: the collision record run before each step ([`Ride::record_on`]); `None`, none runs.
    pub recorded: Option<Recorded>,
    /// MD123a's: the frame's tail run after each step ([`Ride::tail_on`]); `None`, none runs.
    pub tailed: Option<Tailed>,
    /// MD123a's: the wreck, its clock and the put-back ([`Ride::wreck_on`]); `None`, none runs.
    pub wrecked: Option<Wrecked>,
}

/// The quad's physics world: one iteration a substep, at least 115 substeps a second.
pub const WORLD: crate::biplane::World = crate::biplane::World { iterations: 1, rate: F(115.0) };

impl<R: Moto + Default> Ride<R> {
    pub fn new(p: Params, q: Quad, body: crate::atv_body::Body, bike: Bike, rest: R) -> Ride<R> {
        Ride { p, q, body, bike, rest, table: crate::air::AcosTable::build(), recorded: None, tailed: None, wrecked: None }
    }

    /// The pad's levers and stick where the pad reader stores them: the gas and brake into the quad,
    /// the stick into the input limiter's.
    pub fn controls(&mut self, c: &crate::ride::Controls) {
        (self.q.gas, self.q.brake) = (c.gas, c.brake);
        (self.bike.stick.x, self.bike.stick.y) = c.stick;
    }

    /// One tick of [`frame`], with `ground` and `sweep` answering [`Steps`]' ground query and sweep.
    pub fn frame<G: FnMut(Vf, &mut u8) -> (Vf, Vf), S: Sweep>(&mut self, dt: F, ground: G, sweep: S) {
        // The steps own a copy of the bike and the rest for the frame; both come back after.
        let (bike, rest) = (self.bike.clone(), std::mem::take(&mut self.rest));
        let mut m = Steps { bike, rest, ground, sweep };
        frame(&WORLD, &self.p, &mut self.q, &mut self.body, dt, &self.table, &mut m);
        (self.bike, self.rest) = (m.bike, m.rest);
    }

    /// One tick on an MX Unleashed world: [`Ride::frame`] with the ground query [`world_ground`] and
    /// the sweep [`OnWorld`] (`segments` the trigger probe's, [`trigger_segments`]), and the in-world
    /// test's box the world's ([`world_bounds`]).
    pub fn frame_on(&mut self, dt: F, world: &mut dyn crate::world::World, segments: [[Vf; 2]; 4]) {
        let b = &mut self.bike.parts.bounds;
        *b = world_bounds(world, b.floor);
        self.record_on(dt, world);
        {
            let cell = std::cell::RefCell::new(&mut *world);
            let ground = |at: Vf, record: &mut u8| world_ground(&mut **cell.borrow_mut(), at, record);
            self.frame(dt, ground, OnWorld { world: &cell, segments });
        }
        if let Some(r) = &mut self.recorded {
            // The held list comes back from the steps' bike for the next frame's refill.
            if let Some(h) = self.bike.contacts.take() {
                r.held = h;
            }
        }
        self.tail_on(dt);
        self.wreck_on(dt);
    }

    /// One tick as the shell steps it: the controls laid ([`Ride::controls`]), then [`Ride::frame_on`].
    /// A quad that is down ([`Ride::down`]) has no rider on it: its controls are let go.
    pub fn step(&mut self, c: &crate::ride::Controls, dt: F, world: &mut dyn crate::world::World, segments: [[Vf; 2]; 4]) {
        let none = crate::ride::Controls::default();
        self.controls(if self.down() { &none } else { c });
        self.frame_on(dt, world, segments);
    }
}

impl Ride<Vec<Call>> {
    /// The first wreck the steps entered since `rest` was last cleared ([`Call::Wreck`], its reason).
    pub fn wreck(&self) -> Option<u8> {
        self.rest.iter().find_map(|c| if let Call::Wreck(r) = c { Some(*r) } else { None })
    }
}

// --- MD123a: the collision record and the tail on MX Unleashed's worlds ------------------------------

/// **The collision record's outside on an MX Unleashed world** (MD123a), what
/// [`crate::handling::record::clear`] and [`crate::handling::record::pass`] ask of a quad on a track
/// the shell rides, in place of SLUS-21104's part search, sweep and terrain service (MD105's on Desert
/// Oasis):
///
/// - the world rays ([`Collides::search`]): the active part's rays (`rays`, in the scene node's frame)
///   stood at the node's world frame ([`crate::atv_moto::Scene::tip`]) and swept, sixteen to a bundle,
///   against everything solid and the placed groups ([`crate::world::World::solid`],
///   [`crate::world::World::groups`]). A ray that hit is one hit: its segment as the part keeps it,
///   the world's normal and point, and as what was struck the prop's owner number plus one (0 stays
///   "nothing") in category 0, with no record and no mover (nothing on an MX Unleashed world has
///   either);
/// - the sweep back ([`Collides::sweep`]): the segment swept as one ray against the same, its hit
///   the nearest;
/// - the heightfield rays' terrain ([`Collides::under`]): [`crate::world::World::ground`] at the
///   ray's tip, the surface under it ([`crate::world::World::wheel_surface`], else
///   [`crate::world::World::surface`]) in all four corners; the terrain's box the world's map edge
///   ([`world_bounds`]), or everything for a world with none;
/// - the node's moves ([`Collides::node_place`], [`Collides::node_move`]) the quad's scene's.
///
/// The body's damping the clear gives back is kept in `damped` for the caller, who holds the body.
pub struct RecordOn<'a> {
    pub world: &'a mut dyn crate::world::World,
    pub scene: &'a mut crate::atv_moto::Scene,
    pub rays: &'a [[Vf; 2]],
    pub damped: Option<(Vf, Vf)>,
    /// The category every struck prop is given (MX Unleashed's props have none of their own).
    pub category: u32,
    /// Another record a hit names: none ever is on an MX Unleashed world, so this is never reached.
    pub nobody: crate::handling::record::Record,
}

/// How far the terrain's box reaches on a world with no map edge.
const NO_EDGE: f32 = 1.0e30;

impl RecordOn<'_> {
    /// `segments` (world points) swept as ray bundles against the world: per segment, the hit's
    /// normal, point and owner number, or none.
    fn sweep_segments(&mut self, segments: &[[Vf; 2]]) -> Vec<Option<(Vf, Vf, i32)>> {
        use crate::collide::{Bundle, MAX_PROBES, Probe, Record};
        let mut out = Vec::with_capacity(segments.len());
        for chunk in segments.chunks(MAX_PROBES) {
            let centre = chunk[0][0].merge(Dest::W, Vf::VF0);
            let mut bundle = Bundle { count: chunk.len(), centre, ..Bundle::default() };
            let mut radius = F::ZERO;
            for (k, [a, b]) in chunk.iter().enumerate() {
                bundle.probes[k] = Probe::between(*a, *b);
                for p in [a, b] {
                    let d = p.sub(centre);
                    let r = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(d.x(), d.x()), d.y(), d.y()), d.z(), d.z())));
                    if r > radius {
                        radius = r;
                    }
                }
            }
            bundle.radius = radius;
            let mut record = Record::new();
            self.world.solid(&bundle, &mut record);
            self.world.groups(&bundle, &mut record);
            out.extend((0..chunk.len()).map(|k| record.hit[k].then(|| (record.normal[k], record.point[k], record.owner[k]))));
        }
        out
    }
}

impl crate::handling::record::Collides for RecordOn<'_> {
    fn damp(&mut self, linear: Vf, angular: Vf) {
        self.damped = Some((linear, angular));
    }
    fn node_rotate(&mut self, v: Vf) -> Vf {
        self.scene.turn(0, v)
    }
    fn node_position(&mut self) -> Vf {
        self.scene.tip(0, Vf::new(0.0, 0.0, 0.0, 1.0))
    }
    fn node_place(&mut self, v: Vf) -> Vf {
        self.scene.tip(0, v)
    }
    fn node_move(&mut self, at: Vf) {
        // The quad's node has no parent: the move is always made.
        let _ = self.scene.set_position(0, at);
    }
    fn node_up(&self) -> [F; 3] {
        let w = &self.scene.nodes[0].world;
        [F(w[0].y()), F(w[1].y()), F(w[2].y())]
    }
    fn chassis_velocity(&self) -> Vf {
        Vf::ZERO
    }
    fn sweep(&mut self, from: Vf, to: Vf, _nearby: bool) -> Option<crate::handling::record::Swept> {
        let (normal, point, owner) = self.sweep_segments(&[[from, to]])[0]?;
        let struck = crate::handling::record::Struck { object: (owner as u32).wrapping_add(1), category: self.category, ..Default::default() };
        Some(crate::handling::record::Swept { normal, point, struck })
    }
    fn place_part(&mut self) {
        // The rays are stood at the node's frame when they are searched.
    }
    fn part_rays(&self) -> Vec<[Vf; 2]> {
        self.rays.to_vec()
    }
    fn search(&mut self, _nearby: bool) -> Vec<crate::handling::record::Hit> {
        let rays = self.rays;
        let stood: Vec<[Vf; 2]> = rays.iter().map(|[a, b]| [self.scene.tip(0, *a), self.scene.tip(0, *b)]).collect();
        let hits = self.sweep_segments(&stood);
        rays.iter()
            .zip(hits)
            .filter_map(|(&[from, to], h)| {
                let (normal, point, owner) = h?;
                let struck = crate::handling::record::Struck { object: (owner as u32).wrapping_add(1), category: self.category, ..Default::default() };
                Some(crate::handling::record::Hit { from, to, normal, point, struck })
            })
            .collect()
    }
    fn terrain_box(&self) -> [Vf; 2] {
        match world_bounds(&*self.world, F::ZERO).area {
            Some((lo, hi)) => [lo, hi],
            None => [Vf::splat(fpu::neg(NO_EDGE)), Vf::splat(NO_EDGE)],
        }
    }
    fn under(&mut self, g: &mut crate::contact_search::Ground) {
        let at = g.point;
        let (point, normal) = self.world.ground(at);
        let s = match self.world.wheel_surface(at) {
            Some(s) => s,
            None => self.world.surface(at),
        };
        (g.point, g.normal, g.surfaces) = (point, normal, [s; 4]);
    }
    fn velocity(&mut self, _s: &crate::handling::record::Struck) -> Vf {
        Vf::ZERO
    }
    fn other(&mut self, _id: u32) -> &mut crate::handling::record::Record {
        &mut self.nobody
    }
}

/// **The quad's collision record on an MX Unleashed world** (MD123a), as [`Ride`] keeps it: the
/// record ([`crate::handling::record::Record`]), its resolution's fields and list
/// ([`crate::atv_contact::Held`], refilled from each pass), the riding rays and the wreck rays (each
/// in the scene node's frame) and which of the two the record runs on ([`Rays`], [`swap_rays`]).
#[derive(Clone, Debug)]
pub struct Recorded {
    pub record: crate::handling::record::Record,
    pub held: crate::atv_contact::Held,
    pub riding: Vec<[Vf; 2]>,
    pub wreck: Vec<[Vf; 2]>,
    pub rays: Rays,
}

impl Recorded {
    /// The rays the record runs on: the wreck rays once [`swap_rays`] put them in, else the riding.
    pub fn active(&self) -> &[[Vf; 2]] {
        if self.rays.active == self.rays.wreck && self.rays.wreck != self.rays.riding { &self.wreck } else { &self.riding }
    }
}

/// **The quad's frame tail on an MX Unleashed world** (MD123a), as [`Ride`] keeps it: the update's
/// words ([`Tail`]), the vehicle's words its hazard clock keeps ([`crate::handling::tail::Vehicle`]:
/// free, stuck, the stuck clock, the run's flags) and the put-back's ([`PutBack`]).
#[derive(Clone, Debug, Default)]
pub struct Tailed {
    pub tail: Tail,
    pub vehicle: crate::handling::tail::Vehicle,
    pub put_back: PutBack,
}

/// The hazard clock's outside ([`crate::handling::tail::stuck`]): the vehicle's words, and the
/// put-backs it asks for, counted.
struct Clock<'a> {
    v: &'a mut crate::handling::tail::Vehicle,
    put_backs: u32,
}

impl crate::handling::tail::Vehicles for Clock<'_> {
    fn vehicle(&mut self) -> &mut crate::handling::tail::Vehicle {
        self.v
    }
    fn skipped(&mut self) -> bool {
        false
    }
    fn paused(&mut self) -> bool {
        false
    }
    fn rumble_ready(&mut self) -> bool {
        false
    }
    fn word(&mut self, _at: u32) -> u32 {
        0
    }
    fn out(&mut self, c: crate::handling::tail::Out, _dt: F) {
        // The hazard clock asks only for the put-back.
        if c == crate::handling::tail::Out::PutBack {
            self.put_backs += 1;
        }
    }
}

/// The tail's calls on an MX Unleashed world: the hazard surface answered by the vehicle's hazard
/// clock ([`crate::handling::tail::stuck`]), its put-back by [`put_back`] (place and keep set, as the
/// clock asks it); every other call handed to `rest`.
struct TailOn<'a, R> {
    vehicle: &'a mut crate::handling::tail::Vehicle,
    put_back: &'a mut PutBack,
    on: PutOn<'a, R>,
}

impl<R: Moto> Moto for TailOn<'_, R> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body) {
        match c {
            Call::HazardSurface { dt, time, sunk } => {
                let mut clock = Clock { v: self.vehicle, put_backs: 0 };
                crate::handling::tail::stuck(&mut clock, sunk, dt, time);
                for _ in 0..clock.put_backs {
                    put_back(self.put_back, q, body, true, true, &mut self.on);
                }
            }
            c => self.on.rest.call(c, q, body),
        }
    }
}

/// The put-back's calls on an MX Unleashed world: the vehicle's put-back
/// ([`Call::VehiclePutBack`]) answered by standing the body still at the spot kept (where it, its
/// orientation; [`Wrecked::spot`]), or where it is for none; every other call handed to `rest`.
struct PutOn<'a, R> {
    spot: Option<(Vf, Vf)>,
    rest: &'a mut R,
}

impl<R: Moto> Moto for PutOn<'_, R> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body) {
        match c {
            Call::VehiclePutBack { .. } => {
                if let Some((at, turn)) = self.spot {
                    (body.position, body.orientation) = (at, turn);
                }
                (body.velocity, body.ang_vel, body.force, body.moment) = (Vf::ZERO, Vf::ZERO, Vf::ZERO, Vf::ZERO);
                self.rest.call(c, q, body);
            }
            c => self.rest.call(c, q, body),
        }
    }
}

/// **The quad's wreck on an MX Unleashed world** (MD123a), as [`Ride`] keeps it: the reason it is
/// down for (`0` riding), the clock it is down for, `MaxWreckTime`, the struck class the reason-15
/// test masks a struck object's category with and the category an MX Unleashed prop is given, and the
/// spot it is put back to (the last place it stood on all four wheels, and its orientation there)
/// with where its scene node was then.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrecked {
    pub reason: u8,
    pub clock: F,
    pub max_time: F,
    pub class: u32,
    pub category: u32,
    pub spot: Option<(Vf, Vf)>,
    pub node: Vf,
}

impl<R: Moto + Default> Ride<R> {
    /// **The collision record's clear and pass** (MD123a, [`crate::handling::record::clear`] and
    /// [`crate::handling::record::pass`] on [`RecordOn`]), before the steps: the pass's contacts into
    /// the held list ([`crate::atv_contact::Held::refill`]) the gravity's call resolves
    /// ([`Bike::contacts`]), the record's touch and its struck object's class into [`Bike::touching`]
    /// and [`Bike::struck_class`], and the damping the clear gives the body.
    pub fn record_on(&mut self, dt: F, world: &mut dyn crate::world::World) {
        let Some(r) = self.recorded.as_mut() else { return };
        let (class, category) = self.wrecked.as_ref().map_or((0, 0), |w| (w.class, w.category));
        let rays = r.active().to_vec();
        let mut on = RecordOn { world, scene: &mut self.bike.parts.scene, rays: &rays, damped: None, category, nobody: Default::default() };
        crate::handling::record::clear(&mut r.record, &mut on);
        let mut view = self.body.view();
        let body = if r.record.body { Some(&mut view) } else { None };
        // `me` names the record to another one's contact: there is no other on an MX Unleashed world.
        crate::handling::record::pass(&mut r.record, 1, body, dt, &mut on);
        if let Some((linear, angular)) = on.damped {
            (self.body.damping, self.body.ang_damping) = (linear, angular);
        }
        self.body.set_view(&view);
        // A contact is never with another record or a mover here, so the refill always holds.
        r.held.refill(&r.record.contacts);
        self.bike.contacts = Some(r.held.clone());
        self.bike.touching = r.record.touching;
        self.bike.struck_class = r.record.touching && r.record.struck != 0 && category & class != 0;
    }

    /// **The frame's tail** (MD123a, [`update`] on [`Tailed`]), after the steps: the wheels' surfaces,
    /// sunk flags and squeezes and the node's up and forward rows laid from the bike, then the update
    /// with its hazard surface answered by the vehicle's hazard clock and the clock's put-back by
    /// [`put_back`] ([`TailOn`]). Its other calls go to `rest`: the vehicle update
    /// ([`Call::BikeVehicleUpdate`], which the steps ran already), the rumble, the stunts' start and
    /// end, the countdown's three and the post.
    pub fn tail_on(&mut self, dt: F) {
        let Some(t) = self.tailed.as_mut() else { return };
        let b = &self.bike;
        t.tail.wheel_surface = b.more.map(|m| m.record);
        t.tail.wheel_sunk = b.wheels.map(|w| w.ray_hit != 0);
        t.tail.wheel_squeeze = b.wheels.map(|w| w.compression);
        let node = &b.parts.scene.nodes[0].world;
        (t.tail.node_up, t.tail.node_forward) = (node[1], node[2]);
        let spot = self.wrecked.as_ref().and_then(|w| w.spot);
        let mut view = self.body.view();
        let mut m = TailOn { vehicle: &mut t.vehicle, put_back: &mut t.put_back, on: PutOn { spot, rest: &mut self.rest } };
        update(&mut t.tail, &mut self.q, &mut view, dt, &mut m);
        self.body.set_view(&view);
    }

    /// **The swap between the riding and the wreck rays** ([`swap_rays`]) on the record's rays, the
    /// one swapped to in the world already.
    pub fn swap(&mut self, wrecked: bool) {
        let Some(r) = self.recorded.as_mut() else { return };
        let mut view = self.body.view();
        swap_rays(&mut r.rays, wrecked, true, &mut self.q, &mut view, &mut self.rest);
        self.body.set_view(&view);
    }

    /// **The wreck, its clock and the put-back** (MD123a), after the tail: riding, reason 15 asked
    /// as the backflip asks it ([`crate::handling::step::flip_wreck_steep`]: the record touching a
    /// struck-class object, and airborne or the loads past their limits); a wreck is told to `rest`
    /// ([`Call::Wreck`]), swaps the wreck rays in and starts the clock at `MaxWreckTime`. Down, the
    /// clock counts and, run out, swaps the riding rays back and puts the quad back ([`put_back`],
    /// place and keep set) at the spot kept. Riding on all four wheels keeps the spot.
    pub fn wreck_on(&mut self, dt: F) {
        let Some(w) = self.wrecked.as_mut() else { return };
        let mut view = self.body.view();
        if w.reason == 0 {
            let loads = crate::air::Loads { side: F(self.q.attitude.x()), fore_aft: F(self.q.attitude.z()), ..Default::default() };
            if crate::handling::step::flip_wreck_steep(&self.bike.air_keys, self.bike.struck_class, self.q.airborne, &loads) {
                const STRUCK: u8 = crate::air::Wreck::Struck as u8;
                (w.reason, w.clock) = (STRUCK, w.max_time);
                self.rest.call(Call::Wreck(STRUCK), &mut self.q, &mut view);
                self.body.set_view(&view);
                self.swap(true);
            } else if self.q.all_down {
                (w.spot, w.node) = (Some((view.position, view.orientation)), self.bike.parts.scene.nodes[0].world[3]);
            }
            return;
        }
        w.clock = F(fpu::sub(w.clock.0, dt.0));
        if w.clock > F::ZERO {
            return;
        }
        (w.reason, w.clock) = (0, F::ZERO);
        let (spot, node) = (w.spot, w.node);
        self.swap(false);
        let mut view = self.body.view();
        let mut own = PutBack::default();
        let pb = match self.tailed.as_mut() {
            Some(t) => &mut t.put_back,
            None => &mut own,
        };
        put_back(pb, &mut self.q, &mut view, true, true, &mut PutOn { spot, rest: &mut self.rest });
        self.body.set_view(&view);
        (self.body.last_position, self.body.last_quat) = (self.body.position, self.body.quat);
        self.bike.parts.last_position = self.body.position;
        if let Some((_, turn)) = spot {
            self.bike.parts.scene.place(0, turn, node);
        }
    }

    /// Whether the quad is down ([`Wrecked::reason`]).
    pub fn down(&self) -> bool {
        self.wrecked.as_ref().is_some_and(|w| w.reason != 0)
    }
}

/// The wreck body's calls on an MX Unleashed world: the wreck rays' load ([`Call::PartLoad`] of
/// [`Part::Wreck`]) answered with `file`'s segments, every other call handed to `rest`.
struct WreckLoad<'a, R> {
    file: &'a crate::track::Rays,
    loaded: Option<Vec<[Vf; 2]>>,
    rest: &'a mut R,
}

impl<R: Moto> Moto for WreckLoad<'_, R> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body) {
        match c {
            Call::PartLoad(Part::Wreck) => self.loaded = Some(crate::flight_search::Rays::of(self.file).pairs()),
            c => self.rest.call(c, q, body),
        }
    }
}

impl<R: Moto + Default> Ride<R> {
    /// **The wreck body built** (MD123a, [`build_wreck_body`]) for the collision record: the wreck
    /// rays loaded from `file` (the quad's [`WRECK_FILE`], parsed with
    /// [`crate::track::Rays::parse`]) into [`Recorded::wreck`], and the record's rays left on the
    /// riding ones with the wreck rays' handle (`wreck`) to swap to ([`Ride::swap`]). The other calls
    /// (the two parts made, flagged, readied and hung) go to `rest`. Nothing is done without a record.
    #[allow(clippy::too_many_arguments)] // build_wreck_body's inputs and the file
    pub fn build_wreck(
        &mut self,
        w: &mut WreckBody,
        file: &crate::track::Rays,
        trigger: u32,
        wreck: u32,
        actor: u32,
        category: impl FnMut(&str) -> u32,
    ) {
        let Some(r) = self.recorded.as_mut() else { return };
        let mut view = self.body.view();
        let mut m = WreckLoad { file, loaded: None, rest: &mut self.rest };
        build_wreck_body(w, trigger, wreck, actor, category, &mut self.q, &mut view, &mut m);
        if let Some(rays) = m.loaded {
            r.wreck = rays;
        }
        let riding = w.riding_kept;
        r.rays = Rays { active: riding, active_too: riding, record: riding, wreck: w.wreck.handle, riding };
        self.body.set_view(&view);
    }
}

// --- MD123: the quad made from its files -----------------------------------------------------------

/// The body's top speed and the constructor's other words the loaders leave alone, as the spawned
/// quad holds them (C28's spawn image): the rider's stunt with none playing, the backflip's stunt id
/// ([`crate::handling::FIRST_FLIP_STUNT`]) and the 360's two (right, left), its direction, and the
/// quad's flag byte.
const SPAWN_MAX_SPEED: f32 = 300.0;
const NO_STUNT: i32 = 0xff;
const SPIN_STUNTS: (u32, u32) = (44, 45);
const SPAWN_SPIN_DIR: i32 = 1;
const SPAWN_FLAGS: u8 = 5;
/// The steering hinge's axis each wheel starts with, the frame's up.
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);
/// The quad's control authority's ramp time as spawned.
const SPAWN_AUTHORITY_TIME: f32 = 1.0;

impl Model {
    /// [`Model::half_box`]'s whole box, its centre (in `frame`) with the half extent: the quad's scene
    /// node's bounds.
    pub fn bounds(&self, frame: &crate::math::Mat4) -> crate::props::NodeBox {
        use crate::prop_world::BoundsNode;
        let worlds = self.worlds(frame);
        fn tree(m: &Model, worlds: &[crate::math::Mat4], i: usize) -> BoundsNode {
            let children = (0..m.nodes.len()).filter(|&k| m.nodes[k].parent == Some(i)).map(|k| tree(m, worlds, k)).collect();
            BoundsNode { own: Some(m.nodes[i].own), world: worlds[i], children }
        }
        let roots = (0..self.nodes.len()).filter(|&k| self.nodes[k].parent.is_none()).map(|k| tree(self, &worlds, k)).collect();
        crate::prop_world::node_bounds(&BoundsNode { own: None, world: *frame, children: roots })
    }

    /// The model's first root and, below it, each node's first child (in the file's order), as far as
    /// `n` nodes: the chain [`crate::atv_moto::Scene`] holds under the quad's node.
    pub fn first_chain(&self, n: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut at = self.nodes.iter().position(|m| m.parent.is_none());
        while let Some(i) = at.filter(|_| chain.len() < n) {
            chain.push(i);
            at = (0..self.nodes.len()).find(|&k| self.nodes[k].parent == Some(i));
        }
        chain
    }
}

impl<R: Moto + Default> Ride<R> {
    /// **The quad made from its files** (MD123), with no RAM image: `load` (its `.phy`, read with the
    /// model's box and mounts, [`Load::read_model`]) over a quad standing still at `at` facing
    /// `heading` ([`spawn_frame`]), its chassis `model` hung under its scene node.
    ///
    /// - The scene node (the scene's node 0) is the spawn frame with no parent, its box the model's
    ///   ([`Model::bounds`]); under it the model's first chain ([`Model::first_chain`]) with their
    ///   bind locals, up to [`crate::atv_moto::NODES`].
    /// - The body is one foot up the frame from the node (the node is the body less its up row, as
    ///   the steps keep it), turned to the frame ([`crate::math::mat4_to_quat`]), still, enabled on
    ///   every axis, with the loaded mass, damping, inertia and rates ([`Load::tune_body`]), the top
    ///   speed [`SPAWN_MAX_SPEED`] and no acceleration limits.
    /// - The quad, its keys and the bike steps' words are the loaded ones ([`Load::params`],
    ///   [`Load::tune_quad`], [`Load::tune`]) over a quad at rest: in first, engine at its idle state,
    ///   every wheel on its ground query ([`crate::atv_wheel::Ground::Still`]) at its spring when
    ///   stopped, the engine on the rear pair, the gyros [`crate::handling::Gyro::default`], the
    ///   360's keys `[Three Sixty]`'s, and the world's box `bounds` with the part reader's reset
    ///   height as its floor.
    /// - The collision sync is off and the quad has no spatial entry: every call either makes is a
    ///   move in SLUS-21104's spatial database, which an MX Unleashed world does not have.
    ///
    /// No collision record, tail or wreck is laid ([`Ride::recorded`], [`Ride::tailed`],
    /// [`Ride::wrecked`] `None`); [`Ride::rig`] lays them.
    pub fn spawn(load: &Load, model: &Model, at: Vf, heading: Vf, bounds: Option<(Vf, Vf)>, rest: R) -> Ride<R> {
        use crate::atv_moto;
        let frame = spawn_frame(at, heading);
        let rows = frame.0;
        let w1 = |v: Vf| v.merge(Dest::W, Vf::new(0.0, 0.0, 0.0, 1.0));
        let zero = Vf::new(0.0, 0.0, 0.0, 1.0);

        // The scene: the quad's node, then the model's chain under it.
        let mut scene = atv_moto::Scene::default();
        let b = model.bounds(&frame);
        scene.nodes[0] = atv_moto::Node { local: rows, world: rows, centre: b.centre, half: b.half_extent, ..atv_moto::Node::default() };
        let chain = model.first_chain(atv_moto::NODES - 1);
        for (k, &i) in chain.iter().enumerate() {
            let n = k + 1;
            scene.nodes[n] = atv_moto::Node { local: model.nodes[i].local.0, parent: Some(n - 1), ..atv_moto::Node::default() };
            scene.nodes[n - 1].child = Some(n);
        }
        scene.update(chain.len());

        // The body: a foot up the frame's up row from the node.
        let position = Vf::new(at.x(), at.y(), at.z(), 1.0).merge(Dest::XYZ, at.add(rows[1]));
        let quat = crate::math::mat4_to_quat(&frame);
        let mut body = crate::atv_body::Body {
            enabled: true,
            linear_axes: [true; 3],
            angular_axes: [true; 3],
            last_position: position,
            position,
            last_quat: quat,
            quat,
            velocity: zero,
            last_force: zero,
            force: zero,
            accel: zero,
            correction_peak: zero,
            correction: zero,
            moment: zero,
            last_moment: zero,
            rates: zero,
            ang_accel: zero,
            twist_peak: zero,
            twist: zero,
            inv_inertia: zero,
            max_speed: F(SPAWN_MAX_SPEED),
            max_accel: F(f32::MAX),
            max_ang_accel: F(f32::MAX),
            ..crate::atv_body::Body::default()
        };
        load.tune_body(&mut body);

        // The quad's own words.
        let mut q = Quad {
            active: true,
            push: true,
            authority_time: F(SPAWN_AUTHORITY_TIME),
            heading: w1(rows[2]),
            up: w1(rows[1]),
            node_pos: w1(rows[3]),
            node_pos_kept: w1(rows[3]),
            node_local: zero,
            ..Quad::default()
        };
        load.tune_quad(&mut q);
        let springs = [load.tires.front_spring.0, load.tires.front_spring.0, load.tires.rear_spring.0, load.tires.rear_spring.0];
        for (w, s) in q.wheels.iter_mut().zip(springs) {
            w.spring = s;
        }

        // The bike steps' words.
        let t = &load.three_sixty;
        let spin = atv_moto::Spin {
            min_time: t.min_time,
            max_time: t.max_time,
            min_pitch: t.min_torque_x,
            max_pitch: t.max_torque_x,
            min_yaw: t.min_torque_y,
            max_yaw: t.max_torque_y,
            rate: t.rate_decrease,
            moment: zero,
            dir: SPAWN_SPIN_DIR,
            stunt_right: SPIN_STUNTS.0,
            stunt_left: SPIN_STUNTS.1,
            ..atv_moto::Spin::default()
        };
        let parts = atv_moto::Parts {
            cofg_off: load.bike.cofgs[1],
            last_position: position,
            body_on: true,
            flags: SPAWN_FLAGS,
            record: true,
            spin,
            scene,
            bounds: atv_moto::Bounds { floor: load.part.gravity.minimum_reset_height, area: bounds },
            ..atv_moto::Parts::default()
        };
        let wheels = std::array::from_fn(|k| crate::wheel::Wheel {
            has_ground: true,
            steered: k < LEFT_REAR,
            spring_rate: springs[k],
            ..crate::wheel::Wheel::default()
        });
        let more = [crate::atv_wheel::More { steer_axis: UP, ..crate::atv_wheel::More::default() }; 4];
        let spins = std::array::from_fn(|_| crate::atv_wheel::Spin { brake: Some(F::ZERO), ..crate::atv_wheel::Spin::default() });
        let mut bike = Bike {
            held: crate::ride::Held::default(),
            gas_cut: crate::handling::step::GasCut::default(),
            cut_time: F::ZERO,
            joystick: load.joystick,
            stick: crate::trick::Stick::default(),
            clutch: F::ZERO,
            gate_cut: false,
            clutch_keys: crate::r#move::Params::default(),
            ride: crate::r#move::Ride::default(),
            // The constructor's two torque scales, both one (the bike's, [`crate::ride`]'s, and C28's
            // spawned quad's): no `.phy` key writes them, and a zeroed engine makes no torque.
            engine: crate::r#move::Engine { clutch_torque_scale: F::ONE, torque_scale: F::ONE, ..load.engine },
            air_keys: load.air_keys(),
            air: crate::air::Air::default(),
            stick_hold: F::ZERO,
            preload_cue: false,
            spin_applied: false,
            spin_active: false,
            touching: false,
            struck_class: false,
            buttons: 0,
            right_y: F(rows[1].y()),
            up_y: F(rows[2].y()),
            stunt_id: NO_STUNT,
            rider_stunt: NO_STUNT,
            flip_clip: crate::handling::FlipClip::default(),
            flip_stunt: crate::handling::FIRST_FLIP_STUNT as u32,
            gas_gyro_torque: load.gas_gyro_torque,
            gyro: crate::handling::Gyro::default(),
            wheels,
            more,
            ground: [crate::atv_wheel::Ground::Still; 4],
            cofg: load.part.body.cofg,
            spins,
            parts,
            contacts: None,
        };
        load.tune(&mut bike);
        Ride::new(load.params(), q, body, bike, rest)
    }
}

/// Where [`Ride::rig`] lays the part reader's words to read the collision record back
/// ([`crate::handling::record::read`], [`crate::handling::record::held`]): scratch places, nobody's
/// RAM.
const RIG_AT: crate::handling::mass::Layout = crate::handling::mass::Layout { object: 0x1000, body: 0x2000, record: 0x3000, gravity: 0x4000 };
const RIG_CLASS_AT: u32 = 0x5000;
/// The record's constructor words no `.phy` key writes, as the spawned quad holds them (the Oasis
/// spawn image's record): the actor bit it is, what it collides with (everything), and its class
/// word (the record's `+0x208` object's `+0x94`).
const RECORD_ACTORS: u32 = 0x1000;
const RECORD_COLLIDES: u32 = !0;
const RECORD_CLASS: u32 = 0x3012;
/// The handles the quad's ray parts and the quad itself go by ([`build_wreck_body`]'s lists, the
/// swap): any distinct words other than 0, since an MX Unleashed world never names them.
const RIDING_RAYS: u32 = 1;
const WRECK_RAYS: u32 = 2;
const TRIGGER_PROBE: u32 = 3;
const QUAD_ACTOR: u32 = 4;
/// The tail's words the spawned quad holds that no file writes: the vehicle state's mode (1, its
/// rumble and second countdown run) and the speed below which no wheelie or stoppie runs.
const SPAWN_VEHICLE_MODE: u32 = 1;
const SPAWN_STUNT_SPEED: f32 = 5.0;
/// The class a struck object counts for the backflip's wreck (the image's struck-class word).
const SPAWN_STRUCK_CLASS: u32 = 0x20;

impl<R: Moto + Default> Ride<R> {
    /// **MD123a's record, tail and wreck from the files** (MD123), for a quad [`Ride::spawn`] made:
    ///
    /// - The collision record is what the part reader writes ([`crate::handling::mass::Part::words`],
    ///   laid at scratch places and read back as the record and its resolution's fields) over the
    ///   record's constructor: the world rays, the rays and the sweep on as `CheckForCollisions` and
    ///   `CheckForHighSpeedCollisions` say, rests and damping of one, a body, the terrain, the
    ///   near-field list, one contact a riding ray, and the two probes; the held list's body is the
    ///   ride's.
    /// - The riding rays are `riding`'s (the bike's `m.col`, a ray probe), the wreck body is built on
    ///   `wreck` ([`WRECK_FILE`], [`Ride::build_wreck`]).
    /// - The tail has the vehicle state's mode [`SPAWN_VEHICLE_MODE`] and the stunt speed
    ///   [`SPAWN_STUNT_SPEED`], every surface no hazard; the wreck the `.phy`'s `MaxWreckTime` and the
    ///   struck class [`SPAWN_STRUCK_CLASS`].
    pub fn rig(&mut self, load: &Load, riding: &crate::track::Rays, wreck: &crate::track::Rays) {
        use crate::handling::record;
        let riding = crate::flight_search::Rays::of(riding).pairs();
        let r = &load.part.record;
        let mut words = std::collections::BTreeMap::new();
        for (a, mask, v) in load.part.words(RIG_AT) {
            let w = words.entry(a).or_insert(0u32);
            *w = (*w & !mask) | (v & mask);
        }
        let at = RIG_AT.record;
        let one = F::ONE.to_bits();
        for (o, v) in [
            (0x28, 1),
            (0x2c, u32::from(r.check_for_collisions != 0)),
            (0x30, u32::from(r.check_high_speed != 0)),
            (0x48, one),
            (0x4c, one),
            (0x50, one),
            (0x54, one),
            (0xe4, RIG_AT.body),
            (0xec, 1),
            (0xf4, riding.len() as u32),
            (0xfc, 1),
            (0x100, RECORD_ACTORS),
            (0x104, RECORD_COLLIDES),
            (0x200, 1),
            (0x204, 1),
            (0x208, RIG_CLASS_AT),
        ] {
            words.insert(at + o, v);
        }
        // The `w` lanes of the centre of mass in the world and the relative velocity.
        for o in [0x9c, 0x1fc] {
            words.insert(at + o, one);
        }
        words.insert(RIG_CLASS_AT + 0x94, RECORD_CLASS);
        let w = |a: u32| words.get(&a).copied().unwrap_or(0);
        let mut held = record::held(&w, at).expect("a record with no contacts is held");
        held.body = Some(self.body);
        self.recorded = Some(Recorded {
            record: record::read(&w, at),
            held,
            riding,
            wreck: Vec::new(),
            rays: Rays { active: RIDING_RAYS, active_too: RIDING_RAYS, record: RIDING_RAYS, wreck: RIDING_RAYS, riding: RIDING_RAYS },
        });
        let mut body = WreckBody { riding: Col::made(RIDING_RAYS), ..WreckBody::default() };
        self.build_wreck(&mut body, wreck, TRIGGER_PROBE, WRECK_RAYS, QUAD_ACTOR, |_| 0);
        let tail = Tail { vehicle_state: Some(SPAWN_VEHICLE_MODE), stunt_speed: F(SPAWN_STUNT_SPEED), ..Tail::default() };
        self.tailed = Some(Tailed { tail, ..Tailed::default() });
        self.wrecked = Some(Wrecked {
            max_time: load.max_wreck_time,
            class: SPAWN_STRUCK_CLASS,
            category: SPAWN_STRUCK_CLASS,
            ..Wrecked::default()
        });
    }
}
